use super::*;
use crate::backend::wayland::toolbar::events::HitKind;

impl WaylandState {
    pub(in crate::backend::wayland) fn render_inline_toolbars(
        &mut self,
        ctx: &cairo::Context,
        snapshot: &ToolbarSnapshot,
    ) {
        if !self.inline_toolbars_render_active() || !self.toolbar.is_visible() {
            self.clear_inline_toolbar_hits();
            self.clear_inline_toolbar_hover();
            return;
        }

        let top_visible = self.toolbar.is_top_visible();
        let side_visible = self.toolbar.is_side_visible();

        if !top_visible {
            self.data.inline_top_hover = None;
            self.data.inline_top_focus_index = None;
        }
        if !side_visible {
            self.data.inline_side_hover = None;
            self.data.inline_side_focus_index = None;
        }

        if !top_visible && !side_visible {
            self.clear_inline_toolbar_hits();
            self.clear_inline_toolbar_hover();
            return;
        }

        let top_focus_hover = if top_visible {
            self.inline_toolbar_focus_hover(ToolbarFocusTarget::Top)
        } else {
            None
        };
        let side_focus_hover = if side_visible {
            self.inline_toolbar_focus_hover(ToolbarFocusTarget::Side)
        } else {
            None
        };
        self.clear_inline_toolbar_hits();
        self.clamp_toolbar_offsets(snapshot);

        // Position inline toolbars with padding and keep top bar to the right of the side bar.
        let side_offset = (
            Self::INLINE_SIDE_X + self.data.toolbar_side_offset_x,
            Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset,
        );

        let top_base_x = self.inline_top_base_x(snapshot);
        let top_offset = (
            top_base_x + self.data.toolbar_top_offset,
            self.inline_top_base_y() + self.data.toolbar_top_offset_y,
        );

        // Top toolbar
        if top_visible {
            let top_size = top_size(snapshot);
            let top_hover_local = self
                .data
                .inline_top_hover
                .or(top_focus_hover)
                .map(|(x, y)| (x - top_offset.0, y - top_offset.1));
            let _ = ctx.save();
            ctx.translate(top_offset.0, top_offset.1);
            if let Err(err) = render_top_strip(
                ctx,
                top_size.0 as f64,
                top_size.1 as f64,
                snapshot,
                &mut self.data.inline_top_hits,
                top_hover_local,
                self.data.inline_top_hover_start,
            ) {
                log::warn!("Failed to render inline top toolbar: {}", err);
            }
            let _ = ctx.restore();
            for hit in &mut self.data.inline_top_hits {
                hit.rect.0 += top_offset.0;
                hit.rect.1 += top_offset.1;
                if let HitKind::PickColor { x, y, .. } = &mut hit.kind {
                    *x += top_offset.0;
                    *y += top_offset.1;
                }
            }
            self.data.inline_top_rect = Some((
                top_offset.0,
                top_offset.1,
                top_size.0 as f64,
                top_size.1 as f64,
            ));
        } else {
            self.data.inline_top_rect = None;
        }

        // Side toolbar
        if side_visible {
            let side_size = side_size(snapshot);
            let side_hover_local = self
                .data
                .inline_side_hover
                .or(side_focus_hover)
                .map(|(x, y)| (x - side_offset.0, y - side_offset.1));
            let _ = ctx.save();
            ctx.translate(side_offset.0, side_offset.1);
            if let Err(err) = render_side_palette(
                ctx,
                side_size.0 as f64,
                side_size.1 as f64,
                snapshot,
                &mut self.data.inline_side_hits,
                side_hover_local,
                self.data.inline_side_hover_start,
            ) {
                log::warn!("Failed to render inline side toolbar: {}", err);
            }
            let _ = ctx.restore();
            for hit in &mut self.data.inline_side_hits {
                hit.rect.0 += side_offset.0;
                hit.rect.1 += side_offset.1;
                if let HitKind::PickColor { x, y, .. } = &mut hit.kind {
                    *x += side_offset.0;
                    *y += side_offset.1;
                }
            }
            self.data.inline_side_rect = Some((
                side_offset.0,
                side_offset.1,
                side_size.0 as f64,
                side_size.1 as f64,
            ));
        } else {
            self.data.inline_side_rect = None;
        }
    }
}
