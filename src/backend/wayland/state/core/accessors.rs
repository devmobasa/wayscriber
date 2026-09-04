use super::super::*;
use smithay_client_toolkit::shell::{WaylandSurface, wlr_layer::Layer};
use std::time::{Duration, Instant};

const XDG_FROZEN_FULLSCREEN_TIMEOUT: Duration = Duration::from_millis(1500);

fn xdg_frozen_fullscreen_timeout(
    pending_configure: bool,
    requested_at: Option<Instant>,
    now: Instant,
) -> Option<Duration> {
    if !pending_configure {
        return None;
    }
    Some(
        requested_at
            .and_then(|requested_at| requested_at.checked_add(XDG_FROZEN_FULLSCREEN_TIMEOUT))
            .map(|deadline| deadline.saturating_duration_since(now))
            .unwrap_or(Duration::ZERO),
    )
}

fn finish_xdg_frozen_fullscreen_request(
    state: &mut XdgFrozenFullscreenState,
    requested_at: &mut Option<Instant>,
) {
    *state = XdgFrozenFullscreenState::Inactive;
    *requested_at = None;
}

impl WaylandState {
    pub(in crate::backend::wayland) fn has_cursor_focus(&self) -> bool {
        self.focus.pointer_focused() || self.stylus_hover_cursor_visible()
    }

    pub(in crate::backend::wayland) fn cursor_blocked_by_toolbar(&self) -> bool {
        self.stylus_hover_cursor_position().is_none() && self.toolbar_chrome.pointer_over_toolbar()
    }

    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn stylus_hover_cursor_visible(&self) -> bool {
        self.tablet.hover_cursor_position().is_some()
    }

    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn stylus_hover_cursor_position(&self) -> Option<(f64, f64)> {
        self.tablet.hover_cursor_position()
    }

    /// Retire in-flight stylus input without dispatching a canvas release.
    ///
    /// A screen-region modal cancels the gesture the pen was making, and then
    /// consumes the tip-up the compositor still owes us — so nothing else
    /// retires the contact. Left set, `tablet.tip_down` hides the hover cursor
    /// and defers session saving until some unrelated release or proximity-out
    /// happens to clear it. The stroke was cancelled rather than finished, so
    /// its peak pressure is dropped instead of being committed to the tool.
    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn retire_stylus_contact(&mut self) {
        // Tablet events are coalesced until their frame commits, so input that
        // physically happened before the modal opened can still be sitting in
        // the buffer. Replaying it afterwards would start a region from a press
        // that predates the selector, or apply that batch's pressure and barrel
        // actions behind it. Dropping the batch is unconditional: an uncommitted
        // tip-down is exactly the case where no contact is logically held yet.
        // Clearing our own flags does not lift the pen. The compositor keeps
        // reporting this contact — pressure included — until the tip rises.
        let transition = self.tablet.retire_contact();
        self.mark_stylus_hover_cursor_dirty(transition.previous, transition.next);
    }

    #[cfg(not(feature = "tablet-input"))]
    pub(in crate::backend::wayland) fn stylus_hover_cursor_visible(&self) -> bool {
        false
    }

    #[cfg(not(feature = "tablet-input"))]
    pub(in crate::backend::wayland) fn stylus_hover_cursor_position(&self) -> Option<(f64, f64)> {
        None
    }

    /// Consume the disowned-contact latch, reporting whether the tip that just
    /// lifted belonged to a contact a screen modal had taken over.
    ///
    /// A disowned tip-up dispatches no canvas release and commits no thickness;
    /// the next press after it is an ordinary fresh contact.
    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn take_retired_stylus_contact(&mut self) -> bool {
        self.tablet.take_retired_contact()
    }

    #[cfg(not(feature = "tablet-input"))]
    pub(in crate::backend::wayland) fn retire_stylus_contact(&mut self) {}

    pub(in crate::backend::wayland) fn preferred_output_identity(&self) -> Option<&str> {
        self.data.preferred_output_identity.as_deref()
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn set_preferred_output_identity(
        &mut self,
        value: Option<String>,
    ) {
        self.data.preferred_output_identity = value;
    }

    pub(in crate::backend::wayland) fn xdg_fullscreen(&self) -> bool {
        self.data.xdg_fullscreen
    }

    pub(in crate::backend::wayland) fn xdg_frozen_fullscreen_requested(&self) -> bool {
        !matches!(
            self.data.xdg_frozen_fullscreen_state,
            crate::backend::wayland::state::XdgFrozenFullscreenState::Inactive
        )
    }

    pub(in crate::backend::wayland) fn xdg_frozen_fullscreen_pending_configure(&self) -> bool {
        matches!(
            self.data.xdg_frozen_fullscreen_state,
            crate::backend::wayland::state::XdgFrozenFullscreenState::PendingConfigure
        )
    }

    pub(in crate::backend::wayland) fn xdg_frozen_fullscreen_timeout(
        &self,
        now: Instant,
    ) -> Option<Duration> {
        xdg_frozen_fullscreen_timeout(
            self.xdg_frozen_fullscreen_pending_configure(),
            self.data.xdg_frozen_fullscreen_requested_at,
            now,
        )
    }

    pub(in crate::backend::wayland) fn xdg_frozen_fullscreen_timed_out(
        &self,
        now: Instant,
    ) -> bool {
        self.xdg_frozen_fullscreen_timeout(now)
            .is_some_and(|timeout| timeout.is_zero())
    }

    pub(in crate::backend::wayland) fn begin_xdg_frozen_fullscreen(&mut self) -> bool {
        let Some(window) = self.surface.xdg_window().cloned() else {
            return false;
        };
        self.data.xdg_frozen_fullscreen_state =
            crate::backend::wayland::state::XdgFrozenFullscreenState::PendingConfigure;
        self.data.xdg_frozen_fullscreen_requested_at = Some(Instant::now());
        if let Some(output) = self.preferred_fullscreen_output() {
            window.set_fullscreen(Some(&output));
        } else {
            window.set_fullscreen(None);
        }
        window.commit();
        true
    }

    pub(in crate::backend::wayland) fn restore_xdg_after_frozen(&mut self) {
        if !self.xdg_frozen_fullscreen_requested() {
            return;
        }
        if let Some(window) = self.surface.xdg_window().cloned() {
            if self.xdg_fullscreen() {
                if let Some(output) = self.preferred_fullscreen_output() {
                    window.set_fullscreen(Some(&output));
                } else {
                    window.set_fullscreen(None);
                }
            } else {
                window.unset_fullscreen();
                window.set_maximized();
            }
            window.commit();
        }
        finish_xdg_frozen_fullscreen_request(
            &mut self.data.xdg_frozen_fullscreen_state,
            &mut self.data.xdg_frozen_fullscreen_requested_at,
        );
    }

    pub(in crate::backend::wayland) fn activate_pending_frozen_image_for_current_surface(
        &mut self,
    ) {
        let was_xdg_frozen_fullscreen = self.xdg_frozen_fullscreen_requested();
        let (phys_width, phys_height) = self.surface.physical_dimensions();
        let live_output_count = self.live_output_count();
        match self.frozen.activate_pending_image_with_live_outputs(
            phys_width,
            phys_height,
            &mut self.input_state,
            live_output_count,
        ) {
            Ok(true) => {
                if was_xdg_frozen_fullscreen {
                    self.data.xdg_frozen_fullscreen_state =
                        crate::backend::wayland::state::XdgFrozenFullscreenState::Active;
                    self.data.xdg_frozen_fullscreen_requested_at = None;
                }
            }
            Ok(false) => {}
            Err(err) => {
                log::warn!("Frozen pending image activation failed: {}", err);
                self.restore_xdg_after_frozen();
            }
        }
    }

    pub(in crate::backend::wayland) fn main_surface_layer(&self) -> Layer {
        if self.data.main_surface_uses_overlay_layer {
            Layer::Overlay
        } else {
            Layer::Top
        }
    }

    pub(in crate::backend::wayland) fn xdg_focus_loss_exits_overlay(&self) -> bool {
        matches!(
            self.config.ui.xdg_focus_loss_behavior,
            crate::config::XdgFocusLossBehavior::Exit
        )
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn set_xdg_fullscreen(&mut self, value: bool) {
        self.data.xdg_fullscreen = value;
    }

    pub(in crate::backend::wayland) fn session_options(&self) -> Option<&SessionOptions> {
        self.session.options()
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn session_options_mut(
        &mut self,
    ) -> Option<&mut SessionOptions> {
        self.session.options_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_frozen_fullscreen_deadline_uses_injected_time() {
        let start = Instant::now();
        assert_eq!(
            xdg_frozen_fullscreen_timeout(true, Some(start), start),
            Some(XDG_FROZEN_FULLSCREEN_TIMEOUT)
        );
        assert_eq!(
            xdg_frozen_fullscreen_timeout(true, Some(start), start + XDG_FROZEN_FULLSCREEN_TIMEOUT,),
            Some(Duration::ZERO)
        );
        assert_eq!(
            xdg_frozen_fullscreen_timeout(false, Some(start), start),
            None
        );
        assert_eq!(
            xdg_frozen_fullscreen_timeout(true, None, start),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn finishing_xdg_frozen_fullscreen_request_eliminates_an_expired_timeout() {
        let start = Instant::now();
        let mut state = XdgFrozenFullscreenState::PendingConfigure;
        let mut requested_at = Some(start);

        finish_xdg_frozen_fullscreen_request(&mut state, &mut requested_at);

        assert_eq!(state, XdgFrozenFullscreenState::Inactive);
        assert_eq!(requested_at, None);
        assert_eq!(
            xdg_frozen_fullscreen_timeout(
                state == XdgFrozenFullscreenState::PendingConfigure,
                requested_at,
                start + XDG_FROZEN_FULLSCREEN_TIMEOUT,
            ),
            None
        );
    }
}
