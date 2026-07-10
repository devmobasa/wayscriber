use wayland_client::protocol::wl_surface;

use super::*;
use crate::backend::wayland::toolbar::ToolbarFocusTarget;
use crate::backend::wayland::toolbar::layout::side_scroll_bounds;

/// Wheel step for the side palette, in pre-scale spec units.
const SIDE_WHEEL_SCROLL_STEP: f64 = 48.0;

impl WaylandState {
    /// True when a wheel event should scroll the side palette: the pointer
    /// is over the side toolbar surface, or over the inline side palette.
    pub(in crate::backend::wayland) fn wheel_over_side_toolbar(
        &self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
    ) -> bool {
        if self.toolbar.focus_target_for_surface(surface) == Some(ToolbarFocusTarget::Side) {
            return true;
        }
        self.inline_toolbars_active()
            && self.data.inline_side_rect.is_some_and(|(x, y, w, h)| {
                geometry::point_in_rect(position.0, position.1, x, y, w, h)
            })
    }

    /// Scrolls the active side pane by wheel notches when it overflows its
    /// viewport. Returns true when the scroll offset changed.
    pub(in crate::backend::wayland) fn scroll_side_pane_by_wheel(
        &mut self,
        scroll_direction: i32,
    ) -> bool {
        if scroll_direction == 0 {
            return false;
        }
        let snapshot = self.toolbar_snapshot();
        if snapshot.side_minimized {
            return false;
        }
        let (natural, viewport) = side_scroll_bounds(&snapshot);
        let max_scroll = (natural - viewport).max(0.0);
        if max_scroll <= 0.0 {
            return false;
        }
        let next = (snapshot.side_scroll + scroll_direction as f64 * SIDE_WHEEL_SCROLL_STEP)
            .clamp(0.0, max_scroll);
        if (next - snapshot.side_scroll).abs() < 0.5 {
            return false;
        }
        self.handle_toolbar_event(ToolbarEvent::ScrollSidePane(next), None, None);
        true
    }

    /// Drops keyboard focus on the side toolbar. Pane switches rebuild the
    /// hit list, so a stale focus index would land on an unrelated widget;
    /// the next Tab re-seeds focus at the first focusable region.
    pub(in crate::backend::wayland) fn reset_side_toolbar_focus(&mut self) {
        self.toolbar.clear_side_focus();
        if self.data.inline_side_focus_index.take().is_some() && self.inline_toolbars_active() {
            self.input_state.needs_redraw = true;
        }
    }
}
