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
            self.input_state.boards.active_frame().shapes.len()
        );
        let replay_ctx = eraser_ctx.replay_context();
        crate::draw::render_shapes(
            ctx,
            &self.input_state.boards.active_frame().shapes,
            Some(&replay_ctx),
        );

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
