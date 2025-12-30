use super::*;
use crate::backend::wayland::toolbar_icons;

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
        let ui_animation_active = highlight_active || preset_feedback_active || ui_toast_active;
        self.update_ui_animation_tick(now, ui_animation_active);
        let keep_rendering = ui_animation_active && self.ui_animation_interval.is_none();
        let mut eraser_pattern: Option<cairo::SurfacePattern> = None;
        let mut eraser_bg_color: Option<Color> = None;

        // Get a buffer from the pool
        let (buffer, canvas) = {
            let pool = self.surface.ensure_pool(&self.shm, buffer_count)?;
            debug!("Requesting buffer from pool");
            let result = pool
                .create_buffer(
                    phys_width as i32,
                    phys_height as i32,
                    (phys_width * 4) as i32,
                    wl_shm::Format::Argb8888,
                )
                .context("Failed to create buffer")?;
            debug!("Buffer acquired from pool");
            result
        };

        // SAFETY: This unsafe block creates a Cairo surface from raw memory buffer.
        // Safety invariants that must be maintained:
        // 1. `canvas` is a valid mutable slice from SlotPool with exactly (width * height * 4) bytes
        // 2. The buffer format ARgb32 matches the allocation (4 bytes per pixel: alpha, red, green, blue)
        // 3. The stride (width * 4) correctly represents the number of bytes per row
        // 4. `cairo_surface` and `ctx` are explicitly dropped before the buffer is committed to Wayland,
        //    ensuring Cairo doesn't access memory after ownership transfers
        // 5. No other references to this memory exist during Cairo's usage
        // 6. The buffer remains valid throughout Cairo's usage (enforced by Rust's borrow checker)
        let cairo_surface = unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                canvas.as_mut_ptr(),
                cairo::Format::ARgb32,
                phys_width as i32,
                phys_height as i32,
                (phys_width * 4) as i32,
            )
            .context("Failed to create Cairo surface")?
        };

        // Render using Cairo
        let ctx = cairo::Context::new(&cairo_surface).context("Failed to create Cairo context")?;

        // Clear with fully transparent background
        debug!("Clearing background");
        ctx.set_operator(cairo::Operator::Clear);
        ctx.paint().context("Failed to clear background")?;
        ctx.set_operator(cairo::Operator::Over);

        if render_canvas {
            let allow_background_image =
                !(self.zoom.is_engaged() && board_mode != BoardMode::Transparent);
            let zoom_render_image = if self.zoom.active && allow_background_image {
                self.zoom.image().or_else(|| self.frozen.image())
            } else {
                None
            };
            let zoom_render_active = self.zoom.active && zoom_render_image.is_some();
            let zoom_transform_active = self.zoom.active;
            let background_image = if zoom_render_active {
                zoom_render_image
            } else if allow_background_image {
                self.frozen.image()
            } else {
                None
            };

            if let Some(image) = background_image {
                // SAFETY: we create a Cairo surface borrowing our owned buffer; it is dropped
                // before commit, and we hold the buffer alive via `image.data`.
                let surface = unsafe {
                    cairo::ImageSurface::create_for_data_unsafe(
                        image.data.as_ptr() as *mut u8,
                        cairo::Format::ARgb32,
                        image.width as i32,
                        image.height as i32,
                        image.stride,
                    )
                }
                .context("Failed to create frozen image surface")?;

                let scale_x = if image.width > 0 {
                    phys_width as f64 / image.width as f64
                } else {
                    1.0
                };
                let scale_y = if image.height > 0 {
                    phys_height as f64 / image.height as f64
                } else {
                    1.0
                };
                let _ = ctx.save();
                if zoom_render_active {
                    let scale_x_safe = scale_x.max(f64::MIN_POSITIVE);
                    let scale_y_safe = scale_y.max(f64::MIN_POSITIVE);
                    let offset_x = self.zoom.view_offset.0 * (scale as f64) / scale_x_safe;
                    let offset_y = self.zoom.view_offset.1 * (scale as f64) / scale_y_safe;
                    ctx.scale(scale_x * self.zoom.scale, scale_y * self.zoom.scale);
                    ctx.translate(-offset_x, -offset_y);
                } else if (scale_x - 1.0).abs() > f64::EPSILON
                    || (scale_y - 1.0).abs() > f64::EPSILON
                {
                    ctx.scale(scale_x, scale_y);
                }

                if let Err(err) = ctx.set_source_surface(&surface, 0.0, 0.0) {
                    warn!("Failed to set frozen background surface: {}", err);
                } else if let Err(err) = ctx.paint() {
                    warn!("Failed to paint frozen background: {}", err);
                }
                let _ = ctx.restore();

                let pattern = cairo::SurfacePattern::create(&surface);
                pattern.set_extend(cairo::Extend::Pad);
                let mut matrix = cairo::Matrix::identity();
                let scale_x_inv = 1.0 / (scale as f64 * scale_x.max(f64::MIN_POSITIVE));
                let scale_y_inv = 1.0 / (scale as f64 * scale_y.max(f64::MIN_POSITIVE));
                matrix.scale(scale_x_inv, scale_y_inv);
                pattern.set_matrix(matrix);
                eraser_pattern = Some(pattern);
            } else {
                // Render board background if in board mode (whiteboard/blackboard)
                crate::draw::render_board_background(
                    &ctx,
                    self.input_state.board_mode(),
                    &self.input_state.board_config,
                );
                eraser_bg_color = self
                    .input_state
                    .board_mode()
                    .background_color(&self.input_state.board_config);
            }

            // Scale subsequent drawing to logical coordinates
            let _ = ctx.save();
            if scale > 1 {
                ctx.scale(scale as f64, scale as f64);
            }

            if zoom_transform_active {
                let _ = ctx.save();
                ctx.scale(self.zoom.scale, self.zoom.scale);
                ctx.translate(-self.zoom.view_offset.0, -self.zoom.view_offset.1);
            }

            // Render all completed shapes from active frame
            debug!(
                "Rendering {} completed shapes",
                self.input_state.canvas_set.active_frame().shapes.len()
            );
            let eraser_ctx = crate::draw::EraserReplayContext {
                pattern: eraser_pattern.as_ref().map(|p| p as &cairo::Pattern),
                bg_color: eraser_bg_color,
            };
            crate::draw::render_shapes(
                &ctx,
                &self.input_state.canvas_set.active_frame().shapes,
                Some(&eraser_ctx),
            );

            // Render selection halo overlays
            if self.input_state.has_selection() {
                let selected: HashSet<_> = self
                    .input_state
                    .selected_shape_ids()
                    .iter()
                    .copied()
                    .collect();
                let frame = self.input_state.canvas_set.active_frame();
                for drawn in &frame.shapes {
                    if selected.contains(&drawn.id) {
                        crate::draw::render_selection_halo(&ctx, drawn);
                    }
                }
            }
            if matches!(
                self.input_state.state,
                DrawingState::Idle | DrawingState::ResizingText { .. }
            ) && let Some((_shape_id, handle)) = self.input_state.selected_text_resize_handle()
            {
                let _ = ctx.save();
                ctx.rectangle(
                    handle.x as f64,
                    handle.y as f64,
                    handle.width as f64,
                    handle.height as f64,
                );
                ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
                let _ = ctx.fill_preserve();
                ctx.set_source_rgba(0.2, 0.45, 1.0, 0.9);
                ctx.set_line_width(1.5);
                let _ = ctx.stroke();
                let _ = ctx.restore();
            }

            let (mx, my) = if zoom_transform_active {
                self.zoomed_world_coords(
                    self.current_mouse().0 as f64,
                    self.current_mouse().1 as f64,
                )
            } else {
                self.current_mouse()
            };

            let eraser_drawing = matches!(
                self.input_state.state,
                DrawingState::Drawing {
                    tool: Tool::Eraser,
                    ..
                }
            );
            let eraser_stroke = self.input_state.eraser_mode == EraserMode::Stroke;
            let eraser_hover = eraser_stroke
                && !eraser_drawing
                && self.input_state.active_tool() == Tool::Eraser
                && matches!(self.input_state.state, DrawingState::Idle)
                && self.has_pointer_focus()
                && !self.pointer_over_toolbar();
            if eraser_stroke && (eraser_drawing || eraser_hover) {
                self.input_state.ensure_spatial_index_for_active_frame();
                let ids = if eraser_drawing {
                    if let DrawingState::Drawing {
                        tool: Tool::Eraser,
                        points,
                        ..
                    } = &self.input_state.state
                    {
                        self.input_state.hit_test_all_for_points_cached(
                            points,
                            self.input_state.eraser_hit_radius(),
                        )
                    } else {
                        Vec::new()
                    }
                } else {
                    let point = [(mx, my)];
                    self.input_state.hit_test_all_for_points_cached(
                        &point,
                        self.input_state.eraser_hit_radius(),
                    )
                };
                if !ids.is_empty() {
                    let frame = self.input_state.canvas_set.active_frame();
                    match ids.len() {
                        1 => {
                            if let Some(drawn) = frame.shape(ids[0]) {
                                crate::draw::render_selection_halo(&ctx, drawn);
                            }
                        }
                        2..=4 => {
                            let mut indices = Vec::with_capacity(ids.len());
                            for id in &ids {
                                if let Some(index) = frame.find_index(*id) {
                                    indices.push(index);
                                }
                            }
                            if !indices.is_empty() {
                                indices.sort_unstable();
                                indices.dedup();
                                for index in indices {
                                    if let Some(drawn) = frame.shapes.get(index) {
                                        crate::draw::render_selection_halo(&ctx, drawn);
                                    }
                                }
                            }
                        }
                        _ => {
                            let hover_ids: HashSet<_> = ids.into_iter().collect();
                            for drawn in &frame.shapes {
                                if hover_ids.contains(&drawn.id) {
                                    crate::draw::render_selection_halo(&ctx, drawn);
                                }
                            }
                        }
                    }
                }
            }

            // Render provisional shape if actively drawing
            // Use optimized method that avoids cloning for freehand
            if self.input_state.render_provisional_shape(&ctx, mx, my) {
                debug!("Rendered provisional shape");
            }

            // Render text cursor/buffer if in text mode
            if let DrawingState::TextInput { x, y, buffer } = &self.input_state.state {
                let preview_text = if buffer.is_empty() {
                    "_".to_string() // Show cursor when buffer is empty
                } else {
                    format!("{}_", buffer)
                };
                match self.input_state.text_input_mode {
                    crate::input::TextInputMode::Plain => {
                        crate::draw::render_text(
                            &ctx,
                            *x,
                            *y,
                            &preview_text,
                            self.input_state.current_color,
                            self.input_state.current_font_size,
                            &self.input_state.font_descriptor,
                            self.input_state.text_background_enabled,
                            self.input_state.text_wrap_width,
                        );
                    }
                    crate::input::TextInputMode::StickyNote => {
                        crate::draw::render_sticky_note(
                            &ctx,
                            *x,
                            *y,
                            &preview_text,
                            self.input_state.current_color,
                            self.input_state.current_font_size,
                            &self.input_state.font_descriptor,
                            self.input_state.text_wrap_width,
                        );
                    }
                }
            }

            // Render click highlight overlays before UI so status/help remain legible
            self.input_state.render_click_highlights(&ctx, now);

            if zoom_transform_active {
                let _ = ctx.restore();
            }

            if render_ui {
                if self.input_state.show_tool_preview
                    && self.has_pointer_focus()
                    && !self.pointer_over_toolbar()
                    && matches!(
                        self.input_state.state,
                        DrawingState::Idle | DrawingState::PendingTextClick { .. }
                    )
                {
                    let (cursor_x, cursor_y) = self.current_mouse();
                    draw_tool_preview(
                        &ctx,
                        self.input_state.active_tool(),
                        self.input_state.current_color,
                        cursor_x as f64,
                        cursor_y as f64,
                        width as f64,
                        height as f64,
                    );
                }
                // Render frozen badge even if status bar is hidden
                if self.input_state.frozen_active()
                    && !self.zoom.active
                    && self.config.ui.show_frozen_badge
                {
                    crate::ui::render_frozen_badge(&ctx, width, height);
                }
                // Render a zoom badge when the status bar is hidden or zoom is locked.
                if self.input_state.zoom_active()
                    && (!self.input_state.show_status_bar || self.input_state.zoom_locked())
                {
                    crate::ui::render_zoom_badge(
                        &ctx,
                        width,
                        height,
                        self.input_state.zoom_scale(),
                        self.input_state.zoom_locked(),
                    );
                }

                // Render status bar if enabled
                if self.input_state.show_status_bar {
                    crate::ui::render_status_bar(
                        &ctx,
                        &self.input_state,
                        self.config.ui.status_bar_position,
                        &self.config.ui.status_bar_style,
                        width,
                        height,
                    );
                }

                // Render help overlay if toggled
                if self.input_state.show_help {
                    crate::ui::render_help_overlay(
                        &ctx,
                        &self.config.ui.help_overlay_style,
                        width,
                        height,
                        self.frozen_enabled(),
                    );
                }

                crate::ui::render_ui_toast(&ctx, &self.input_state, width, height);
                crate::ui::render_preset_toast(&ctx, &self.input_state, width, height);

                if !self.zoom.active {
                    if self.input_state.is_properties_panel_open() {
                        self.input_state
                            .update_properties_panel_layout(&ctx, width, height);
                    } else {
                        self.input_state.clear_properties_panel_layout();
                    }
                    crate::ui::render_properties_panel(&ctx, &self.input_state, width, height);

                    if self.input_state.is_context_menu_open() {
                        self.input_state
                            .update_context_menu_layout(&ctx, width, height);
                    } else {
                        self.input_state.clear_context_menu_layout();
                    }

                    // Render context menu if open
                    crate::ui::render_context_menu(&ctx, &self.input_state, width, height);
                } else {
                    self.input_state.clear_context_menu_layout();
                    self.input_state.clear_properties_panel_layout();
                }

                // Inline toolbars (xdg fallback) render directly into main surface when layer-shell is unavailable.
                if self.toolbar.is_visible() && self.inline_toolbars_active() {
                    let snapshot = self.toolbar_snapshot();
                    if self.toolbar.update_snapshot(&snapshot) {
                        self.toolbar.mark_dirty();
                    }
                    self.render_inline_toolbars(&ctx, &snapshot);
                }
            } else {
                self.input_state.clear_context_menu_layout();
            }

            let _ = ctx.restore();
        }

        // Flush Cairo
        debug!("Flushing Cairo surface");
        cairo_surface.flush();
        drop(ctx);
        drop(cairo_surface);

        // Attach buffer and commit
        debug!("Attaching buffer and committing surface");
        let wl_surface = self
            .surface
            .wl_surface()
            .cloned()
            .context("Surface not created")?;
        wl_surface.set_buffer_scale(scale);
        wl_surface.attach(Some(buffer.wl_buffer()), 0, 0);

        // Capture damage hints for diagnostics. We still apply full damage below to avoid missed
        // redraws, but logging the computed regions helps pinpoint under-reporting issues.
        let logical_damage = resolve_damage_regions(
            self.surface.width().min(i32::MAX as u32) as i32,
            self.surface.height().min(i32::MAX as u32) as i32,
            self.input_state.take_dirty_regions(),
        );
        if debug_damage_logging_enabled() {
            let scaled_damage = scale_damage_regions(logical_damage.clone(), scale);
            debug!(
                "Damage hints (scaled): count={}, {}",
                scaled_damage.len(),
                damage_summary(&scaled_damage)
            );
        }

        // Prefer correctness over micro-optimizations: full damage avoids cases where incomplete
        // hints result in stale pixels (reported as disappearing/reappearing strokes). If we ever
        // return to partial damage, implement per-buffer damage tracking instead of draining a
        // single accumulator.
        wl_surface.damage_buffer(0, 0, phys_width as i32, phys_height as i32);

        let force_frame_callback = self.frozen.preflight_pending() || self.zoom.preflight_pending();
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

        Ok(keep_rendering)
    }
}

fn draw_tool_preview(
    ctx: &cairo::Context,
    tool: Tool,
    color: Color,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    let icon_size = 18.0;
    let pad = 6.0;
    let bubble = icon_size + pad * 2.0;
    let mut bx = x + 16.0;
    let mut by = y + 16.0;
    let max_x = (w - bubble - 4.0).max(4.0);
    let max_y = (h - bubble - 4.0).max(4.0);
    if bx < 4.0 {
        bx = 4.0;
    } else if bx > max_x {
        bx = max_x;
    }
    if by < 4.0 {
        by = 4.0;
    } else if by > max_y {
        by = max_y;
    }

    let cx = bx + bubble / 2.0;
    let cy = by + bubble / 2.0;
    let radius = bubble / 2.0;
    let _ = ctx.save();
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.35);
    ctx.arc(cx + 1.0, cy + 1.5, radius, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();
    ctx.set_source_rgba(0.08, 0.08, 0.1, 0.6);
    ctx.arc(cx, cy, radius, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    let (r, g, b, a) = match tool {
        Tool::Eraser | Tool::Select => (0.95, 0.95, 0.98, 0.95),
        _ => (color.r, color.g, color.b, 0.95),
    };
    ctx.set_source_rgba(r, g, b, a);
    let icon_x = bx + pad;
    let icon_y = by + pad;
    match tool {
        Tool::Select => toolbar_icons::draw_icon_select(ctx, icon_x, icon_y, icon_size),
        Tool::Pen => toolbar_icons::draw_icon_pen(ctx, icon_x, icon_y, icon_size),
        Tool::Line => toolbar_icons::draw_icon_line(ctx, icon_x, icon_y, icon_size),
        Tool::Rect => toolbar_icons::draw_icon_rect(ctx, icon_x, icon_y, icon_size),
        Tool::Ellipse => toolbar_icons::draw_icon_circle(ctx, icon_x, icon_y, icon_size),
        Tool::Arrow => toolbar_icons::draw_icon_arrow(ctx, icon_x, icon_y, icon_size),
        Tool::Marker => toolbar_icons::draw_icon_marker(ctx, icon_x, icon_y, icon_size),
        Tool::Highlight => toolbar_icons::draw_icon_highlight(ctx, icon_x, icon_y, icon_size),
        Tool::Eraser => toolbar_icons::draw_icon_eraser(ctx, icon_x, icon_y, icon_size),
    }
    let _ = ctx.restore();
}
