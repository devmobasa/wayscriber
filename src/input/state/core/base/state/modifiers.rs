use super::super::types::DrawingState;
use super::structs::InputState;

impl InputState {
    /// Resets all tracked keyboard modifiers to the "released" state.
    ///
    /// This is used as a safety net when external UI (portals, other windows)
    /// or focus transitions may cause us to miss key release events from
    /// the compositor, which would otherwise leave modifiers "stuck" and break
    /// shortcut handling and tool selection.
    pub fn reset_modifiers(&mut self) {
        self.modifiers.shift = false;
        self.modifiers.ctrl = false;
        self.modifiers.alt = false;
        self.modifiers.logo = false;
        self.modifiers.tab = false;
        self.keymap.clear_consumed_pointer_buttons();
        self.clear_pending_sequence();
        if matches!(self.state, DrawingState::Idle) {
            self.sync_current_settings_from_active_tool();
        }
    }

    /// Clears key state whose release can be lost with keyboard focus.
    ///
    /// The backend owns its own repeat and board-pan latches; this is the
    /// `InputState` half shared by protocol focus leave and synthetic focus
    /// loss during layer-output recreation.
    pub(crate) fn clear_focus_owned_key_state(&mut self) {
        self.reset_modifiers();
        self.clear_command_palette_repeat();
        self.clear_font_picker_repeat();
    }

    /// Synchronize modifier state from backend-provided values (e.g. compositor).
    ///
    /// This lets us correct cases where a key release event was missed but the compositor's
    /// authoritative modifier state is still accurate.
    pub fn sync_modifiers(&mut self, shift: bool, ctrl: bool, alt: bool, logo: bool) {
        self.modifiers.shift = shift;
        self.modifiers.ctrl = ctrl;
        self.modifiers.alt = alt;
        self.modifiers.logo = logo;
        // Tab has no direct compositor flag; leave it unchanged.
        if matches!(self.state, DrawingState::Idle) {
            self.sync_current_settings_from_active_tool();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use crate::input::Key;
    use crate::input::state::test_support::make_test_input_state;

    #[test]
    fn focus_loss_clears_modal_repeats_and_modifiers() {
        let mut state = make_test_input_state();
        state.toggle_command_palette();
        assert!(state.handle_command_palette_key(Key::Down));
        state.sync_modifiers(true, true, true, true);
        state.modifiers.tab = true;
        assert!(
            state
                .command_palette_repeat_timeout(Instant::now())
                .is_some()
        );

        state.clear_focus_owned_key_state();

        assert!(
            state
                .command_palette_repeat_timeout(Instant::now())
                .is_none()
        );
        assert!(!state.modifiers.shift);
        assert!(!state.modifiers.ctrl);
        assert!(!state.modifiers.alt);
        assert!(!state.modifiers.logo);
        assert!(!state.modifiers.tab);

        state.open_font_picker();
        assert!(state.handle_font_picker_key(Key::Down, None));
        assert!(state.font_picker_repeat_timeout(Instant::now()).is_some());

        state.clear_focus_owned_key_state();

        assert!(state.font_picker_repeat_timeout(Instant::now()).is_none());
    }
}
