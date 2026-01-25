use crate::config::Action;
use crate::draw::Color;
use crate::input::Tool;
use crate::util;

use super::super::InputState;

impl InputState {
    pub(super) fn handle_color_action(&mut self, action: Action) -> bool {
        match action {
            Action::SetColorRed => {
                let _ = self.apply_color_from_ui(util::key_to_color('r').unwrap());
            }
            Action::SetColorGreen => {
                let _ = self.apply_color_from_ui(util::key_to_color('g').unwrap());
            }
            Action::SetColorBlue => {
                let _ = self.apply_color_from_ui(util::key_to_color('b').unwrap());
            }
            Action::SetColorYellow => {
                let _ = self.apply_color_from_ui(util::key_to_color('y').unwrap());
            }
            Action::SetColorOrange => {
                let _ = self.apply_color_from_ui(util::key_to_color('o').unwrap());
            }
            Action::SetColorPink => {
                let _ = self.apply_color_from_ui(util::key_to_color('p').unwrap());
            }
            Action::SetColorWhite => {
                let _ = self.apply_color_from_ui(util::key_to_color('w').unwrap());
            }
            Action::SetColorBlack => {
                let _ = self.apply_color_from_ui(util::key_to_color('k').unwrap());
            }
            _ => return false,
        }

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
