use crate::input::events::Key;

use super::super::{DrawingState, InputState};

impl InputState {
    /// Processes a key release event.
    ///
    /// Currently only tracks modifier key releases to update the modifier state.
    pub fn on_key_release(&mut self, key: Key) {
        let was_modifier = matches!(key, Key::Shift | Key::Ctrl | Key::Alt | Key::Tab);
        match key {
            Key::Shift => self.modifiers.shift = false,
            Key::Ctrl => self.modifiers.ctrl = false,
            Key::Alt => self.modifiers.alt = false,
            Key::Tab => self.modifiers.tab = false,
            _ => {}
        }
        if was_modifier && matches!(self.state, DrawingState::Idle) {
            self.sync_current_settings_from_active_tool();
        }
    }
}
