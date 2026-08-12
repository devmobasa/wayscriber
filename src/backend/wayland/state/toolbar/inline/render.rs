use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn render_inline_toolbars(
        &mut self,
        ctx: &cairo::Context,
        snapshot: &ToolbarSnapshot,
    ) {
        if !self.inline_toolbars_render_active() || !self.toolbar.is_top_visible() {
            self.clear_inline_toolbar_hits();
            self.clear_inline_toolbar_hover();
            return;
        }

        let focus_hover = self.inline_toolbar_focus_hover();
        self.clear_inline_toolbar_hits();
        self.clamp_toolbar_offsets(snapshot);
        let ui_scale = if snapshot.toolbar_scale.is_finite() {
            snapshot.toolbar_scale.clamp(0.5, 3.0)
        } else {
            1.0
        };

        let top_offset = (
            self.inline_top_base_x() + self.data.toolbar_top_offset,
            self.inline_top_base_y() + self.data.toolbar_top_offset_y,
        );

        let top_size = top_size(snapshot);
        let top_base_w = top_size.0 as f64 / ui_scale;
        let top_base_h = top_size.1 as f64 / ui_scale;
        let top_hover_local = self
            .data
            .inline_top_hover
            .or(focus_hover)
            .map(|(x, y)| (x - top_offset.0, y - top_offset.1))
            .map(|(x, y)| (x / ui_scale, y / ui_scale));
        let _ = ctx.save();
        ctx.translate(top_offset.0, top_offset.1);
        if (ui_scale - 1.0).abs() > f64::EPSILON {
            ctx.scale(ui_scale, ui_scale);
        }
        if let Err(err) = render_top_strip(
            ctx,
            top_base_w,
            top_base_h,
            snapshot,
            &mut self.data.inline_top_hits,
            top_hover_local,
            self.data.inline_top_hover_start,
        ) {
            log::warn!("Failed to render inline top toolbar: {}", err);
        }
        let _ = ctx.restore();
        for hit in &mut self.data.inline_top_hits {
            hit.rect.0 = hit.rect.0 * ui_scale + top_offset.0;
            hit.rect.1 = hit.rect.1 * ui_scale + top_offset.1;
            hit.rect.2 *= ui_scale;
            hit.rect.3 *= ui_scale;
        }
        let top_rect = (
            top_offset.0,
            top_offset.1,
            top_size.0 as f64,
            top_size.1 as f64,
        );
        self.data.inline_top_rect = Some(top_rect);
        crate::backend::wayland::toolbar::hit::clip_hit_regions_to_bounds(
            &mut self.data.inline_top_hits,
            0,
            top_rect,
        );
    }
}
