use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn render_inline_toolbars(
        &mut self,
        ctx: &cairo::Context,
        snapshot: &ToolbarSnapshot,
    ) {
        if !self.inline_toolbars_render_active() || !self.toolbar.is_top_visible() {
            self.toolbar_chrome.clear_inline_hits();
            self.toolbar_chrome.clear_inline_hover();
            return;
        }

        let focus_hover = self.inline_toolbar_focus_hover();
        self.toolbar_chrome.clear_inline_hits();
        self.clamp_toolbar_offsets(snapshot);
        let ui_scale = if snapshot.toolbar_scale.is_finite() {
            snapshot.toolbar_scale.clamp(0.5, 3.0)
        } else {
            1.0
        };

        let authored_offset = self.toolbar_chrome.top_offset();
        let top_offset = (
            self.inline_top_base_x() + authored_offset.0,
            self.inline_top_base_y() + authored_offset.1,
        );

        let top_size = top_size(snapshot);
        let top_base_w = top_size.0 as f64 / ui_scale;
        let top_base_h = top_size.1 as f64 / ui_scale;
        let top_hover_local = self
            .toolbar_chrome
            .inline_hover()
            .or(focus_hover)
            .map(|(x, y)| (x - top_offset.0, y - top_offset.1))
            .map(|(x, y)| (x / ui_scale, y / ui_scale));
        let _ = ctx.save();
        ctx.translate(top_offset.0, top_offset.1);
        if (ui_scale - 1.0).abs() > f64::EPSILON {
            ctx.scale(ui_scale, ui_scale);
        }
        let mut hits = Vec::new();
        if let Err(err) = render_top_strip(
            ctx,
            top_base_w,
            top_base_h,
            snapshot,
            &mut hits,
            top_hover_local,
            self.toolbar_chrome.inline_hover_start(),
        ) {
            log::warn!("Failed to render inline top toolbar: {}", err);
        }
        let _ = ctx.restore();
        for hit in &mut hits {
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
        crate::backend::wayland::toolbar::hit::clip_hit_regions_to_bounds(&mut hits, 0, top_rect);
        self.toolbar_chrome.set_inline_rendered(hits, top_rect);
    }
}
