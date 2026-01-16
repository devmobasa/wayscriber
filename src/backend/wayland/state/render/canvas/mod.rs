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
        render_ui: bool,
        now: Instant,
        damage: &[crate::util::Rect],
    ) -> Result<()> {
        let zoom_transform_active = self.zoom.active;
        let eraser_ctx = self.render_canvas_background(ctx, scale, phys_width, phys_height)?;

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
        let shapes = &self.input_state.canvas_set.active_frame().shapes;
        let replay_ctx = eraser_ctx.replay_context();
        
        // Manual Culling: Only render shapes that intersect with the damage regions.
        // Cairo's internal clipping is efficient for rasterization, but sending
        // thousands of shapes to Cairo still incurs overhead for geometry processing.
        // A simple bounding box check here eliminates that overhead.
        
        // Compute bounding box of all damage regions for fast rejection
        // (Union of all dirty rects)
        let damage_bounds = damage.iter().fold(None, |acc: Option<crate::util::Rect>, r| {
            match acc {
                None => Some(*r),
                Some(u) => {
                    // Manual union since Rect doesn't have it exposed directly in this scope maybe?
                    // Let's implement min/max logic
                    let min_x = u.x.min(r.x);
                    let min_y = u.y.min(r.y);
                    let max_x = (u.x + u.width).max(r.x + r.width);
                    let max_y = (u.y + u.height).max(r.y + r.height);
                    Some(crate::util::Rect {
                        x: min_x,
                        y: min_y,
                        width: max_x - min_x,
                        height: max_y - min_y,
                    })
                }
            }
        });

        if let Some(bounds) = damage_bounds {
            // Expand bounds slightly to account for line width/glow that might extend outside
            // the logical shape bounds (though Shape::bounding_box should theoretically cover it,
            // safety margin is good).
            // Clamp to logical surface bounds to avoid negative coords or overflow.
            let margin = 2;
            let logical_width = width as i32;
            let logical_height = height as i32;
            let safe_x = (bounds.x - margin).max(0);
            let safe_y = (bounds.y - margin).max(0);
            let safe_bounds = crate::util::Rect {
                x: safe_x,
                y: safe_y,
                width: (bounds.width + 2 * margin).min(logical_width - safe_x),
                height: (bounds.height + 2 * margin).min(logical_height - safe_y),
            };

            for drawn_shape in shapes {
                // If shape has no bounding box (e.g. empty freehand), skip it.
                // If it has one, check intersection.
                if let Some(bbox) = drawn_shape.shape.bounding_box() {
                    // Check intersection:
                    // !(bbox.left > safe.right || bbox.right < safe.left || ...)
                    let bbox_right = bbox.x + bbox.width;
                    let bbox_bottom = bbox.y + bbox.height;
                    let safe_right = safe_bounds.x + safe_bounds.width;
                    let safe_bottom = safe_bounds.y + safe_bounds.height;

                    let intersects = !(bbox.x >= safe_right || bbox_right <= safe_bounds.x || 
                                     bbox.y >= safe_bottom || bbox_bottom <= safe_bounds.y);
                    
                    if intersects {
                        // We must handle eraser strokes specially here, just like render_shapes does,
                        // because render_shape() ignores them.
                        match &drawn_shape.shape {
                            crate::draw::Shape::EraserStroke { points, brush } => {
                                crate::draw::render_eraser_stroke(ctx, points, brush, &replay_ctx);
                            }
                            other => {
                                crate::draw::render_shape(ctx, other);
                            }
                        }
                    }
                }
            }
        } else {
            // If damage is empty, nothing to draw.
        }

        self.render_selection_overlays(ctx);

        let (mx, my) = if zoom_transform_active {
            self.zoomed_world_coords(self.current_mouse().0 as f64, self.current_mouse().1 as f64)
        } else {
            self.current_mouse()
        };

        self.render_eraser_hover_halos(ctx, mx, my);

        // Render provisional shape if actively drawing
        // Use optimized method that avoids cloning for freehand
        if self.input_state.render_provisional_shape(ctx, mx, my) {
            debug!("Rendered provisional shape");
        }

        // Render text cursor/buffer if in text mode
        self.render_text_input_preview(ctx);

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
