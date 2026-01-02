use crate::input::events::Key;
use log::info;

use super::super::InputState;

impl InputState {
    /// Processes a key release event.
    ///
    /// Currently only tracks modifier key releases to update the modifier state.
    pub fn on_key_release(&mut self, key: Key) {
        info!(
            "input key release: key={:?} mods(ctrl={},shift={},alt={},tab={})",
            key, self.modifiers.ctrl, self.modifiers.shift, self.modifiers.alt, self.modifiers.tab
        );
        match key {
            Key::Shift => {
                self.modifiers.shift = false;
                info!("input modifier shift set false");
            }
            Key::Ctrl => {
                self.modifiers.ctrl = false;
                info!("input modifier ctrl set false");
            }
            Key::Alt => {
                self.modifiers.alt = false;
                info!("input modifier alt set false");
            }
            Key::Tab => {
                self.modifiers.tab = false;
                info!("input modifier tab set false");
            }
            _ => {}
        }
    }
}
