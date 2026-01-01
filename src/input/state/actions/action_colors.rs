use crate::config::Action;
use crate::util;

use super::super::InputState;

impl InputState {
    pub(super) fn handle_color_action(&mut self, action: Action) -> bool {
        match action {
            Action::SetColorRed => {
                let _ = self.set_color(util::key_to_color('r').unwrap());
            }
            Action::SetColorGreen => {
                let _ = self.set_color(util::key_to_color('g').unwrap());
            }
            Action::SetColorBlue => {
                let _ = self.set_color(util::key_to_color('b').unwrap());
            }
            Action::SetColorYellow => {
                let _ = self.set_color(util::key_to_color('y').unwrap());
            }
            Action::SetColorOrange => {
                let _ = self.set_color(util::key_to_color('o').unwrap());
            }
            Action::SetColorPink => {
                let _ = self.set_color(util::key_to_color('p').unwrap());
            }
            Action::SetColorWhite => {
                let _ = self.set_color(util::key_to_color('w').unwrap());
            }
            Action::SetColorBlack => {
                let _ = self.set_color(util::key_to_color('k').unwrap());
            }
            _ => return false,
        }

        true
    }
}
