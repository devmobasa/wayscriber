use crate::config::Action;
use crate::draw::Color;
use crate::input::Tool;
use crate::util;

use super::super::InputState;

impl InputState {
    pub(super) fn handle_color_action(&mut self, action: Action) -> bool {
        let Some(color) = action_color(action) else {
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

fn action_color(action: Action) -> Option<Color> {
    match action {
        Action::SetColorRed => util::key_to_color('r'),
        Action::SetColorGreen => util::key_to_color('g'),
        Action::SetColorBlue => util::key_to_color('b'),
        Action::SetColorYellow => util::key_to_color('y'),
        Action::SetColorOrange => util::key_to_color('o'),
        Action::SetColorPink => util::key_to_color('p'),
        Action::SetColorWhite => util::key_to_color('w'),
        Action::SetColorBlack => util::key_to_color('k'),
        _ => None,
    }
}
