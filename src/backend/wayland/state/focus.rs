use std::time::{Duration, Instant};

use smithay_client_toolkit::shell::wlr_layer::KeyboardInteractivity;
use wayland_client::{Proxy, protocol::wl_seat};

use super::WaylandState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum MainLayerFocusPhase {
    #[default]
    Acquiring,
    Acquired,
}

/// Keyboard, pointer, activation, and focus-loss state for the overlay.
pub(in crate::backend::wayland) struct FocusState {
    has_keyboard_focus: bool,
    main_layer_focus_phase: MainLayerFocusPhase,
    has_pointer_focus: bool,
    current_seat: Option<wl_seat::WlSeat>,
    last_activation_serial: Option<u32>,
    overlay_ready: bool,
    suppress_focus_exit_until: Option<Instant>,
    xdg_close_guard_until: Option<Instant>,
    xdg_explicit_close_requested: bool,
    pending_activation_token: Option<String>,
    startup_activation_token: Option<String>,
    current_keyboard_interactivity: Option<KeyboardInteractivity>,
    /// When a reclaim after a toolbar keyboard release stops asking for
    /// Exclusive if no keyboard enter arrived, so the GTK bars (click-through
    /// while the overlay is Exclusive) can never stay unusable.
    keyboard_reclaim_deadline: Option<Instant>,
}

/// How long the overlay asks for Exclusive keyboard interactivity to take the
/// keyboard back from a toolbar that released it.
const KEYBOARD_RECLAIM_WINDOW: Duration = Duration::from_millis(750);

impl FocusState {
    pub(in crate::backend::wayland) fn new(startup_activation_token: Option<String>) -> Self {
        Self {
            has_keyboard_focus: false,
            main_layer_focus_phase: MainLayerFocusPhase::default(),
            has_pointer_focus: false,
            current_seat: None,
            last_activation_serial: None,
            overlay_ready: false,
            suppress_focus_exit_until: None,
            xdg_close_guard_until: None,
            xdg_explicit_close_requested: false,
            pending_activation_token: None,
            startup_activation_token,
            current_keyboard_interactivity: None,
            keyboard_reclaim_deadline: None,
        }
    }

    pub(in crate::backend::wayland) fn keyboard_focused(&self) -> bool {
        self.has_keyboard_focus
    }

    pub(in crate::backend::wayland) fn keyboard_entered(&mut self) {
        self.has_keyboard_focus = true;
    }

    pub(in crate::backend::wayland) fn keyboard_left(&mut self) {
        self.has_keyboard_focus = false;
        self.overlay_ready = false;
        self.main_layer_focus_phase = self.main_layer_focus_phase.after_keyboard_teardown();
    }

    pub(in crate::backend::wayland) fn pointer_focused(&self) -> bool {
        self.has_pointer_focus
    }

    pub(in crate::backend::wayland) fn set_pointer_focused(&mut self, focused: bool) {
        self.has_pointer_focus = focused;
    }

    pub(in crate::backend::wayland) fn current_seat(&self) -> Option<wl_seat::WlSeat> {
        self.current_seat.clone()
    }

    pub(in crate::backend::wayland) fn current_seat_id(&self) -> Option<u32> {
        self.current_seat
            .as_ref()
            .map(|seat| seat.id().protocol_id())
    }

    pub(in crate::backend::wayland) fn set_current_seat(&mut self, seat: Option<wl_seat::WlSeat>) {
        self.current_seat = seat;
    }

    pub(in crate::backend::wayland) fn last_activation_serial(&self) -> Option<u32> {
        self.last_activation_serial
    }

    pub(in crate::backend::wayland) fn note_activation_serial(&mut self, serial: u32) {
        self.last_activation_serial = Some(serial);
    }

    pub(in crate::backend::wayland) fn current_keyboard_interactivity(
        &self,
    ) -> Option<KeyboardInteractivity> {
        self.current_keyboard_interactivity
    }

    pub(in crate::backend::wayland) fn set_keyboard_interactivity(
        &mut self,
        interactivity: Option<KeyboardInteractivity>,
    ) {
        self.current_keyboard_interactivity = interactivity;
    }

    pub(in crate::backend::wayland) fn begin_main_layer_acquisition(&mut self) {
        self.main_layer_focus_phase = MainLayerFocusPhase::Acquiring;
    }

    pub(in crate::backend::wayland) fn main_layer_acquiring(&self) -> bool {
        self.main_layer_focus_phase == MainLayerFocusPhase::Acquiring
    }

    pub(in crate::backend::wayland) fn can_complete_main_layer_acquisition(
        &self,
        is_current_main_layer_surface: bool,
        keyboard_release_requested: bool,
    ) -> bool {
        is_current_main_layer_surface
            && self.main_layer_acquiring()
            && self.current_keyboard_interactivity == Some(KeyboardInteractivity::Exclusive)
            && !keyboard_release_requested
    }

    /// Take the keyboard back after the GTK toolbar released it without handing
    /// it on (Hyprland moves focus only on the next pointer motion): acquire
    /// the main layer the way startup does, but only for a short window.
    pub(in crate::backend::wayland) fn begin_keyboard_reclaim(&mut self, now: Instant) {
        self.main_layer_focus_phase = MainLayerFocusPhase::Acquiring;
        self.keyboard_reclaim_deadline = Some(now + KEYBOARD_RECLAIM_WINDOW);
    }

    /// Give up a reclaim whose window passed without a keyboard enter.
    /// Returns true when the overlay stopped asking for Exclusive.
    pub(in crate::backend::wayland) fn expire_keyboard_reclaim(&mut self, now: Instant) -> bool {
        let Some(deadline) = self.keyboard_reclaim_deadline else {
            return false;
        };
        if now < deadline {
            return false;
        }

        self.keyboard_reclaim_deadline = None;
        if self.main_layer_focus_phase != MainLayerFocusPhase::Acquiring {
            return false;
        }
        self.main_layer_focus_phase = MainLayerFocusPhase::Acquired;
        true
    }

    pub(in crate::backend::wayland) fn keyboard_reclaim_timeout(
        &self,
        now: Instant,
    ) -> Option<Duration> {
        self.keyboard_reclaim_deadline
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    pub(in crate::backend::wayland) fn complete_main_layer_acquisition(&mut self) -> bool {
        self.keyboard_reclaim_deadline = None;
        if self.main_layer_focus_phase == MainLayerFocusPhase::Acquired {
            return false;
        }
        self.main_layer_focus_phase = MainLayerFocusPhase::Acquired;
        true
    }

    pub(in crate::backend::wayland) fn mark_ready_if_focused(&mut self) -> bool {
        if !self.has_keyboard_focus || self.overlay_ready {
            return false;
        }
        self.overlay_ready = true;
        true
    }

    pub(in crate::backend::wayland) fn is_ready(&self) -> bool {
        self.overlay_ready
    }

    pub(in crate::backend::wayland) fn suppress_exit_for(
        &mut self,
        now: Instant,
        duration: Duration,
    ) {
        self.suppress_focus_exit_until = Some(now + duration);
    }

    pub(in crate::backend::wayland) fn exit_suppressed(&self, now: Instant) -> bool {
        self.suppress_focus_exit_until
            .is_some_and(|until| now <= until)
    }

    pub(in crate::backend::wayland) fn exit_timeout(&self, now: Instant) -> Option<Duration> {
        self.suppress_focus_exit_until
            .and_then(|until| (until > now).then(|| until.saturating_duration_since(now)))
    }

    pub(in crate::backend::wayland) fn exit_suppression_expired(&self, now: Instant) -> bool {
        self.suppress_focus_exit_until
            .is_some_and(|until| now >= until)
    }

    pub(in crate::backend::wayland) fn clear_exit_suppression(&mut self) {
        self.suppress_focus_exit_until = None;
    }

    pub(in crate::backend::wayland) fn guard_xdg_close_for(
        &mut self,
        now: Instant,
        duration: Duration,
    ) {
        self.xdg_close_guard_until = Some(now + duration);
    }

    pub(in crate::backend::wayland) fn clear_xdg_close_guard(&mut self) {
        self.xdg_close_guard_until = None;
    }

    pub(in crate::backend::wayland) fn xdg_close_guard_active(&self, now: Instant) -> bool {
        self.xdg_close_guard_until.is_some_and(|until| now <= until)
    }

    pub(in crate::backend::wayland) fn ignores_xdg_close(
        &self,
        stay_mode: bool,
        now: Instant,
    ) -> bool {
        stay_mode && !self.has_keyboard_focus && self.xdg_close_guard_active(now)
    }

    pub(in crate::backend::wayland) fn mark_xdg_explicit_close_requested(&mut self) {
        self.xdg_explicit_close_requested = true;
    }

    pub(in crate::backend::wayland) fn take_xdg_explicit_close_requested(&mut self) -> bool {
        std::mem::take(&mut self.xdg_explicit_close_requested)
    }

    pub(in crate::backend::wayland) fn activation_token_to_apply(&self) -> Option<String> {
        self.pending_activation_token.clone()
    }

    pub(in crate::backend::wayland) fn note_activation_token(&mut self, token: String) {
        self.pending_activation_token = Some(token);
    }

    pub(in crate::backend::wayland) fn defer_activation_until_serial(&mut self) {
        self.pending_activation_token = Some(String::new());
    }

    pub(in crate::backend::wayland) fn clear_pending_activation_token(&mut self) {
        self.pending_activation_token = None;
    }

    pub(in crate::backend::wayland) fn retry_activation_wanted(&self) -> bool {
        self.pending_activation_token.is_some() && self.last_activation_serial.is_some()
    }

    pub(in crate::backend::wayland) fn take_startup_activation_token(&mut self) -> Option<String> {
        self.startup_activation_token.take()
    }
}

impl MainLayerFocusPhase {
    fn after_keyboard_teardown(self) -> Self {
        self
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn try_complete_main_layer_focus_acquisition(
        &mut self,
        is_current_main_layer_surface: bool,
    ) -> bool {
        if !self.focus.can_complete_main_layer_acquisition(
            is_current_main_layer_surface,
            self.overlay_keyboard_passthrough_requested(),
        ) {
            return false;
        }
        self.focus.complete_main_layer_acquisition()
    }

    /// The GTK toolbar dropped keyboard focus (a focused slider spent Escape
    /// on releasing it). Claim it for the overlay right away, unless the
    /// overlay already has it or deliberately passes keys through.
    pub(in crate::backend::wayland) fn reclaim_keyboard_after_toolbar_release(
        &mut self,
        now: Instant,
    ) -> bool {
        if self.focus.keyboard_focused() || self.overlay_keyboard_passthrough_requested() {
            return false;
        }

        self.focus.begin_keyboard_reclaim(now);
        true
    }

    /// Retire every keyboard-owned transient when focus is lost.
    pub(in crate::backend::wayland) fn teardown_keyboard_focus(&mut self) {
        self.focus.keyboard_left();
        self.clear_toolbar_focus();
        self.input_state.clear_focus_owned_key_state();
        self.sync_region_square_modifier(false);
        self.clear_key_repeat();
        self.pointer.set_board_pan_key_held(false);
        self.pointer.stop_board_pan();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reclaim asks for Exclusive until the keyboard enters; the enter
    /// settles it and clears the deadline, so nothing expires later.
    #[test]
    fn keyboard_reclaim_is_settled_by_the_keyboard_enter() {
        let now = Instant::now();
        let mut focus = FocusState::new(None);
        focus.complete_main_layer_acquisition();

        focus.begin_keyboard_reclaim(now);
        assert!(focus.main_layer_acquiring());
        assert_eq!(
            focus.keyboard_reclaim_timeout(now),
            Some(KEYBOARD_RECLAIM_WINDOW)
        );

        assert!(focus.complete_main_layer_acquisition());
        assert!(!focus.main_layer_acquiring());
        assert_eq!(focus.keyboard_reclaim_timeout(now), None);
        assert!(!focus.expire_keyboard_reclaim(now + KEYBOARD_RECLAIM_WINDOW));
    }

    /// Without an enter the reclaim gives up at its deadline, so the overlay
    /// stops asking for Exclusive and the GTK bars take clicks again.
    #[test]
    fn keyboard_reclaim_without_an_enter_gives_up_at_its_deadline() {
        let now = Instant::now();
        let mut focus = FocusState::new(None);
        focus.complete_main_layer_acquisition();
        focus.begin_keyboard_reclaim(now);

        assert!(!focus.expire_keyboard_reclaim(now + Duration::from_millis(100)));
        assert!(focus.main_layer_acquiring());

        assert!(focus.expire_keyboard_reclaim(now + KEYBOARD_RECLAIM_WINDOW));
        assert!(!focus.main_layer_acquiring());
        assert_eq!(focus.keyboard_reclaim_timeout(now), None);
    }

    #[test]
    fn focus_exit_window_expires_at_its_deadline() {
        let now = Instant::now();
        let mut focus = FocusState::new(None);
        focus.suppress_exit_for(now, Duration::from_millis(20));

        assert!(focus.exit_suppressed(now + Duration::from_millis(20)));
        assert!(focus.exit_suppression_expired(now + Duration::from_millis(20)));
        assert_eq!(focus.exit_timeout(now + Duration::from_millis(20)), None);
    }

    #[test]
    fn close_guard_is_active_at_deadline_and_inactive_after() {
        let now = Instant::now();
        let mut focus = FocusState::new(None);
        focus.guard_xdg_close_for(now, Duration::from_millis(20));

        assert!(focus.xdg_close_guard_active(now + Duration::from_millis(20)));
        assert!(!focus.xdg_close_guard_active(now + Duration::from_millis(21)));
    }

    #[test]
    fn explicit_close_is_one_shot() {
        let mut focus = FocusState::new(None);
        focus.mark_xdg_explicit_close_requested();

        assert!(focus.take_xdg_explicit_close_requested());
        assert!(!focus.take_xdg_explicit_close_requested());
    }

    #[test]
    fn ignores_close_only_for_unfocused_stay_with_active_guard() {
        let now = Instant::now();
        let mut focus = FocusState::new(None);
        focus.guard_xdg_close_for(now, Duration::from_millis(20));

        assert!(focus.ignores_xdg_close(true, now));
        focus.keyboard_entered();
        assert!(!focus.ignores_xdg_close(true, now));
        focus.keyboard_left();
        assert!(!focus.ignores_xdg_close(false, now));
        assert!(!focus.ignores_xdg_close(true, now + Duration::from_millis(21)));
    }

    #[test]
    fn startup_token_is_taken_once() {
        let mut focus = FocusState::new(Some("startup".to_string()));

        assert_eq!(
            focus.take_startup_activation_token().as_deref(),
            Some("startup")
        );
        assert_eq!(focus.take_startup_activation_token(), None);
    }

    #[test]
    fn readiness_requires_keyboard_focus() {
        let mut focus = FocusState::new(None);

        assert!(!focus.mark_ready_if_focused());
        assert!(!focus.is_ready());
        focus.keyboard_entered();
        assert!(focus.mark_ready_if_focused());
        assert!(!focus.mark_ready_if_focused());
        assert!(focus.is_ready());
    }

    #[test]
    fn main_layer_phase_completes_once_and_restarts() {
        let mut focus = FocusState::new(None);
        focus.set_keyboard_interactivity(Some(KeyboardInteractivity::Exclusive));

        assert!(focus.can_complete_main_layer_acquisition(true, false));
        assert!(focus.complete_main_layer_acquisition());
        assert!(!focus.main_layer_acquiring());
        assert!(!focus.complete_main_layer_acquisition());

        focus.begin_main_layer_acquisition();

        assert!(focus.main_layer_acquiring());
        assert!(focus.can_complete_main_layer_acquisition(true, false));
    }

    #[test]
    fn main_layer_completion_requires_current_exclusive_surface_without_release() {
        let mut focus = FocusState::new(None);
        focus.set_keyboard_interactivity(Some(KeyboardInteractivity::Exclusive));

        assert!(!focus.can_complete_main_layer_acquisition(false, false));
        assert!(!focus.can_complete_main_layer_acquisition(true, true));
        focus.set_keyboard_interactivity(Some(KeyboardInteractivity::OnDemand));
        assert!(!focus.can_complete_main_layer_acquisition(true, false));
        focus.set_keyboard_interactivity(None);
        assert!(!focus.can_complete_main_layer_acquisition(true, false));
    }

    #[test]
    fn keyboard_teardown_keeps_acquired_main_layer_out_of_acquisition() {
        let mut focus = FocusState::new(None);
        assert!(focus.complete_main_layer_acquisition());

        focus.keyboard_left();

        assert!(!focus.main_layer_acquiring());
    }
}
