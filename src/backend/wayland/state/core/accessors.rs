use super::super::*;
use std::time::{Duration, Instant};

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

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn set_xdg_fullscreen(&mut self, value: bool) {
        self.data.xdg_fullscreen = value;
    }

    pub(in crate::backend::wayland) fn session_options(&self) -> Option<&SessionOptions> {
        self.session.options()
    }

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
