use crate::config::Action;
use crate::config::QuickColorPalette;
use crate::draw::Color;
use crate::input::Tool;

use super::super::InputState;

impl InputState {
    pub fn set_quick_colors(&mut self, quick_colors: QuickColorPalette) {
        self.quick_colors = quick_colors;
    }

    pub(in crate::input::state) fn handle_color_action(&mut self, action: Action) -> bool {
        if action == Action::PickScreenColor {
            self.request_eyedropper_toggle();
            return true;
        }
        let Some(color) = self.quick_colors.color_for_action(action) else {
            return false;
        };
        let _ = self.apply_color_from_ui(color);
        true
    }

    pub(crate) fn apply_color_from_ui(&mut self, color: Color) -> bool {
        let mut changed = self.set_color(color);
        if self.active_tool() == Tool::Select && !self.selected_shape_ids().is_empty() {
            let selection_changed = self.apply_selection_color_value(color);
            changed = selection_changed || changed;
        }
        changed
    }

    /// Take and clear the pending copy hex color request.
    pub fn take_pending_copy_hex(&mut self) -> bool {
        std::mem::take(&mut self.pending_copy_hex)
    }

    /// Take and clear the pending paste hex color request.
    pub fn take_pending_paste_hex(&mut self) -> bool {
        std::mem::take(&mut self.pending_paste_hex)
    }
}
