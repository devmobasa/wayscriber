use super::*;

impl WaylandState {
    pub(in crate::backend::wayland::state::toolbar) fn clamp_toolbar_offsets(
        &mut self,
        snapshot: &ToolbarSnapshot,
    ) -> bool {
        let width = self.surface.width() as f64;
        let height = self.surface.height() as f64;
        if width == 0.0 || height == 0.0 {
            drag_log(format!(
                "skip clamp: surface not configured (width={}, height={})",
                width, height
            ));
            return false;
        }
        let (top_w, top_h) = top_size(snapshot);
        let (side_w, side_h) = side_size(snapshot);
        let top_base_x = self.inline_top_base_x(snapshot);
        let top_base_y = self.inline_top_base_y();

        let before_top = (self.data.toolbar_top_offset, self.data.toolbar_top_offset_y);
        let before_side = (
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset,
        );
        let input = geometry::ToolbarClampInput {
            width,
            height,
            top_size: (top_w, top_h),
            side_size: (side_w, side_h),
            top_base_x,
            top_base_y,
            top_margin_right: Self::TOP_MARGIN_RIGHT,
            top_margin_bottom: Self::TOP_MARGIN_BOTTOM,
            side_base_margin_left: Self::SIDE_BASE_MARGIN_LEFT,
            side_base_margin_top: Self::SIDE_BASE_MARGIN_TOP,
            side_margin_right: Self::SIDE_MARGIN_RIGHT,
            side_margin_bottom: Self::SIDE_MARGIN_BOTTOM,
        };
        let offsets = geometry::ToolbarOffsets {
            top_x: self.data.toolbar_top_offset,
            top_y: self.data.toolbar_top_offset_y,
            side_x: self.data.toolbar_side_offset_x,
            side_y: self.data.toolbar_side_offset,
        };
        let (clamped, bounds) = geometry::clamp_toolbar_offsets(offsets, input);
        self.data.toolbar_top_offset = clamped.top_x;
        self.data.toolbar_top_offset_y = clamped.top_y;
        self.data.toolbar_side_offset_x = clamped.side_x;
        self.data.toolbar_side_offset = clamped.side_y;
        drag_log(format!(
            "clamp offsets: before=({:.3}, {:.3})/({:.3}, {:.3}), after=({:.3}, {:.3})/({:.3}, {:.3}), max=({:.3}, {:.3})/({:.3}, {:.3}), size=({}, {}), top_base_x={:.3}, top_base_y={:.3}",
            before_top.0,
            before_top.1,
            before_side.0,
            before_side.1,
            self.data.toolbar_top_offset,
            self.data.toolbar_top_offset_y,
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset,
            bounds.max_top_x,
            bounds.max_top_y,
            bounds.max_side_x,
            bounds.max_side_y,
            width,
            height,
            top_base_x,
            top_base_y
        ));
        true
    }

    pub(in crate::backend::wayland::state::toolbar) fn apply_toolbar_offsets(
        &mut self,
        snapshot: &ToolbarSnapshot,
    ) -> (bool, bool) {
        if self.surface.width() == 0 || self.surface.height() == 0 {
            drag_log(format!(
                "skip apply_toolbar_offsets: surface not configured (width={}, height={})",
                self.surface.width(),
                self.surface.height()
            ));
            return (false, false);
        }
        let _ = self.clamp_toolbar_offsets(snapshot);
        if self.layer_shell.is_none() {
            return (false, false);
        }
        let top_base_x = self.inline_top_base_x(snapshot);
        let (top_margin_left, top_margin_top, side_margin_top, side_margin_left) =
            geometry::compute_layer_margins(
                top_base_x,
                Self::TOP_BASE_MARGIN_TOP,
                Self::SIDE_BASE_MARGIN_LEFT,
                Self::SIDE_BASE_MARGIN_TOP,
                geometry::ToolbarOffsets {
                    top_x: self.data.toolbar_top_offset,
                    top_y: self.data.toolbar_top_offset_y,
                    side_x: self.data.toolbar_side_offset_x,
                    side_y: self.data.toolbar_side_offset,
                },
            );
        drag_log(format!(
            "apply_toolbar_offsets: top_margin_left={}, top_margin_top={}, side_margin_top={}, side_margin_left={}, offsets=({}, {})/({}, {}), scale={}, top_base_x={}",
            top_margin_left,
            top_margin_top,
            side_margin_top,
            side_margin_left,
            self.data.toolbar_top_offset,
            self.data.toolbar_top_offset_y,
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset,
            self.surface.scale(),
            top_base_x
        ));
        if debug_toolbar_drag_logging_enabled() {
            debug!(
                "apply_toolbar_offsets: top_margin_left={} (last={:?}), top_margin_top={} (last={:?}), side_margin_top={} (last={:?}), side_margin_left={} (last={:?}), offsets=({}, {})/({}, {}), top_base_x={}",
                top_margin_left,
                self.data.last_applied_top_margin,
                top_margin_top,
                self.data.last_applied_top_margin_top,
                side_margin_top,
                self.data.last_applied_side_margin,
                side_margin_left,
                self.data.last_applied_side_margin_left,
                self.data.toolbar_top_offset,
                self.data.toolbar_top_offset_y,
                self.data.toolbar_side_offset_x,
                self.data.toolbar_side_offset,
                top_base_x
            );
        }
        let top_changed = self.data.last_applied_top_margin != Some(top_margin_left)
            || self.data.last_applied_top_margin_top != Some(top_margin_top);
        let side_changed = self.data.last_applied_side_margin != Some(side_margin_top)
            || self.data.last_applied_side_margin_left != Some(side_margin_left);
        if !top_changed && !side_changed {
            return (false, false);
        }
        self.data.last_applied_top_margin = Some(top_margin_left);
        self.data.last_applied_side_margin = Some(side_margin_top);
        self.data.last_applied_top_margin_top = Some(top_margin_top);
        self.data.last_applied_side_margin_left = Some(side_margin_left);
        if top_changed {
            self.toolbar
                .set_top_margins(top_margin_top, top_margin_left);
        }
        if side_changed {
            self.toolbar
                .set_side_margins(side_margin_top, side_margin_left);
        }
        (top_changed, side_changed)
    }
}
