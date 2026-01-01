use super::super::KeybindingsConfig;
use super::BindingInserter;
use crate::config::Action;

impl KeybindingsConfig {
    pub(super) fn insert_board_bindings(
        &self,
        inserter: &mut BindingInserter,
    ) -> Result<(), String> {
        inserter.insert_all(&self.toggle_whiteboard, Action::ToggleWhiteboard)?;
        inserter.insert_all(&self.toggle_blackboard, Action::ToggleBlackboard)?;
        inserter.insert_all(&self.return_to_transparent, Action::ReturnToTransparent)?;
        inserter.insert_all(&self.page_prev, Action::PagePrev)?;
        inserter.insert_all(&self.page_next, Action::PageNext)?;
        inserter.insert_all(&self.page_new, Action::PageNew)?;
        inserter.insert_all(&self.page_duplicate, Action::PageDuplicate)?;
        inserter.insert_all(&self.page_delete, Action::PageDelete)?;
        Ok(())
    }
}
