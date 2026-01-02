mod bindings;
mod panels;
mod text_input;

use crate::config::Action;
use crate::input::events::Key;
use log::info;

use super::super::{DrawingState, InputState};
use bindings::key_to_action_label;

impl InputState {
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
        info!(
            "input key press: key={:?} state={:?} mods(ctrl={},shift={},alt={},tab={}) help={} board_picker={} properties_panel={} context_menu={} selection={}",
            key,
            self.state,
            self.modifiers.ctrl,
            self.modifiers.shift,
            self.modifiers.alt,
            self.modifiers.tab,
            self.show_help,
            self.is_board_picker_open(),
            self.is_properties_panel_open(),
            self.is_context_menu_open(),
            self.has_selection()
        );
        if self.show_help && self.handle_help_overlay_key(key) {
            info!("input key handled by help overlay");
            return;
        }

        if self.is_board_picker_open() && self.handle_board_picker_key(key) {
            info!("input key handled by board picker");
            return;
        }

        // Handle modifier keys first
        match key {
            Key::Shift => {
                self.modifiers.shift = true;
                info!("input modifier shift set true");
                return;
            }
            Key::Ctrl => {
                self.modifiers.ctrl = true;
                info!("input modifier ctrl set true");
                return;
            }
            Key::Alt => {
                self.modifiers.alt = true;
                info!("input modifier alt set true");
                return;
            }
            Key::Tab => {
                self.modifiers.tab = true;
                info!("input modifier tab set true");
                return;
            }
            _ => {}
        }

        if self.is_properties_panel_open() {
            let handled = self.handle_properties_panel_key(key);
            if handled {
                info!("input key handled by properties panel");
                return;
            }
            return;
        }

        if self.is_context_menu_open() {
            let handled = self.handle_context_menu_key(key);
            if handled {
                info!("input key handled by context menu");
                return;
            }
        }

        if matches!(key, Key::Escape)
            && matches!(self.state, DrawingState::Idle)
            && self.has_selection()
        {
            let bounds = self.selection_bounding_box(self.selected_shape_ids());
            self.clear_selection();
            self.mark_selection_dirty_region(bounds);
            self.needs_redraw = true;
            info!("input escape cleared selection");
            return;
        }

        if matches!(&self.state, DrawingState::TextInput { .. }) {
            self.handle_text_input_key(key);
            info!("input key handled by text input");
            return;
        }

        // Handle Escape in Drawing state for canceling
        if matches!(key, Key::Escape)
            && let DrawingState::Drawing { .. } = &self.state
            && let Some(Action::Exit) = self.find_action("Escape")
        {
            self.state = DrawingState::Idle;
            self.needs_redraw = true;
            info!("input escape canceled drawing state");
            return;
        }

        // Convert key to string for action lookup
        let Some(key_str) = key_to_action_label(key) else {
            info!("input key has no action label");
            return;
        };

        // Look up action based on keybinding
        if let Some(action) = self.find_action(&key_str) {
            info!("input action: key_str={} action={:?}", key_str, action);
            self.handle_action(action);
            return;
        }
        info!("input key no action: key_str={}", key_str);

        if matches!(key, Key::Return)
            && !self.modifiers.ctrl
            && !self.modifiers.shift
            && !self.modifiers.alt
            && matches!(self.state, DrawingState::Idle)
            && self.edit_selected_text()
        {}
    }
}
