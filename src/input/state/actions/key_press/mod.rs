pub(in crate::input::state) mod bindings;
pub(in crate::input::state) mod caret_edit;
mod panels;
mod text_input;

use crate::input::Modifiers;
use crate::input::events::Key;

use super::super::{InputState, interaction};

impl InputState {
    pub(in crate::input::state) fn handle_modifier_key_press(&mut self, key: Key) -> bool {
        let press: fn(&mut Modifiers) = match key {
            Key::Shift => |modifiers| modifiers.shift = true,
            Key::Ctrl => |modifiers| modifiers.ctrl = true,
            Key::Alt => |modifiers| modifiers.alt = true,
            Key::Super => |modifiers| modifiers.logo = true,
            Key::Tab => |modifiers| modifiers.tab = true,
            _ => return false,
        };
        self.update_modifiers(press);
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
        let measurer = crate::draw::TextMeasurer::default();
        let ui_engine = crate::ui_text::UiTextEngine::default();
        self.on_key_press_with_resources(
            crate::input::state::InputTextResources {
                measurer: &measurer,
                ui_engine: &ui_engine,
            },
            key,
        );
    }

    pub(crate) fn on_key_press_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        key: Key,
    ) {
        let _ = interaction::route_key_press_with_resources(self, resources, key);
    }

    pub fn on_key_repeat(&mut self, key: Key) {
        let measurer = crate::draw::TextMeasurer::default();
        let ui_engine = crate::ui_text::UiTextEngine::default();
        self.on_key_repeat_with_resources(
            crate::input::state::InputTextResources {
                measurer: &measurer,
                ui_engine: &ui_engine,
            },
            key,
        );
    }

    pub(crate) fn on_key_repeat_with_resources(
        &mut self,
        resources: crate::input::state::InputTextResources<'_>,
        key: Key,
    ) {
        let _ = interaction::route_key_repeat_with_resources(self, resources, key);
    }
}
