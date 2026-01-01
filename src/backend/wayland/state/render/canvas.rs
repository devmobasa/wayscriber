use super::super::*;

impl WaylandState {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_canvas_layer(
        &mut self,
        ctx: &cairo::Context,
        width: u32,
        height: u32,
        scale: i32,
        phys_width: u32,
        phys_height: u32,
        render_ui: bool,
        now: Instant,
    ) -> Result<()> {
        let board_mode = self.input_state.board_mode();
        let mut eraser_pattern: Option<cairo::SurfacePattern> = None;
        let mut eraser_bg_color: Option<Color> = None;

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
            } else if (scale_x - 1.0).abs() > f64::EPSILON || (scale_y - 1.0).abs() > f64::EPSILON {
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
                ctx,
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
            ctx,
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
                    crate::draw::render_selection_halo(ctx, drawn);
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
            self.zoomed_world_coords(self.current_mouse().0 as f64, self.current_mouse().1 as f64)
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
                self.input_state
                    .hit_test_all_for_points_cached(&point, self.input_state.eraser_hit_radius())
            };
            if !ids.is_empty() {
                let frame = self.input_state.canvas_set.active_frame();
                match ids.len() {
                    1 => {
                        if let Some(drawn) = frame.shape(ids[0]) {
                            crate::draw::render_selection_halo(ctx, drawn);
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
                                    crate::draw::render_selection_halo(ctx, drawn);
                                }
                            }
                        }
                    }
                    _ => {
                        let hover_ids: HashSet<_> = ids.into_iter().collect();
                        for drawn in &frame.shapes {
                            if hover_ids.contains(&drawn.id) {
                                crate::draw::render_selection_halo(ctx, drawn);
                            }
                        }
                    }
                }
            }
        }

        // Render provisional shape if actively drawing
        // Use optimized method that avoids cloning for freehand
        if self.input_state.render_provisional_shape(ctx, mx, my) {
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
                        ctx,
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
                        ctx,
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
        self.input_state.render_click_highlights(ctx, now);

        if zoom_transform_active {
            let _ = ctx.restore();
        }

        self.render_ui_layers(ctx, width, height, render_ui);

        let _ = ctx.restore();

        Ok(())
    }
}
