use wayland_client::protocol::wl_surface;

use super::*;
use crate::backend::wayland::toolbar::layout::top_popover_scroll_bounds;

/// Wheel step for the top-strip Canvas/Session/Settings popovers, in pre-scale
/// spec units.
const WHEEL_SCROLL_STEP: f64 = 48.0;

impl WaylandState {
    /// True when a wheel event landed on the top strip: the pointer is over
    /// the top toolbar surface, or over the inline top strip. With a
    /// Canvas/Session/Settings popover open the wheel scrolls its content (the
    /// GTK popovers scroll via their `ScrolledWindow`).
    pub(in crate::backend::wayland) fn wheel_over_top_toolbar(
        &self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
    ) -> bool {
        if self.toolbar.is_focusable_surface(surface) {
            return true;
        }
        self.toolbar_chrome.inline_toolbars()
            && self
                .toolbar_chrome
                .inline_rect()
                .is_some_and(|(x, y, w, h)| {
                    geometry::point_in_rect(position.0, position.1, x, y, w, h)
                })
    }

    /// Scrolls the open Canvas/Session/Settings popover by wheel notches when its
    /// content overflows the capped viewport. Returns true when the scroll
    /// offset changed. `ScrollTopPopover` is on the popovers' spared-event
    /// list, so routing it never dismisses them.
    pub(in crate::backend::wayland) fn scroll_top_popover_by_wheel(
        &mut self,
        scroll_direction: i32,
    ) -> bool {
        if scroll_direction == 0 {
            return false;
        }
        let snapshot = self.toolbar_snapshot();
        let Some((natural, viewport)) = top_popover_scroll_bounds(&snapshot) else {
            return false;
        };
        let max_scroll = (natural - viewport).max(0.0);
        if max_scroll <= 0.0 {
            return false;
        }
        let next = (snapshot.top_popover_scroll + scroll_direction as f64 * WHEEL_SCROLL_STEP)
            .clamp(0.0, max_scroll);
        if (next - snapshot.top_popover_scroll).abs() < 0.5 {
            return false;
        }
        self.handle_toolbar_event(ToolbarEvent::ScrollTopPopover(next), None, None);
        true
    }
}
