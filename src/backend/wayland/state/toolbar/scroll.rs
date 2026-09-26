use smithay_client_toolkit::seat::pointer::AxisScroll;
use wayland_client::protocol::wl_surface;

use super::*;
use crate::backend::wayland::toolbar::layout::top_popover_scroll_bounds;
use crate::ui::toolbar::model::StrokeSetting;

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
        let Some((natural, viewport)) = top_popover_scroll_bounds(self.render.ui_text(), &snapshot)
        else {
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

    /// Steps the level meter under the pointer (inline in the style pill, or
    /// in the Pen feel panel): travel away from
    /// the user raises it, toward lowers it, one level per wheel notch.
    /// Partial notches from a high-resolution wheel or a touchpad accumulate
    /// until they make a whole level. Returns true when the wheel landed on a
    /// meter, whether or not a level changed, so a partial notch or a wheel at
    /// either end of the range is still consumed there.
    pub(in crate::backend::wayland) fn step_style_meter_by_wheel(
        &mut self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
        vertical: AxisScroll,
    ) -> bool {
        let snapshot = self.toolbar_snapshot();
        let Some(setting) = self.meter_setting_at(surface, position, &snapshot) else {
            self.toolbar_chrome.meter_wheel_mut().reset();
            return false;
        };

        let levels = self.toolbar_chrome.meter_wheel_mut().levels(
            setting,
            vertical.value120,
            vertical.discrete,
            vertical.absolute,
        );
        // Positive Wayland axis values scroll down; the meter rises when the
        // user scrolls up.
        if levels != 0
            && let Some(event) = setting.wheel_event(&snapshot, -levels)
        {
            self.handle_toolbar_event(event, None, None);
        }
        true
    }

    /// Drops partial wheel travel once the pointer is off the meter it
    /// belongs to, so a later visit starts from zero. Only looks up the
    /// pointer's target while a partial level is actually pending.
    pub(in crate::backend::wayland) fn forget_meter_wheel_off_meter(
        &mut self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
    ) {
        if !self.toolbar_chrome.meter_wheel().is_pending() {
            return;
        }

        let snapshot = self.toolbar_snapshot();
        let setting = self.meter_setting_at(surface, position, &snapshot);
        self.toolbar_chrome.meter_wheel_mut().keep_only(setting);
    }

    /// The meter setting whose bar is under the pointer, on the toolbar
    /// surface or the inline strip.
    fn meter_setting_at(
        &self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
        snapshot: &ToolbarSnapshot,
    ) -> Option<StrokeSetting> {
        self.toolbar
            .top_hit_at(surface, position)
            .or_else(|| self.inline_toolbar_hit_at(position))
            .and_then(|(intent, _)| StrokeSetting::for_wheel(&intent.0, snapshot))
    }
}
