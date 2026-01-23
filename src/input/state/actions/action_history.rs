use crate::config::Action;

use super::super::InputState;

impl InputState {
    pub(super) fn handle_history_action(&mut self, action: Action) -> bool {
        match action {
            Action::Undo => {
                if let Some(action) = self.boards.active_frame_mut().undo_last() {
                    self.apply_action_side_effects(&action);
                } else {
                    // Nothing to undo - show blocked feedback
                    self.trigger_blocked_feedback();
                }
                true
            }
            Action::Redo => {
                if let Some(action) = self.boards.active_frame_mut().redo_last() {
                    self.apply_action_side_effects(&action);
                } else {
                    // Nothing to redo - show blocked feedback
                    self.trigger_blocked_feedback();
                }
                true
            }
            Action::UndoAll => {
                self.undo_all_immediate();
                true
            }
            Action::RedoAll => {
                self.redo_all_immediate();
                true
            }
            Action::UndoAllDelayed => {
                self.start_undo_all_delayed(self.undo_all_delay_ms);
                true
            }
            Action::RedoAllDelayed => {
                self.start_redo_all_delayed(self.redo_all_delay_ms);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn undo_all_immediate(&mut self) {
        while let Some(action) = self.boards.active_frame_mut().undo_last() {
            self.apply_action_side_effects(&action);
        }
    }

    pub(crate) fn redo_all_immediate(&mut self) {
        while let Some(action) = self.boards.active_frame_mut().redo_last() {
            self.apply_action_side_effects(&action);
        }
    }
}
