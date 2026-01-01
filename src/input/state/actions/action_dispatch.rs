use crate::config::Action;

use super::super::InputState;

impl InputState {
    /// Handle an action triggered by a keybinding.
    pub(crate) fn handle_action(&mut self, action: Action) {
        if !matches!(
            action,
            Action::OpenContextMenu | Action::ToggleSelectionProperties
        ) {
            self.close_properties_panel();
        }

        if self.handle_core_action(action) {
            return;
        }
        if self.handle_history_action(action) {
            return;
        }
        if self.handle_selection_action(action) {
            return;
        }
        if self.handle_tool_action(action) {
            return;
        }
        if self.handle_board_pages_action(action) {
            return;
        }
        if self.handle_ui_action(action) {
            return;
        }
        if self.handle_color_action(action) {
            return;
        }
        if self.handle_capture_zoom_action(action) {
            return;
        }
        self.handle_preset_action(action);
    }
}
