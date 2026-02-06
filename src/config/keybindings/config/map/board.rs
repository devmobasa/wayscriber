use super::super::KeybindingsConfig;
use super::BindingInserter;
use crate::config::Action;

impl KeybindingsConfig {
    pub(super) fn insert_board_bindings(
        &self,
        inserter: &mut BindingInserter,
    ) -> Result<(), String> {
        inserter.insert_all(&self.board.toggle_whiteboard, Action::ToggleWhiteboard)?;
        inserter.insert_all(&self.board.toggle_blackboard, Action::ToggleBlackboard)?;
        inserter.insert_all(
            &self.board.return_to_transparent,
            Action::ReturnToTransparent,
        )?;
        inserter.insert_all(&self.board.page_prev, Action::PagePrev)?;
        inserter.insert_all(&self.board.page_next, Action::PageNext)?;
        inserter.insert_all(&self.board.page_new, Action::PageNew)?;
        inserter.insert_all(&self.board.page_duplicate, Action::PageDuplicate)?;
        inserter.insert_all(&self.board.page_delete, Action::PageDelete)?;
        inserter.insert_all(&self.board.board_1, Action::Board1)?;
        inserter.insert_all(&self.board.board_2, Action::Board2)?;
        inserter.insert_all(&self.board.board_3, Action::Board3)?;
        inserter.insert_all(&self.board.board_4, Action::Board4)?;
        inserter.insert_all(&self.board.board_5, Action::Board5)?;
        inserter.insert_all(&self.board.board_6, Action::Board6)?;
        inserter.insert_all(&self.board.board_7, Action::Board7)?;
        inserter.insert_all(&self.board.board_8, Action::Board8)?;
        inserter.insert_all(&self.board.board_9, Action::Board9)?;
        inserter.insert_all(&self.board.board_next, Action::BoardNext)?;
        inserter.insert_all(&self.board.board_prev, Action::BoardPrev)?;
        inserter.insert_all(&self.board.focus_next_output, Action::FocusNextOutput)?;
        inserter.insert_all(&self.board.focus_prev_output, Action::FocusPrevOutput)?;
        inserter.insert_all(&self.board.board_new, Action::BoardNew)?;
        inserter.insert_all(&self.board.board_duplicate, Action::BoardDuplicate)?;
        inserter.insert_all(&self.board.board_delete, Action::BoardDelete)?;
        inserter.insert_all(&self.board.board_picker, Action::BoardPicker)?;
        Ok(())
    }
}
