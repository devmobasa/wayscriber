//! Shortcut edit modes are mutually exclusive and clear as one workflow.
use crate::models::{
    KeybindingField, PendingShortcutConflict, ShortcutRecorderState, ShortcutTextEditor,
};

#[derive(Debug, Default)]
enum ShortcutPhase {
    #[default]
    Idle,
    Recording(ShortcutRecorderState),
    Text(ShortcutTextEditor),
    Conflict(PendingShortcutConflict),
}

#[derive(Debug, Default)]
pub(crate) struct ShortcutWorkflow {
    phase: ShortcutPhase,
    pub(crate) review: bool,
}

impl ShortcutWorkflow {
    pub(crate) fn begin_recording(&mut self, recorder: ShortcutRecorderState) {
        if self.conflict().is_none() {
            self.phase = ShortcutPhase::Recording(recorder);
        }
    }
    pub(crate) fn begin_text_edit(&mut self, editor: ShortcutTextEditor) {
        if self.conflict().is_none() {
            self.phase = ShortcutPhase::Text(editor);
        }
    }
    pub(crate) fn cancel_recording(&mut self, field: KeybindingField) {
        if self
            .recorder()
            .is_some_and(|recorder| recorder.field == field)
        {
            self.phase = ShortcutPhase::Idle;
        }
    }
    pub(crate) fn cancel_text_edit(&mut self, field: KeybindingField) {
        if self.editor().is_some_and(|editor| editor.field == field) {
            self.phase = ShortcutPhase::Idle;
        }
    }
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
    pub(crate) fn recorder(&self) -> Option<&ShortcutRecorderState> {
        match &self.phase {
            ShortcutPhase::Recording(value) => Some(value),
            _ => None,
        }
    }
    pub(crate) fn recorder_mut(&mut self) -> Option<&mut ShortcutRecorderState> {
        match &mut self.phase {
            ShortcutPhase::Recording(value) => Some(value),
            _ => None,
        }
    }
    pub(crate) fn take_recorder(&mut self) -> Option<ShortcutRecorderState> {
        self.recorder()?;
        match std::mem::take(&mut self.phase) {
            ShortcutPhase::Recording(value) => Some(value),
            _ => None,
        }
    }
    pub(crate) fn set_recorder(&mut self, value: Option<ShortcutRecorderState>) {
        if let Some(value) = value {
            self.phase = ShortcutPhase::Recording(value);
        } else if self.recorder().is_some() {
            self.phase = ShortcutPhase::Idle;
        }
    }
    pub(crate) fn editor(&self) -> Option<&ShortcutTextEditor> {
        match &self.phase {
            ShortcutPhase::Text(value) => Some(value),
            _ => None,
        }
    }
    pub(crate) fn editor_mut(&mut self) -> Option<&mut ShortcutTextEditor> {
        match &mut self.phase {
            ShortcutPhase::Text(value) => Some(value),
            _ => None,
        }
    }
    pub(crate) fn set_editor(&mut self, value: Option<ShortcutTextEditor>) {
        if let Some(value) = value {
            self.phase = ShortcutPhase::Text(value);
        } else if self.editor().is_some() {
            self.phase = ShortcutPhase::Idle;
        }
    }
    pub(crate) fn conflict(&self) -> Option<&PendingShortcutConflict> {
        match &self.phase {
            ShortcutPhase::Conflict(value) => Some(value),
            _ => None,
        }
    }
    pub(crate) fn take_conflict(&mut self) -> Option<PendingShortcutConflict> {
        self.conflict()?;
        match std::mem::take(&mut self.phase) {
            ShortcutPhase::Conflict(value) => Some(value),
            _ => None,
        }
    }
    pub(crate) fn set_conflict(&mut self, value: Option<PendingShortcutConflict>) {
        if let Some(value) = value {
            self.phase = ShortcutPhase::Conflict(value);
        } else if self.conflict().is_some() {
            self.phase = ShortcutPhase::Idle;
        }
    }
}
