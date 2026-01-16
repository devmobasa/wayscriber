mod bindings;
mod panels;
mod text_input;

use crate::config::Action;
use crate::input::events::Key;

use super::super::{DrawingState, InputState};
use bindings::{fallback_unshifted_label, key_to_action_label};

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
        // Tour takes highest priority when active
        if self.tour_active && self.handle_tour_key(key) {
            return;
        }

        // Command palette takes priority
        if self.command_palette_open && self.handle_command_palette_key(key) {
            return;
        }

        if self.show_help && self.handle_help_overlay_key(key) {
            return;
        }

        if self.is_board_picker_open() && self.handle_board_picker_key(key) {
            return;
        }

        // Handle modifier keys first
        match key {
            Key::Shift => {
                self.modifiers.shift = true;
                return;
            }
            Key::Ctrl => {
                self.modifiers.ctrl = true;
                return;
            }
            Key::Alt => {
                self.modifiers.alt = true;
                return;
            }
            Key::Tab => {
                self.modifiers.tab = true;
                return;
            }
            _ => {}
        }

        if self.is_properties_panel_open() {
            let handled = self.handle_properties_panel_key(key);
            if handled {
                return;
            }
            return;
        }

        if self.is_context_menu_open() {
            let handled = self.handle_context_menu_key(key);
            if handled {
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
            return;
        }

        if matches!(&self.state, DrawingState::TextInput { .. }) {
            self.handle_text_input_key(key);
            return;
        }

        // Handle Escape in Drawing state for canceling
        if matches!(key, Key::Escape)
            && let DrawingState::Drawing { .. } = &self.state
            && let Some(Action::Exit) = self.find_action("Escape")
        {
            self.state = DrawingState::Idle;
            self.needs_redraw = true;
            return;
        }

        // Convert key to string for action lookup
        let Some(key_str) = key_to_action_label(key) else {
            return;
        };

        // Look up action based on keybinding
        if let Some(action) = self.find_action(&key_str) {
            self.handle_action(action);
            return;
        }
        if self.modifiers.shift
            && let Some(fallback) = fallback_unshifted_label(&key_str)
            && let Some(action) = self.find_action(fallback)
        {
            self.handle_action(action);
            return;
        }

        if matches!(key, Key::Return)
            && !self.modifiers.ctrl
            && !self.modifiers.shift
            && !self.modifiers.alt
            && matches!(self.state, DrawingState::Idle)
            && self.edit_selected_text()
        {}
    }
}
