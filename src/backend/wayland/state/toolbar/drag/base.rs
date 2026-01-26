use super::*;

impl WaylandState {
    /// Base X position for the top toolbar when laid out inline.
    /// When a drag is in progress we freeze this base to avoid shifting the top bar while moving the side bar.
    pub(in crate::backend::wayland::state::toolbar) fn inline_top_base_x(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        if self.is_move_dragging()
            && let Some(x) = self.data.drag_top_base_x
        {
            return x;
        }
        let side_visible = self.toolbar.is_side_visible();
        let side_size = side_size(snapshot);
        let top_size = top_size(snapshot);
        let side_start_y = Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset;
        let top_bottom_y = Self::INLINE_TOP_Y + self.data.toolbar_top_offset_y + top_size.1 as f64;
        let base = Self::INLINE_SIDE_X;
        // When dragging the side toolbar, don't push the top bar; keep its base stable so it
        // doesn't shift while moving the side bar.
        let allow_push = self.active_move_drag_kind() != Some(MoveDragKind::Side);
        geometry::compute_inline_top_base_x(
            base,
            side_visible,
            side_size.0 as f64,
            side_start_y,
            top_bottom_y,
            Self::INLINE_TOP_PUSH,
            allow_push,
        )
    }

    pub(in crate::backend::wayland::state::toolbar) fn inline_top_base_y(&self) -> f64 {
        if self.is_move_dragging()
            && let Some(y) = self.data.drag_top_base_y
        {
            return y;
        }
        Self::INLINE_TOP_Y
    }

    /// Convert a toolbar-local coordinate into a screen-relative coordinate so that
    /// dragging continues to work even after the surface has moved.
    pub(in crate::backend::wayland) fn local_to_screen_coords(
        &self,
        kind: MoveDragKind,
        local_coord: (f64, f64),
    ) -> (f64, f64) {
        match kind {
            MoveDragKind::Top => (
                self.inline_top_base_x(&self.toolbar_snapshot())
                    + self.data.toolbar_top_offset
                    + local_coord.0,
                self.inline_top_base_y() + self.data.toolbar_top_offset_y + local_coord.1,
            ),
            MoveDragKind::Side => (
                Self::SIDE_BASE_MARGIN_LEFT + self.data.toolbar_side_offset_x + local_coord.0,
                Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset + local_coord.1,
            ),
        }
    }
}
