use super::super::KeybindingsConfig;
use super::BindingInserter;
use crate::config::Action;

impl KeybindingsConfig {
    pub(super) fn insert_core_bindings(
        &self,
        inserter: &mut BindingInserter,
    ) -> Result<(), String> {
        inserter.insert_all(&self.core.exit, Action::Exit)?;
        inserter.insert_all(&self.core.enter_text_mode, Action::EnterTextMode)?;
        inserter.insert_all(
            &self.core.enter_sticky_note_mode,
            Action::EnterStickyNoteMode,
        )?;
        inserter.insert_all(&self.core.clear_canvas, Action::ClearCanvas)?;
        inserter.insert_all(&self.core.undo, Action::Undo)?;
        inserter.insert_all(&self.core.redo, Action::Redo)?;
        inserter.insert_all(&self.core.undo_all, Action::UndoAll)?;
        inserter.insert_all(&self.core.redo_all, Action::RedoAll)?;
        inserter.insert_all(&self.core.undo_all_delayed, Action::UndoAllDelayed)?;
        inserter.insert_all(&self.core.redo_all_delayed, Action::RedoAllDelayed)?;
        Ok(())
    }
}
