use super::*;

fn active_drag_top_base_x(
    move_dragging: bool,
    gtk_drag_preview_active: bool,
    frozen_base_x: Option<f64>,
) -> Option<f64> {
    (move_dragging || gtk_drag_preview_active)
        .then_some(frozen_base_x)
        .flatten()
}

impl WaylandState {
    /// Base X position for the top toolbar when laid out inline.
    /// When a drag is in progress we freeze this base to avoid shifting the top bar while moving the side bar.
    pub(in crate::backend::wayland::state::toolbar) fn inline_top_base_x(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> f64 {
        if let Some(x) = active_drag_top_base_x(
            self.is_move_dragging(),
            self.data.gtk_drag_preview.is_some(),
            self.data.drag_top_base_x,
        ) {
            return x;
        }
        let allow_push = self.active_move_drag_kind() != Some(MoveDragKind::Side);
        self.computed_inline_top_base_x(snapshot, allow_push)
    }

    fn computed_inline_top_base_x(&self, snapshot: &ToolbarSnapshot, allow_push: bool) -> f64 {
        // The GTK frontend keeps the built-in side surface unmapped, but its
        // side palette occupies the same space and must push the top strip
        // identically.
        let side_visible = self.toolbar.is_side_visible()
            || (self.gtk_toolbars_active() && self.input_state.toolbar_side_visible());
        let side_size = side_size(snapshot);
        let top_size = top_size(snapshot);
        let side_start_y = Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset;
        let top_bottom_y =
            self.inline_top_base_y() + self.data.toolbar_top_offset_y + top_size.1 as f64;
        let base = Self::INLINE_SIDE_X;
        let result = geometry::compute_inline_top_base_x(
            base,
            side_visible,
            side_size.0 as f64,
            side_start_y,
            top_bottom_y,
            Self::INLINE_TOP_PUSH,
            allow_push,
        );
        if self.is_move_dragging()
            || self.toolbar_dragging()
            || self.data.gtk_drag_preview.is_some()
        {
            drag_log(format!(
                "inline_top_base_x: base={:.3}, side_visible={}, side_width={:.3}, side_start_y={:.3}, top_bottom_y={:.3}, allow_push={}, result={:.3}",
                base,
                side_visible,
                side_size.0 as f64,
                side_start_y,
                top_bottom_y,
                allow_push,
                result
            ));
        }
        result
    }

    /// Preserve the top strip's screen X while switching from the base frozen
    /// for a drag back to the resting overlap-derived base.
    pub(in crate::backend::wayland::state::toolbar) fn reconcile_top_base_after_drag(&mut self) {
        let Some(old_base_x) = self.data.drag_top_base_x else {
            return;
        };
        let snapshot = self.toolbar_snapshot();
        let new_base_x = self.computed_inline_top_base_x(&snapshot, true);
        let delta = old_base_x - new_base_x;
        if delta.abs() <= 0.01 {
            return;
        }
        self.data.toolbar_top_offset += delta;
        drag_log(format!(
            "end move drag: preserve top position, old_base_x={old_base_x:.3}, new_base_x={new_base_x:.3}, delta={delta:.3}, top_offset=({}, {})",
            self.data.toolbar_top_offset, self.data.toolbar_top_offset_y,
        ));
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

#[cfg(test)]
mod tests {
    use super::active_drag_top_base_x;

    #[test]
    fn gtk_preview_uses_the_base_frozen_at_drag_start() {
        assert_eq!(active_drag_top_base_x(false, true, Some(24.0)), Some(24.0));
    }

    #[test]
    fn idle_layout_does_not_reuse_a_stale_frozen_base() {
        assert_eq!(active_drag_top_base_x(false, false, Some(24.0)), None);
    }
}
