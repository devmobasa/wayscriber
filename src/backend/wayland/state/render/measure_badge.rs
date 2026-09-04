use super::super::*;
use crate::ui::theme;
use crate::ui_text::text_layout;

impl WaylandState {
    pub(super) fn shape_measure_badge_visual(
        &self,
        width: u32,
        height: u32,
    ) -> Option<crate::ui::ShapeMeasureBadge> {
        let pointer = self.pointer.position();
        let world = self.canvas_world_coords(pointer.0 as f64, pointer.1 as f64);
        let size = self.input_state.provisional_shape_size(world.0, world.1)?;
        crate::ui::measure_shape_badge(
            self.config.ui.show_shape_size_readout,
            size,
            (pointer.0 as f64, pointer.1 as f64),
            width,
            height,
        )
    }

    pub(super) fn render_shape_measure_badge(&self, ctx: &cairo::Context, width: u32, height: u32) {
        let Some(badge) = self.shape_measure_badge_visual(width, height) else {
            return;
        };
        let (x, y, badge_width, badge_height) = badge.bounds;
        let _ = ctx.save();

        crate::ui::draw_pill(
            ctx,
            x,
            y,
            badge_width,
            badge_height,
            6.0,
            (12.0 / 255.0, 12.0 / 255.0, 15.0 / 255.0, 0.92),
            (1.0, 1.0, 1.0, 0.16),
            None,
        );

        theme::set_color(ctx, (1.0, 1.0, 1.0, 1.0));
        ctx.rectangle(x, y, badge_width, badge_height);
        ctx.clip();
        text_layout(
            ctx,
            crate::ui::shape_measure_badge_text_style(),
            &badge.text,
            None,
        )
        .show_at_baseline(ctx, badge.baseline.0, badge.baseline.1);

        let _ = ctx.restore();
    }
}
