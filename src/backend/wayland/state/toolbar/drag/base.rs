use super::*;

impl WaylandState {
    /// Base X position for the top toolbar when laid out inline.
    /// A drag freezes this base so the resting layout cannot shift underneath
    /// the surface being moved.
    pub(in crate::backend::wayland::state) fn inline_top_base_x(&self) -> f64 {
        self.toolbar_drag
            .frozen_base_x()
            .unwrap_or(Self::INLINE_TOP_X)
    }

    /// Preserve the top strip's screen X while switching from the base frozen
    /// for a drag back to the resting base.
    ///
    /// Only the two explicit drag-commit paths call this
    /// (`finish_toolbar_move_drag` with `commit`, and `finish_gtk_offset_change`),
    /// each immediately before committing the drag's position override. Every
    /// other implicit toolbar move — a layout-mode switch, an output resize, a
    /// relayout clamp — adjusts the live offsets in `toolbar_chrome` only and never
    /// stages an override.
    pub(in crate::backend::wayland::state::toolbar) fn reconcile_top_base_after_drag(
        &mut self,
        old_base_x: f64,
    ) {
        let new_base_x = Self::INLINE_TOP_X;
        let delta = old_base_x - new_base_x;
        if delta.abs() <= 0.01 {
            return;
        }
        self.toolbar_chrome.add_top_offset((delta, 0.0));
        drag_log(|| {
            format!(
                "end move drag: preserve top position, old_base_x={old_base_x:.3}, new_base_x={new_base_x:.3}, delta={delta:.3}, top_offset=({}, {})",
                self.toolbar_chrome.top_offset().0,
                self.toolbar_chrome.top_offset().1,
            )
        });
    }

    pub(in crate::backend::wayland::state) fn inline_top_base_y(&self) -> f64 {
        self.toolbar_drag
            .frozen_base_y()
            .unwrap_or(Self::INLINE_TOP_Y)
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
                self.inline_top_base_x() + self.toolbar_chrome.top_offset().0 + local_coord.0,
                self.inline_top_base_y() + self.toolbar_chrome.top_offset().1 + local_coord.1,
            ),
        }
    }
}
