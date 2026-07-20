use super::*;

mod canvas;
mod tool_preview;
mod ui;
mod ui_effect_damage;

impl WaylandState {
    pub(in crate::backend::wayland) fn render(&mut self, qh: &QueueHandle<Self>) -> Result<bool> {
        debug!("=== RENDER START ===");
        let board_is_transparent = self.input_state.board_is_transparent();
        let suppression = self
            .data
            .overlay_suppression
            .effective_for_board(board_is_transparent);
        let render_canvas = suppression.renders_canvas();
        let render_ui = suppression.renders_ui();

        // Create pool if needed
        let buffer_count = self.config.performance.buffer_count as usize;
        let scale = self.surface.scale().max(1);
        let width = self.surface.width();
        let height = self.surface.height();
        let phys_width = width.saturating_mul(scale as u32);
        let phys_height = height.saturating_mul(scale as u32);
        let perf_enabled = self.perf_enabled();
        let mut render_breakdown = perf_enabled.then(|| PerfRenderBreakdown {
            surface_px: u64::from(phys_width).saturating_mul(u64::from(phys_height)),
            ..PerfRenderBreakdown::default()
        });
        macro_rules! record_stage {
            ($field:ident, $body:expr) => {{
                let stage_start = perf_enabled.then(Instant::now);
                let result = $body;
                if let (Some(breakdown), Some(stage_start)) =
                    (render_breakdown.as_mut(), stage_start)
                {
                    breakdown.stages.$field = breakdown
                        .stages
                        .$field
                        .saturating_add(Instant::now().saturating_duration_since(stage_start));
                }
                result
            }};
        }

        let now = Instant::now();
        let (
            highlight_active,
            preset_feedback_active,
            ui_toast_active,
            blocked_feedback_active,
            text_edit_entry_active,
        ) = record_stage!(advance_animations, {
            (
                self.input_state.advance_click_highlights(now),
                self.input_state.advance_preset_feedback(now),
                self.input_state.advance_ui_toast(now),
                self.input_state.advance_blocked_feedback(now),
                self.input_state.advance_text_edit_entry_feedback(now),
            )
        });
        let ui_animation_active = highlight_active
            || preset_feedback_active
            || ui_toast_active
            || blocked_feedback_active
            || text_edit_entry_active;
        self.update_ui_animation_tick(now, ui_animation_active);
        let keep_rendering = ui_animation_active && self.ui_animation_interval.is_none();

        // Add new dirty regions from input state to the per-buffer damage tracker.
        // We do this BEFORE acquiring the buffer/damage so the current frame's changes
        // are included in the damage for the current buffer.
        let logical_width = width.min(i32::MAX as u32) as i32;
        let logical_height = height.min(i32::MAX as u32) as i32;
        let mut damage_diagnostics = PerfDamageDiagnostics::default();
        record_stage!(dirty_collect, {
            let input_damage_report = self.input_state.take_dirty_region_report();
            let input_damage = input_damage_report.regions;
            let input_full_reason = input_full_damage_reason(input_damage_report.full_reason);
            damage_diagnostics.input_regions = input_damage.len();
            damage_diagnostics.input_full_reason = input_full_reason;
            damage_diagnostics.input_covers_surface =
                damage_covers_logical_surface(&input_damage, logical_width, logical_height);
            let force_full_damage_reason = self.render_force_full_damage_reason();
            // Transient UI effects (toasts, feedback flashes) emit targeted damage
            // instead of forcing full-surface redraws. Collect on every frame so the
            // previous-bounds tracking stays in sync even when full damage is forced
            // for other reasons.
            let tool_preview_active = render_ui && self.mouse_tool_preview_eligible();
            let zoom_chip_active = render_ui && self.zoom_chip_visible();
            let command_palette_active = render_ui && self.input_state.command_palette_is_engaged();
            let ui_effect_damage = self.collect_ui_effect_damage(
                ui_toast_active,
                preset_feedback_active,
                blocked_feedback_active,
                text_edit_entry_active,
                render_ui && self.input_state.show_status_bar,
                zoom_chip_active,
                command_palette_active,
                tool_preview_active,
                width,
                height,
            );
            if let Some(reason) = force_full_damage_reason {
                // Zoom and board pan use a world transform; full damage avoids
                // mismatched coordinate spaces.
                self.buffer_damage.mark_all_full(reason);
            } else if let Some(reason) = input_full_reason {
                self.buffer_damage.mark_all_full(reason);
            } else {
                self.buffer_damage.add_regions(input_damage);
                self.buffer_damage.add_regions(ui_effect_damage);
            }
        });

        // Get a buffer from the pool for rendering
        let (buffer, canvas_ptr, pool_gen, pool_size) = record_stage!(buffer_acquire, {
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
        });

        // Record pool size after create_buffer to detect growth.
        self.surface.update_pool_size(pool_size);

        // Take damage for this buffer slot (identified by canvas memory address).
        // Pool identity (generation + size) is passed to detect pool recreation/growth.
        // SlotPool reuses the same memory regions for released buffers, so the
        // canvas pointer serves as a stable slot identifier across buffer reuse.
        let damage_report = self.buffer_damage.take_buffer_damage_report(
            canvas_ptr,
            logical_width,
            logical_height,
            pool_gen,
            pool_size,
        );
        damage_diagnostics.buffer_regions_before_merge = damage_report.regions_before_merge;
        damage_diagnostics.buffer_regions_after_merge = damage_report.regions_after_merge;
        let mut logical_damage = damage_report.regions;
        let mut full_damage_reason = damage_report.full_reason;
        if logical_damage.is_empty()
            && let Some(full) = crate::util::Rect::new(0, 0, logical_width, logical_height)
        {
            logical_damage = vec![full];
            full_damage_reason = Some(FullDamageReason::EmptyDamageFallback);
            self.buffer_damage
                .mark_all_full(FullDamageReason::EmptyDamageFallback);
        }
        let damage_screen = logical_damage;
        damage_diagnostics.buffer_covers_surface =
            damage_covers_logical_surface(&damage_screen, logical_width, logical_height);
        let damage_world = if self.canvas_transform_active() {
            let scale = if self.zoom.active {
                self.zoom.scale.max(f64::MIN_POSITIVE)
            } else {
                1.0
            };
            let view_width = ((width as f64) / scale).ceil() as i32;
            let view_height = ((height as f64) / scale).ceil() as i32;
            let (view_x, view_y) = self.canvas_view_origin();
            crate::util::Rect::new(
                view_x.floor() as i32,
                view_y.floor() as i32,
                view_width,
                view_height,
            )
            .map(|rect| vec![rect])
            .unwrap_or_default()
        } else {
            damage_screen.clone()
        };
        let scaled_damage = scale_damage_regions(damage_screen.clone(), scale);
        let active_render_profile = self.input_state.active_render_profile().cloned();
        let remap_canvas = self.input_state.active_canvas_render_profile().is_some();
        let remap_ui = self.input_state.active_ui_render_profile().is_some();
        if let Some(breakdown) = render_breakdown.as_mut() {
            breakdown.render_profile = PerfRenderProfileKind::from_flags(remap_canvas, remap_ui);
        }
        let stride = (phys_width * 4) as i32;
        let canvas_len = phys_height as usize * stride as usize;

        // SAFETY: This unsafe block creates a Cairo surface from raw memory buffer.
        // Safety invariants that must be maintained:
        // 1. `canvas_ptr` comes from a valid SlotPool slice with exactly (width * height * 4) bytes
        // 2. The buffer format ARgb32 matches the allocation (4 bytes per pixel: alpha, red, green, blue)
        // 3. The stride (width * 4) correctly represents the number of bytes per row
        // 4. `cairo_surface` and `ctx` are explicitly dropped before the buffer is committed to Wayland,
        //    ensuring Cairo doesn't access memory after ownership transfers
        // 5. No other references to this memory exist during Cairo's usage
        // 6. The buffer remains valid throughout Cairo's usage (enforced by Rust's borrow checker)
        // Render using Cairo
        let draw_start = std::time::Instant::now();
        let (cairo_surface, ctx) = record_stage!(cairo_surface, {
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

            let ctx =
                cairo::Context::new(&cairo_surface).context("Failed to create Cairo context")?;
            (cairo_surface, ctx)
        });

        record_stage!(clear_clip, {
            // Optimization: Clip drawing to the damage regions.
            // This dramatically reduces CPU fill rate pressure on high-res screens by
            // avoiding redraws of static content (which is preserved in the back-buffer).
            // Note: Cairo works in logical coordinates if we scale it, but here we are
            // pre-scale (identity transform). We must scale the logical damage rects to pixels.
            if !damage_screen.is_empty() {
                for rect in &damage_screen {
                    // Scale logical rect to physical pixels
                    let x = rect.x as f64 * scale as f64;
                    let y = rect.y as f64 * scale as f64;
                    let w = rect.width as f64 * scale as f64;
                    let h = rect.height as f64 * scale as f64;
                    ctx.rectangle(x, y, w, h);
                }
                ctx.clip();
            }

            // Clear with fully transparent background (only clears within clip)
            debug!("Clearing background");
            ctx.set_operator(cairo::Operator::Clear);
            ctx.paint().context("Failed to clear background")?;
            ctx.set_operator(cairo::Operator::Over);
            Ok::<(), anyhow::Error>(())
        })?;

        if render_canvas {
            self.render_canvas_layer(
                &ctx,
                width,
                height,
                scale,
                phys_width,
                phys_height,
                now,
                &damage_world,
                render_breakdown.as_mut(),
            )?;
        }

        let mut has_ui_baseline = false;
        record_stage!(render_profile, {
            if let Some(profile) = active_render_profile.as_ref() {
                if remap_canvas && !remap_ui {
                    cairo_surface.flush();
                    // SAFETY: `canvas_ptr` points to the SlotPool memory for the buffer created above.
                    // Cairo has flushed all pending writes, so the rendered canvas pixels can be
                    // rewritten before UI is drawn on top.
                    let canvas = unsafe {
                        std::slice::from_raw_parts_mut(canvas_ptr as *mut u8, canvas_len)
                    };
                    profile.remap_argb8888_regions(
                        canvas,
                        phys_width as i32,
                        phys_height as i32,
                        stride,
                        &scaled_damage,
                    );
                    cairo_surface.mark_dirty();
                } else if !remap_canvas && remap_ui && render_ui {
                    cairo_surface.flush();
                    // SAFETY: `canvas_ptr` points to the SlotPool memory for the buffer created above.
                    // Cairo has flushed all pending writes, so we can snapshot the canvas-only pixels in
                    // a reusable scratch buffer before drawing UI and later remap only bytes changed by
                    // the UI pass.
                    let canvas = unsafe {
                        std::slice::from_raw_parts_mut(canvas_ptr as *mut u8, canvas_len)
                    };
                    self.data.render_profile_ui_baseline.resize(canvas_len, 0);
                    self.data.render_profile_ui_baseline.copy_from_slice(canvas);
                    has_ui_baseline = true;
                }
            }
        });

        record_stage!(ui, {
            self.render_ui_layer(&ctx, width, height, scale, render_ui);
        });

        // Flush Cairo
        debug!("Flushing Cairo surface");
        cairo_surface.flush();
        drop(ctx);
        drop(cairo_surface);

        record_stage!(render_profile, {
            if let Some(profile) = active_render_profile.as_ref() {
                // SAFETY: `canvas_ptr` points to the SlotPool memory for the buffer created above.
                // Cairo has been flushed and dropped, and the buffer has not been attached yet, so this
                // is the only active mutable access to the rendered pixel bytes.
                let canvas =
                    unsafe { std::slice::from_raw_parts_mut(canvas_ptr as *mut u8, canvas_len) };
                if remap_canvas && remap_ui {
                    profile.remap_argb8888_regions(
                        canvas,
                        phys_width as i32,
                        phys_height as i32,
                        stride,
                        &scaled_damage,
                    );
                } else if !remap_canvas && remap_ui && has_ui_baseline {
                    profile.remap_argb8888_regions_changed_from(
                        canvas,
                        &self.data.render_profile_ui_baseline,
                        phys_width as i32,
                        phys_height as i32,
                        stride,
                        &scaled_damage,
                    );
                }
            }
        });

        let draw_duration = draw_start.elapsed();
        if draw_duration > std::time::Duration::from_millis(2) {
            debug!("Cairo draw took {:?}", draw_duration);
        }

        record_stage!(damage_commit, {
            // Attach buffer and commit
            debug!("Attaching buffer and committing surface");
            let wl_surface = self
                .surface
                .wl_surface()
                .cloned()
                .context("Surface not created")?;
            wl_surface.set_buffer_scale(scale);
            wl_surface.attach(Some(buffer.wl_buffer()), 0, 0);

            // Damage logic moved to top of function (add_regions and take_buffer_damage).
            // We now use the computed screen-space damage for clipping and compositor hints.

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

            let capture_generation = self
                .data
                .overlay_capture_barrier
                .begin_main_surface_submission();
            if self.config.performance.enable_vsync {
                debug!("Requesting frame callback (vsync enabled)");
                let callback = self
                    .surface
                    .begin_frame_callback(wl_surface.clone(), capture_generation);
                wl_surface.frame(qh, callback);
            } else if capture_generation.is_some() {
                debug!("Requesting frame callback (preflight)");
                let callback = self
                    .surface
                    .begin_frame_callback(wl_surface.clone(), capture_generation);
                wl_surface.frame(qh, callback);
            } else {
                debug!("Skipping frame callback (vsync disabled - allows back-to-back renders)");
            }

            self.commit_perf_frame(
                PerfFrameDamageContext {
                    damage_screen: &damage_screen,
                    logical_width: width,
                    logical_height: height,
                    damage_rects: scaled_damage.len(),
                    force_full_reason: full_damage_reason,
                    diagnostics: damage_diagnostics,
                },
                Instant::now(),
            );
            wl_surface.commit();
            Ok::<(), anyhow::Error>(())
        })?;
        debug!("=== RENDER COMPLETE ===");

        // Render toolbar overlays if visible, only when state/hover changed.
        record_stage!(toolbar, {
            self.render_layer_toolbars_if_needed();
        });
        if let Some(breakdown) = render_breakdown {
            self.record_perf_render_breakdown(breakdown);
        }

        if self.capture_suppressed() {
            self.capture.mark_preflight_rendered();
        }
        Ok(keep_rendering)
    }

    fn render_force_full_damage_reason(&self) -> Option<FullDamageReason> {
        if self.zoom.active {
            Some(FullDamageReason::Zoom)
        } else if self.canvas_transform_active() {
            Some(FullDamageReason::BoardPan)
        } else {
            None
        }
    }
}

fn input_full_damage_reason(
    reason: Option<crate::draw::DirtyFullReason>,
) -> Option<FullDamageReason> {
    reason.map(|reason| match reason {
        crate::draw::DirtyFullReason::CanvasClear => FullDamageReason::CanvasClear,
        crate::draw::DirtyFullReason::FirstRunOnboarding => FullDamageReason::FirstRunOnboarding,
    })
}
