pub(in crate::input::state) mod bindings;
mod panels;
mod text_input;

use crate::input::events::Key;

use super::super::{DrawingState, InputState, interaction};

impl InputState {
    pub(in crate::input::state) fn handle_modifier_key_press(&mut self, key: Key) -> bool {
        match key {
            Key::Shift => self.modifiers.shift = true,
            Key::Ctrl => self.modifiers.ctrl = true,
            Key::Alt => self.modifiers.alt = true,
            Key::Tab => self.modifiers.tab = true,
            _ => return false,
        }
        if matches!(self.state, DrawingState::Idle) {
            self.sync_current_settings_from_active_tool();
        }
        true
    }

    /// Processes a key press event.
    ///
    /// Handles all keyboard input including:
    /// - Drawing color selection (configurable keybindings)
    /// - Tool actions (text mode, clear, undo - configurable)
    /// - Text input (when in TextInput state)
    /// - Exit commands (configurable)
    /// - Thickness adjustment (configurable)
    /// - Help toggle (configurable)
    /// - Modifier key tracking
    pub fn on_key_press(&mut self, key: Key) {
        let _ = interaction::route_key_press(self, key);
    }
}
