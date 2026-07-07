mod background;
mod overlays;
mod text;

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
        now: Instant,
        damage_world: &[crate::util::Rect],
        mut perf: Option<&mut PerfRenderBreakdown>,
    ) -> Result<()> {
        let canvas_transform_active = self.canvas_transform_active();
        let (canvas_origin_x, canvas_origin_y) = self.canvas_view_origin();
        let shapes_total = self.input_state.boards.active_frame().shapes.len();

        // For pure pan transforms, serve the board background and committed
        // shapes from the baked layer cache: pan frames force full damage, so
        // this turns an O(shapes) Cairo replay into a single aligned blit.
        let layer_cache_start = perf.as_ref().map(|_| Instant::now());
        let layer_cache_ready = if self.canvas_layer_cache_usable() {
            self.ensure_canvas_layer_cache(width, height, scale)
        } else {
            self.canvas_layer_cache.clear();
            false
        };
        if let (Some(perf), Some(layer_cache_start)) = (perf.as_mut(), layer_cache_start) {
            perf.stages.completed_shapes = perf
                .stages
                .completed_shapes
                .saturating_add(Instant::now().saturating_duration_since(layer_cache_start));
        }

        let background_start = perf.as_ref().map(|_| Instant::now());
        let eraser_ctx = self.render_canvas_background(ctx, scale, phys_width, phys_height)?;
        if let (Some(perf), Some(background_start)) = (perf.as_mut(), background_start) {
            perf.stages.background = perf
                .stages
                .background
                .saturating_add(Instant::now().saturating_duration_since(background_start));
        }

        // Scale subsequent drawing to logical coordinates
        let _ = ctx.save();
        if scale > 1 {
            ctx.scale(scale as f64, scale as f64);
        }

        if canvas_transform_active {
            let _ = ctx.save();
            if self.zoom.active {
                ctx.scale(self.zoom.scale, self.zoom.scale);
            }
            ctx.translate(-canvas_origin_x, -canvas_origin_y);
        }

        let replay_ctx = eraser_ctx.replay_context();

        let completed_shapes_start = perf.as_ref().map(|_| Instant::now());
        if layer_cache_ready && self.canvas_layer_cache.blit(ctx) {
            // Board background and committed shapes came from the baked layer.
            debug!("Rendered committed shapes from layer cache");
            if let Some(perf) = perf.as_mut() {
                perf.shapes_total = shapes_total;
                perf.canvas_layer_cache_hit = true;
            }
        } else {
            // Render all completed shapes from active frame
            debug!("Rendering {} completed shapes", shapes_total);
            let shapes = &self.input_state.boards.active_frame().shapes;
            if let Some(perf) = perf.as_mut() {
                perf.shapes_total = shapes.len();
            }

            // Manual Culling: Only render shapes that intersect with the damage regions.
            // Cairo's internal clipping is efficient for rasterization, but sending
            // thousands of shapes to Cairo still incurs overhead for geometry processing.
            // A simple bounding box check here eliminates that overhead.
            let render_drawn_shape = |drawn_shape: &crate::draw::DrawnShape| {
                super::super::canvas_layer::render_committed_shape(ctx, drawn_shape, &replay_ctx)
            };

            // Compute bounding box of all damage regions for fast rejection
            // (Union of all dirty rects). These bounds are in world coordinates.
            let damage_bounds =
                damage_world
                    .iter()
                    .fold(None, |acc: Option<crate::util::Rect>, r| match acc {
                        None => Some(*r),
                        Some(u) => {
                            // Manual union to avoid extra allocations.
                            let min_x = u.x.min(r.x);
                            let min_y = u.y.min(r.y);
                            let max_x =
                                u.x.saturating_add(u.width).max(r.x.saturating_add(r.width));
                            let max_y =
                                u.y.saturating_add(u.height)
                                    .max(r.y.saturating_add(r.height));
                            Some(crate::util::Rect {
                                x: min_x,
                                y: min_y,
                                width: max_x - min_x,
                                height: max_y - min_y,
                            })
                        }
                    });

            if let Some(bounds) = damage_bounds {
                // Expand bounds slightly to account for line width/glow that might extend outside
                // the logical shape bounds (though Shape::bounding_box should theoretically cover it,
                // safety margin is good).
                let margin = 2;
                let safe_x = bounds.x.saturating_sub(margin);
                let safe_y = bounds.y.saturating_sub(margin);
                let safe_width = bounds.width.saturating_add(margin * 2);
                let safe_height = bounds.height.saturating_add(margin * 2);
                let safe_bounds = if canvas_transform_active {
                    crate::util::Rect::new(safe_x, safe_y, safe_width, safe_height)
                } else {
                    // Clamp to logical surface bounds to avoid negative coords or overflow.
                    let logical_width = width as i32;
                    let logical_height = height as i32;
                    let clamped_x = safe_x.max(0);
                    let clamped_y = safe_y.max(0);
                    let max_width = logical_width.saturating_sub(clamped_x);
                    let max_height = logical_height.saturating_sub(clamped_y);
                    crate::util::Rect::new(
                        clamped_x,
                        clamped_y,
                        safe_width.min(max_width),
                        safe_height.min(max_height),
                    )
                };

                if let Some(safe_bounds) = safe_bounds {
                    let mut shapes_tested = 0usize;
                    let mut shapes_rendered = 0usize;
                    for drawn_shape in shapes {
                        shapes_tested += 1;
                        // If shape has no bounding box (e.g. empty freehand), skip it.
                        // If it has one, check intersection. Uses the per-shape
                        // memoized bounds to avoid O(points) recomputation per frame.
                        if let Some(bbox) = drawn_shape.bounding_box() {
                            // Check intersection:
                            // !(bbox.left > safe.right || bbox.right < safe.left || ...)
                            let bbox_right = bbox.x.saturating_add(bbox.width);
                            let bbox_bottom = bbox.y.saturating_add(bbox.height);
                            let safe_right = safe_bounds.x.saturating_add(safe_bounds.width);
                            let safe_bottom = safe_bounds.y.saturating_add(safe_bounds.height);

                            let intersects = !(bbox.x >= safe_right
                                || bbox_right <= safe_bounds.x
                                || bbox.y >= safe_bottom
                                || bbox_bottom <= safe_bounds.y);

                            if intersects {
                                render_drawn_shape(drawn_shape);
                                shapes_rendered += 1;
                            }
                        }
                    }
                    if let Some(perf) = perf.as_mut() {
                        perf.shapes_tested = shapes_tested;
                        perf.shapes_rendered = shapes_rendered;
                    }
                }
            } else {
                // If we don't have damage bounds, render everything to stay correct.
                let mut shapes_rendered = 0usize;
                for drawn_shape in shapes {
                    render_drawn_shape(drawn_shape);
                    shapes_rendered += 1;
                }
                if let Some(perf) = perf.as_mut() {
                    perf.shapes_tested = shapes.len();
                    perf.shapes_rendered = shapes_rendered;
                }
            }
        }
        if let (Some(perf), Some(completed_shapes_start)) = (perf.as_mut(), completed_shapes_start)
        {
            perf.stages.completed_shapes = perf
                .stages
                .completed_shapes
                .saturating_add(Instant::now().saturating_duration_since(completed_shapes_start));
        }

        self.render_selection_overlays(ctx);

        let (mx, my) =
            self.canvas_world_coords(self.current_mouse().0 as f64, self.current_mouse().1 as f64);
        let (hover_mx, hover_my) = self
            .stylus_hover_cursor_position()
            .map(|(x, y)| self.canvas_world_coords(x, y))
            .unwrap_or((mx, my));

        self.render_eraser_hover_halos(ctx, hover_mx, hover_my);

        let provisional = self.input_state.provisional_tool_stroke(mx, my);
        let provisional_points = provisional_point_count(&provisional);
        let provisional_start = perf.as_ref().map(|_| Instant::now());
        let rendered_provisional = match provisional {
            crate::input::tool::ProvisionalToolStroke::BlurReplayPreview(params) => {
                crate::draw::render_blur_rect(ctx, params, &replay_ctx);
                true
            }
            _ => self
                .input_state
                .render_provisional_shape_for_damage(ctx, mx, my, damage_world),
        };
        if let (Some(perf), Some(provisional_start)) = (perf.as_mut(), provisional_start) {
            perf.provisional_points = provisional_points;
            perf.stages.provisional = perf
                .stages
                .provisional
                .saturating_add(Instant::now().saturating_duration_since(provisional_start));
        }
        if rendered_provisional {
            debug!("Rendered provisional shape");
        }

        // Render text cursor/buffer if in text mode
        self.render_text_input_preview(ctx);

        self.input_state.render_highlight_tool_ring(ctx, mx, my);

        // Render click highlight overlays before UI so status/help remain legible
        self.input_state.render_click_highlights(ctx, now);

        if canvas_transform_active {
            let _ = ctx.restore();
        }

        let _ = ctx.restore();

        Ok(())
    }
}

fn provisional_point_count(stroke: &crate::input::tool::ProvisionalToolStroke<'_>) -> usize {
    match stroke {
        crate::input::tool::ProvisionalToolStroke::BorrowedFreehand { points, .. }
        | crate::input::tool::ProvisionalToolStroke::BorrowedPressureFreehand { points, .. }
        | crate::input::tool::ProvisionalToolStroke::BorrowedMarker { points, .. }
        | crate::input::tool::ProvisionalToolStroke::EraserPreview { points, .. } => points.len(),
        crate::input::tool::ProvisionalToolStroke::Shape(_)
        | crate::input::tool::ProvisionalToolStroke::BlurReplayPreview(_)
        | crate::input::tool::ProvisionalToolStroke::None => 0,
    }
}
