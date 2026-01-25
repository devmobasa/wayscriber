use crate::config::Action;
use log::warn;

use super::super::{InputState, UiToastKind};

impl InputState {
    /// Handle an action triggered by a keybinding.
    pub(crate) fn handle_action(&mut self, action: Action) {
        if !matches!(
            action,
            Action::OpenContextMenu | Action::ToggleSelectionProperties
        ) {
            self.close_properties_panel();
        }

        if matches!(action, Action::PickScreenColorDeprecated) {
            warn!("Deprecated action pick_screen_color triggered; ignoring.");
            self.set_ui_toast(
                UiToastKind::Warning,
                "Pick screen color was removed. Use the palette or paste a hex value.",
            );
            return;
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
