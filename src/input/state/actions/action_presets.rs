use crate::config::Action;

use super::super::InputState;

impl InputState {
    pub(super) fn handle_preset_action(&mut self, action: Action) -> bool {
        match action {
            Action::ApplyPreset1 => {
                let _ = self.apply_preset(1);
            }
            Action::ApplyPreset2 => {
                let _ = self.apply_preset(2);
            }
            Action::ApplyPreset3 => {
                let _ = self.apply_preset(3);
            }
            Action::ApplyPreset4 => {
                let _ = self.apply_preset(4);
            }
            Action::ApplyPreset5 => {
                let _ = self.apply_preset(5);
            }
            Action::SavePreset1 => {
                let _ = self.save_preset(1);
            }
            Action::SavePreset2 => {
                let _ = self.save_preset(2);
            }
            Action::SavePreset3 => {
                let _ = self.save_preset(3);
            }
            Action::SavePreset4 => {
                let _ = self.save_preset(4);
            }
            Action::SavePreset5 => {
                let _ = self.save_preset(5);
            }
            Action::ClearPreset1 => {
                let _ = self.clear_preset(1);
            }
            Action::ClearPreset2 => {
                let _ = self.clear_preset(2);
            }
            Action::ClearPreset3 => {
                let _ = self.clear_preset(3);
            }
            Action::ClearPreset4 => {
                let _ = self.clear_preset(4);
            }
            Action::ClearPreset5 => {
                let _ = self.clear_preset(5);
            }
            _ => return false,
        }

        true
    }
}
