use smithay_client_toolkit::shell::{WaylandSurface, wlr_layer::Layer};

use super::super::*;
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

impl WaylandState {
    pub(in crate::backend::wayland) fn current_mouse(&self) -> (i32, i32) {
        (self.data.current_mouse_x, self.data.current_mouse_y)
    }

    pub(in crate::backend::wayland) fn set_current_mouse(&mut self, x: i32, y: i32) {
        self.data.current_mouse_x = x;
        self.data.current_mouse_y = y;
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn has_keyboard_focus(&self) -> bool {
        self.data.has_keyboard_focus
    }

    pub(in crate::backend::wayland) fn set_keyboard_focus(&mut self, value: bool) {
        self.data.has_keyboard_focus = value;
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn has_pointer_focus(&self) -> bool {
        self.data.has_pointer_focus
    }

    pub(in crate::backend::wayland) fn has_cursor_focus(&self) -> bool {
        self.has_pointer_focus() || self.stylus_hover_cursor_visible()
    }

    pub(in crate::backend::wayland) fn cursor_blocked_by_toolbar(&self) -> bool {
        self.stylus_hover_cursor_position().is_none() && self.pointer_over_toolbar()
    }

    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn stylus_hover_cursor_visible(&self) -> bool {
        self.stylus_on_overlay
            && !self.stylus_on_toolbar
            && !self.stylus_tip_down
            && self.stylus_last_pos.is_some()
    }

    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn stylus_hover_cursor_position(&self) -> Option<(f64, f64)> {
        if self.stylus_hover_cursor_visible() {
            self.stylus_last_pos
        } else {
            None
        }
    }

    #[cfg(not(feature = "tablet-input"))]
    pub(in crate::backend::wayland) fn stylus_hover_cursor_visible(&self) -> bool {
        false
    }

    #[cfg(not(feature = "tablet-input"))]
    pub(in crate::backend::wayland) fn stylus_hover_cursor_position(&self) -> Option<(f64, f64)> {
        None
    }

    pub(in crate::backend::wayland) fn set_pointer_focus(&mut self, value: bool) {
        self.data.has_pointer_focus = value;
    }

    pub(in crate::backend::wayland) fn current_seat(&self) -> Option<wl_seat::WlSeat> {
        self.data.current_seat.clone()
    }

    #[allow(dead_code)] // Kept for potential future pointer lock support
    pub(in crate::backend::wayland) fn current_pointer(&self) -> Option<wl_pointer::WlPointer> {
        self.themed_pointer.as_ref().map(|p| p.pointer().clone())
    }

    pub(in crate::backend::wayland) fn current_seat_id(&self) -> Option<u32> {
        self.data
            .current_seat
            .as_ref()
            .map(|seat| seat.id().protocol_id())
    }

    pub(in crate::backend::wayland) fn set_current_seat(&mut self, seat: Option<wl_seat::WlSeat>) {
        self.data.current_seat = seat;
    }

    pub(in crate::backend::wayland) fn last_activation_serial(&self) -> Option<u32> {
        self.data.last_activation_serial
    }

    pub(in crate::backend::wayland) fn set_last_activation_serial(&mut self, serial: Option<u32>) {
        self.data.last_activation_serial = serial;
    }

    pub(in crate::backend::wayland) fn current_keyboard_interactivity(
        &self,
    ) -> Option<KeyboardInteractivity> {
        self.data.current_keyboard_interactivity
    }

    pub(in crate::backend::wayland) fn set_current_keyboard_interactivity(
        &mut self,
        interactivity: Option<KeyboardInteractivity>,
    ) {
        self.data.current_keyboard_interactivity = interactivity;
    }

    pub(in crate::backend::wayland) fn suppress_focus_exit_for(&mut self, duration: Duration) {
        self.data.suppress_focus_exit_until = Some(Instant::now() + duration);
    }

    pub(in crate::backend::wayland) fn focus_exit_suppressed(&self) -> bool {
        self.data
            .suppress_focus_exit_until
            .is_some_and(|until| Instant::now() <= until)
    }

    pub(in crate::backend::wayland) fn focus_exit_timeout(&self, now: Instant) -> Option<Duration> {
        self.data
            .suppress_focus_exit_until
            .and_then(|until| (until > now).then(|| until.saturating_duration_since(now)))
    }

    pub(in crate::backend::wayland) fn focus_exit_suppression_expired(&self, now: Instant) -> bool {
        self.data
            .suppress_focus_exit_until
            .is_some_and(|until| now >= until)
    }

    pub(in crate::backend::wayland) fn clear_focus_exit_suppression(&mut self) {
        self.data.suppress_focus_exit_until = None;
    }

    pub(in crate::backend::wayland) fn set_xdg_close_guard_for(&mut self, duration: Duration) {
        self.data.xdg_close_guard_until = Some(Instant::now() + duration);
    }

    pub(in crate::backend::wayland) fn clear_xdg_close_guard(&mut self) {
        self.data.xdg_close_guard_until = None;
    }

    pub(in crate::backend::wayland) fn xdg_close_guard_active(&self, now: Instant) -> bool {
        self.data
            .xdg_close_guard_until
            .is_some_and(|until| now <= until)
    }

    pub(in crate::backend::wayland) fn mark_xdg_explicit_close_requested(&mut self) {
        self.data.xdg_explicit_close_requested = true;
    }

    pub(in crate::backend::wayland) fn take_xdg_explicit_close_requested(&mut self) -> bool {
        let was_requested = self.data.xdg_explicit_close_requested;
        self.data.xdg_explicit_close_requested = false;
        was_requested
    }

    pub(in crate::backend::wayland) fn frozen_enabled(&self) -> bool {
        self.data.frozen_enabled
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn set_frozen_enabled(&mut self, value: bool) {
        self.data.frozen_enabled = value;
    }

    pub(in crate::backend::wayland) fn pending_freeze_on_start(&self) -> bool {
        self.data.pending_freeze_on_start
    }

    pub(in crate::backend::wayland) fn set_pending_freeze_on_start(&mut self, value: bool) {
        self.data.pending_freeze_on_start = value;
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn has_seen_surface_enter(&self) -> bool {
        self.data.has_seen_surface_enter
    }

    pub(in crate::backend::wayland) fn set_has_seen_surface_enter(&mut self, value: bool) {
        self.data.has_seen_surface_enter = value;
    }

    pub(in crate::backend::wayland) fn pending_activation_token(&self) -> Option<String> {
        self.data.pending_activation_token.clone()
    }

    pub(in crate::backend::wayland) fn set_pending_activation_token(
        &mut self,
        token: Option<String>,
    ) {
        self.data.pending_activation_token = token;
    }

    pub(in crate::backend::wayland) fn take_startup_activation_token(&mut self) -> Option<String> {
        self.data.startup_activation_token.take()
    }

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
        self.data.xdg_frozen_fullscreen_state =
            crate::backend::wayland::state::XdgFrozenFullscreenState::Inactive;
        self.data.xdg_frozen_fullscreen_requested_at = None;
    }

    pub(in crate::backend::wayland) fn activate_pending_frozen_image_for_current_surface(
        &mut self,
    ) {
        let was_xdg_frozen_fullscreen = self.xdg_frozen_fullscreen_requested();
        let (phys_width, phys_height) = self.surface.physical_dimensions();
        match self
            .frozen
            .activate_pending_image(phys_width, phys_height, &mut self.input_state)
        {
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
                self.input_state
                    .set_ui_toast(crate::input::state::UiToastKind::Error, err);
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

    /// Returns true if the overlay is ready to process keybinds (surface configured + focus).
    pub(in crate::backend::wayland) fn is_overlay_ready(&self) -> bool {
        self.data.overlay_ready
    }

    /// Sets the overlay ready state. Should be true only when surface is configured and has focus.
    pub(in crate::backend::wayland) fn set_overlay_ready(&mut self, value: bool) {
        self.data.overlay_ready = value;
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
}
