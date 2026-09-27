use crate::domain::Action;
use crate::draw::Frame;

use super::super::InputState;

impl InputState {
    pub(in crate::input::state) fn handle_history_action_with_measurer(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
        action: Action,
    ) -> bool {
        match action {
            Action::Undo => {
                if self.step_history_with(measurer, Frame::undo_last) {
                    self.pending_onboarding_usage.first_undo_done = true;
                } else {
                    // Nothing to undo - show blocked feedback
                    self.trigger_blocked_feedback();
                }
                true
            }
            Action::Redo => {
                if !self.step_history_with(measurer, Frame::redo_last) {
                    // Nothing to redo - show blocked feedback
                    self.trigger_blocked_feedback();
                }
                true
            }
            Action::UndoAll => {
                self.undo_all_immediate_with_measurer(measurer);
                true
            }
            Action::RedoAll => {
                self.redo_all_immediate_with_measurer(measurer);
                true
            }
            Action::UndoAllDelayed => {
                self.start_undo_all_delayed(self.history_limits.undo_all_delay_ms());
                true
            }
            Action::RedoAllDelayed => {
                self.start_redo_all_delayed(self.history_limits.redo_all_delay_ms());
                true
            }
            _ => false,
        }
    }

    pub(crate) fn undo_all_immediate_with_measurer(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
    ) {
        while self.step_history_with(measurer, Frame::undo_last) {}
    }

    pub(crate) fn redo_all_immediate_with_measurer(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
    ) {
        while self.step_history_with(measurer, Frame::redo_last) {}
    }
}
