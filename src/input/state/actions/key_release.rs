use crate::input::Modifiers;
use crate::input::events::Key;

use super::super::InputState;

impl InputState {
    /// Processes a key release event.
    ///
    /// Currently only tracks modifier key releases to update the modifier state.
    pub fn on_key_release(&mut self, key: Key) {
        self.release_command_palette_repeat_key(key);
        self.release_font_picker_repeat_key(key);
        let release: fn(&mut Modifiers) = match key {
            Key::Shift => |modifiers| modifiers.shift = false,
            Key::Ctrl => |modifiers| modifiers.ctrl = false,
            Key::Alt => |modifiers| modifiers.alt = false,
            Key::Super => |modifiers| modifiers.logo = false,
            Key::Tab => |modifiers| modifiers.tab = false,
            _ => return,
        };
        self.update_modifiers(release);
    }
}
