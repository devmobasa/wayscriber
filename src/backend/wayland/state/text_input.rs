use wayland_client::{QueueHandle, protocol::wl_seat};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::ZwpTextInputManagerV3, zwp_text_input_v3::ZwpTextInputV3,
};

use super::WaylandState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalTransition {
    EnableCommitted,
    DisableCommitted,
    Leave,
}

/// Runtime ownership and compositor synchronization state for text-input-v3.
pub(in crate::backend::wayland) struct TextInputState {
    manager: Option<ZwpTextInputManagerV3>,
    protocol: Option<ZwpTextInputV3>,
    seat: Option<wl_seat::WlSeat>,
    focused: bool,
    enabled: bool,
    serial: u32,
    cursor_update_pending: bool,
    external_change_pending: bool,
    cursor_update_blocked_until: Option<u32>,
}

impl TextInputState {
    pub(super) fn new(manager: Option<ZwpTextInputManagerV3>) -> Self {
        Self {
            manager,
            protocol: None,
            seat: None,
            focused: false,
            enabled: false,
            serial: 0,
            cursor_update_pending: false,
            external_change_pending: false,
            cursor_update_blocked_until: None,
        }
    }

    pub(in crate::backend::wayland) fn attach_if_absent(
        &mut self,
        seat: &wl_seat::WlSeat,
        qh: &QueueHandle<WaylandState>,
    ) -> bool {
        if self.protocol.is_some() {
            return false;
        }
        let Some(manager) = &self.manager else {
            return false;
        };
        self.protocol = Some(manager.get_text_input(seat, qh, ()));
        self.seat = Some(seat.clone());
        self.reset_protocol_state();
        true
    }

    pub(in crate::backend::wayland) fn detach_if_owned(
        &mut self,
        removed_seat: &wl_seat::WlSeat,
    ) -> bool {
        if self.seat.as_ref() != Some(removed_seat) {
            return false;
        }
        if let Some(protocol) = self.protocol.take() {
            protocol.destroy();
        }
        self.seat = None;
        self.reset_protocol_state();
        true
    }

    pub(in crate::backend::wayland) fn protocol(&self) -> Option<ZwpTextInputV3> {
        self.protocol.clone()
    }

    pub(in crate::backend::wayland) fn enter(&mut self) {
        self.focused = true;
    }

    pub(in crate::backend::wayland) fn leave(&mut self) {
        self.focused = false;
        self.apply_transition(LocalTransition::Leave);
    }

    pub(in crate::backend::wayland) fn is_focused(&self) -> bool {
        self.focused
    }

    pub(in crate::backend::wayland) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(in crate::backend::wayland) fn enabled_committed(&mut self) {
        self.apply_transition(LocalTransition::EnableCommitted);
    }

    pub(in crate::backend::wayland) fn disabled_committed(&mut self) {
        self.apply_transition(LocalTransition::DisableCommitted);
    }

    pub(in crate::backend::wayland) fn collect_editor_changes(
        &mut self,
        cursor_dirty: bool,
        external_change: bool,
    ) {
        self.cursor_update_pending |= cursor_dirty;
        self.external_change_pending |= external_change;
    }

    pub(in crate::backend::wayland) fn cursor_update_ready_after_done(
        &mut self,
        editor_changed: bool,
        done_serial: u32,
    ) -> bool {
        self.cursor_update_pending |= editor_changed;
        self.cursor_update_blocked_until = (done_serial != self.serial).then_some(self.serial);
        self.cursor_update_ready()
    }

    pub(in crate::backend::wayland) fn cursor_update_ready(&self) -> bool {
        self.cursor_update_pending && self.enabled && self.cursor_update_blocked_until.is_none()
    }

    pub(in crate::backend::wayland) fn external_change_pending(&self) -> bool {
        self.external_change_pending
    }

    pub(in crate::backend::wayland) fn cursor_update_committed(&mut self) {
        self.serial = self.serial.wrapping_add(1);
        self.cursor_update_pending = false;
        self.external_change_pending = false;
    }

    fn apply_transition(&mut self, transition: LocalTransition) {
        self.enabled = matches!(transition, LocalTransition::EnableCommitted);
        if transition != LocalTransition::Leave {
            self.serial = self.serial.wrapping_add(1);
        }
        self.cursor_update_pending = false;
        self.external_change_pending = false;
        self.cursor_update_blocked_until = None;
    }

    fn reset_protocol_state(&mut self) {
        self.focused = false;
        self.enabled = false;
        self.serial = 0;
        self.cursor_update_pending = false;
        self.external_change_pending = false;
        self.cursor_update_blocked_until = None;
    }
}

#[cfg(test)]
mod tests {
    use super::TextInputState;

    fn state() -> TextInputState {
        TextInputState::new(None)
    }

    #[test]
    fn leave_preserves_the_last_compositor_visible_commit_serial() {
        let mut state = state();
        state.enabled_committed();
        state.enabled_committed();
        state.enabled_committed();
        state.enabled_committed();
        state.collect_editor_changes(true, true);

        state.leave();

        assert!(!state.is_enabled());
        assert!(!state.cursor_update_pending);
        assert!(!state.external_change_pending);
        assert_eq!(state.cursor_update_blocked_until, None);
        assert_eq!(state.serial, 4);

        state.enabled_committed();
        assert_eq!(state.serial, 5);
    }

    #[test]
    fn stale_done_defers_cursor_update_until_a_matching_serial() {
        let mut state = state();
        state.enabled_committed();
        state.enabled_committed();
        state.enabled_committed();

        assert!(!state.cursor_update_ready_after_done(true, 2));
        assert!(state.cursor_update_pending);
        assert!(state.cursor_update_ready_after_done(false, 3));
    }

    #[test]
    fn disabled_text_input_retains_update_until_it_can_be_reconciled() {
        let mut state = state();

        assert!(!state.cursor_update_ready_after_done(true, 0));
        assert!(state.cursor_update_pending);
    }

    #[test]
    fn bare_caret_move_is_ready_without_waiting_for_done() {
        let mut state = state();
        state.enabled_committed();
        state.collect_editor_changes(true, false);

        assert!(state.cursor_update_ready());
    }

    #[test]
    fn stale_done_blocks_bare_caret_updates_until_the_matching_serial() {
        let mut state = state();
        state.enabled_committed();
        state.enabled_committed();
        state.enabled_committed();

        assert!(!state.cursor_update_ready_after_done(true, 2));
        state.collect_editor_changes(true, false);
        assert!(!state.cursor_update_ready());

        assert!(state.cursor_update_ready_after_done(false, 3));
    }
}
