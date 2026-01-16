use super::*;

mod canvas;
mod tool_preview;
mod ui;

impl WaylandState {
    pub(in crate::backend::wayland) fn render(&mut self, qh: &QueueHandle<Self>) -> Result<bool> {
        debug!("=== RENDER START ===");
        let board_mode = self.input_state.board_mode();
        let suppression = if self.data.overlay_suppression == OverlaySuppression::Zoom
            && board_mode != BoardMode::Transparent
        {
            OverlaySuppression::None
        } else {
            self.data.overlay_suppression
        };
        let render_canvas = !matches!(
            suppression,
            OverlaySuppression::Frozen | OverlaySuppression::Zoom
        );
        let render_ui = suppression == OverlaySuppression::None;

        // Create pool if needed
        let buffer_count = self.config.performance.buffer_count as usize;
        let scale = self.surface.scale().max(1);
        let width = self.surface.width();
        let height = self.surface.height();
        let phys_width = width.saturating_mul(scale as u32);
        let phys_height = height.saturating_mul(scale as u32);
        let now = Instant::now();
        let highlight_active = self.input_state.advance_click_highlights(now);
        let preset_feedback_active = self.input_state.advance_preset_feedback(now);
        let ui_toast_active = self.input_state.advance_ui_toast(now);
        let blocked_feedback_active = self.input_state.advance_blocked_feedback(now);
        let ui_animation_active = highlight_active
            || preset_feedback_active
            || ui_toast_active
            || blocked_feedback_active;
        self.update_ui_animation_tick(now, ui_animation_active);
        let keep_rendering = ui_animation_active && self.ui_animation_interval.is_none();

        // Get a buffer from the pool for rendering
        let (buffer, canvas_ptr, pool_gen, pool_size) = {
            let (pool, generation) = self.surface.ensure_pool(&self.shm, buffer_count)?;
            debug!(
                "Requesting buffer from pool (gen {}, size {})",
                generation,
                pool.len()
            );
            let (buf, cvs) = pool
                .create_buffer(
                    phys_width as i32,
                    phys_height as i32,
                    (phys_width * 4) as i32,
                    wl_shm::Format::Argb8888,
                )
                .context("Failed to create buffer")?;
            // Capture canvas pointer as stable slot identifier for damage tracking.
            // SlotPool reuses the same memory regions, so this pointer identifies the slot.
            let ptr = cvs.as_mut_ptr();
            let key = ptr as usize;
            // Drop the slice borrow so we can query pool metadata; keep raw pointer for Cairo.
            let _ = cvs;
            let pool_size = pool.len();
            debug!("Buffer acquired from pool (slot ptr: 0x{:x})", key);
            (buf, key, generation, pool_size)
        };

        // SAFETY: This unsafe block creates a Cairo surface from raw memory buffer.
        // Safety invariants that must be maintained:
        // 1. `canvas_ptr` comes from a valid SlotPool slice with exactly (width * height * 4) bytes
        // 2. The buffer format ARgb32 matches the allocation (4 bytes per pixel: alpha, red, green, blue)
        // 3. The stride (width * 4) correctly represents the number of bytes per row
        // 4. `cairo_surface` and `ctx` are explicitly dropped before the buffer is committed to Wayland,
        //    ensuring Cairo doesn't access memory after ownership transfers
        // 5. No other references to this memory exist during Cairo's usage
        // 6. The buffer remains valid throughout Cairo's usage (enforced by Rust's borrow checker)
        let cairo_surface = unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                canvas_ptr as *mut u8,
                cairo::Format::ARgb32,
                phys_width as i32,
                phys_height as i32,
                (phys_width * 4) as i32,
            )
            .context("Failed to create Cairo surface")?
        };

        // Render using Cairo
        let draw_start = std::time::Instant::now();
        let ctx = cairo::Context::new(&cairo_surface).context("Failed to create Cairo context")?;

        // Clear with fully transparent background
        debug!("Clearing background");
        ctx.set_operator(cairo::Operator::Clear);
        ctx.paint().context("Failed to clear background")?;
        ctx.set_operator(cairo::Operator::Over);

        if render_canvas {
            self.render_canvas_layer(
                &ctx,
                width,
                height,
                scale,
                phys_width,
                phys_height,
                render_ui,
                now,
            )?;
        }

        // Flush Cairo
        debug!("Flushing Cairo surface");
        cairo_surface.flush();
        drop(ctx);
        drop(cairo_surface);

        let draw_duration = draw_start.elapsed();
        if draw_duration > std::time::Duration::from_millis(2) {
            debug!("Cairo draw took {:?}", draw_duration);
        }

        // Attach buffer and commit
        debug!("Attaching buffer and committing surface");
        let wl_surface = self
            .surface
            .wl_surface()
            .cloned()
            .context("Surface not created")?;
        wl_surface.set_buffer_scale(scale);
        wl_surface.attach(Some(buffer.wl_buffer()), 0, 0);

        // Add new dirty regions from input state to the per-buffer damage tracker.
        // This ensures each buffer accumulates damage since it was last rendered.
        let input_damage = self.input_state.take_dirty_regions();
        self.buffer_damage.add_regions(input_damage);

        // Record pool size after create_buffer to detect growth.
        self.surface.update_pool_size(pool_size);

        // Take damage for this buffer slot (identified by canvas memory address).
        // Pool identity (generation + size) is passed to detect pool recreation/growth.
        // SlotPool reuses the same memory regions for released buffers, so the
        // canvas pointer serves as a stable slot identifier across buffer reuse.
        let logical_damage = self.buffer_damage.take_buffer_damage(
            canvas_ptr,
            self.surface.width().min(i32::MAX as u32) as i32,
            self.surface.height().min(i32::MAX as u32) as i32,
            pool_gen,
            pool_size,
        );
        let scaled_damage = scale_damage_regions(logical_damage.clone(), scale);

        if debug_damage_logging_enabled() {
            debug!(
                "Damage (scaled): count={}, {}",
                scaled_damage.len(),
                damage_summary(&scaled_damage)
            );
        }

        // Apply per-buffer damage regions for correct incremental rendering.
        // Each buffer tracks damage since it was last displayed, avoiding stale pixels.
        for region in &scaled_damage {
            wl_surface.damage_buffer(region.x, region.y, region.width, region.height);
        }

        let force_frame_callback = self.frozen.preflight_pending()
            || self.zoom.preflight_pending()
            || self.capture.preflight_needs_frame_callback();
        if self.config.performance.enable_vsync {
            debug!("Requesting frame callback (vsync enabled)");
            wl_surface.frame(qh, wl_surface.clone());
        } else if force_frame_callback {
            debug!("Requesting frame callback (preflight)");
            wl_surface.frame(qh, wl_surface.clone());
            self.surface.set_frame_callback_pending(true);
        } else {
            debug!("Skipping frame callback (vsync disabled - allows back-to-back renders)");
        }

        wl_surface.commit();
        debug!("=== RENDER COMPLETE ===");

        // Render toolbar overlays if visible, only when state/hover changed.
        self.render_layer_toolbars_if_needed();

        if self.data.overlay_suppression == OverlaySuppression::Capture {
            self.capture.mark_preflight_rendered();
        }

        Ok(keep_rendering)
    }
}
