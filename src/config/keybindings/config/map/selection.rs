use super::super::KeybindingsConfig;
use super::BindingInserter;
use crate::config::Action;

impl KeybindingsConfig {
    pub(super) fn insert_selection_bindings(
        &self,
        inserter: &mut BindingInserter,
    ) -> Result<(), String> {
        inserter.insert_all(&self.duplicate_selection, Action::DuplicateSelection)?;
        inserter.insert_all(&self.copy_selection, Action::CopySelection)?;
        inserter.insert_all(&self.paste_selection, Action::PasteSelection)?;
        inserter.insert_all(&self.select_all, Action::SelectAll)?;
        inserter.insert_all(&self.move_selection_to_front, Action::MoveSelectionToFront)?;
        inserter.insert_all(&self.move_selection_to_back, Action::MoveSelectionToBack)?;
        inserter.insert_all(&self.nudge_selection_up, Action::NudgeSelectionUp)?;
        inserter.insert_all(&self.nudge_selection_down, Action::NudgeSelectionDown)?;
        inserter.insert_all(&self.nudge_selection_left, Action::NudgeSelectionLeft)?;
        inserter.insert_all(&self.nudge_selection_right, Action::NudgeSelectionRight)?;
        inserter.insert_all(
            &self.nudge_selection_up_large,
            Action::NudgeSelectionUpLarge,
        )?;
        inserter.insert_all(
            &self.nudge_selection_down_large,
            Action::NudgeSelectionDownLarge,
        )?;
        inserter.insert_all(&self.move_selection_to_start, Action::MoveSelectionToStart)?;
        inserter.insert_all(&self.move_selection_to_end, Action::MoveSelectionToEnd)?;
        inserter.insert_all(&self.move_selection_to_top, Action::MoveSelectionToTop)?;
        inserter.insert_all(
            &self.move_selection_to_bottom,
            Action::MoveSelectionToBottom,
        )?;
        inserter.insert_all(&self.delete_selection, Action::DeleteSelection)?;
        Ok(())
    }
}
