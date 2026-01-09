use crate::input::events::Key;

use super::super::InputState;
use super::key_press::bindings::key_to_action_label;

impl InputState {
    /// Processes a key release event.
    ///
    /// Currently only tracks modifier key releases to update the modifier state.
    pub fn on_key_release(&mut self, key: Key) {
        match key {
            Key::Shift => self.modifiers.shift = false,
            Key::Ctrl => self.modifiers.ctrl = false,
            Key::Alt => self.modifiers.alt = false,
            Key::Tab => self.modifiers.tab = false,
            _ => {}
        }

        if let Some(key_str) = key_to_action_label(key) {
            self.remove_hold_to_draw_key(&key_str);
        }
    }
}
