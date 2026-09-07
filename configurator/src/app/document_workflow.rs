//! Owns document transfer to effects and mutually exclusive load/save phases.
use std::path::PathBuf;
use wayscriber::config::{ConfigDocument, ConfigValidationReport};

#[derive(Debug)]
enum DocumentPhase {
    Idle,
    Loading,
    Saving(SaveContext),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LeaveAction {
    Reload,
    Close,
}

#[derive(Debug)]
pub(crate) struct DocumentWorkflow {
    loaded: Option<ConfigDocument>,
    phase: DocumentPhase,
    last_backup_path: Option<PathBuf>,
}
impl DocumentWorkflow {
    pub(crate) fn loading() -> Self {
        Self {
            loaded: None,
            phase: DocumentPhase::Loading,
            last_backup_path: None,
        }
    }
    pub(crate) fn loaded(&self) -> Option<&ConfigDocument> {
        self.loaded.as_ref()
    }
    pub(crate) fn is_loading(&self) -> bool {
        matches!(self.phase, DocumentPhase::Loading)
    }
    pub(crate) fn is_saving(&self) -> bool {
        matches!(self.phase, DocumentPhase::Saving(_))
    }
    pub(crate) fn allows_editing(&self) -> bool {
        matches!(self.phase, DocumentPhase::Idle)
    }
    pub(crate) fn begin_reload(&mut self) -> bool {
        if !matches!(self.phase, DocumentPhase::Idle) {
            return false;
        }
        self.phase = DocumentPhase::Loading;
        true
    }
    pub(crate) fn finish_load(&mut self, document: Option<ConfigDocument>) {
        self.phase = DocumentPhase::Idle;
        if document.is_some() {
            self.loaded = document;
        }
    }
    pub(crate) fn begin_save(
        &mut self,
        validation: ConfigValidationReport,
        after_save: Option<LeaveAction>,
    ) -> Option<ConfigDocument> {
        if !self.allows_editing() {
            return None;
        }
        let document = self.loaded.take()?;
        self.phase = DocumentPhase::Saving(SaveContext {
            validation,
            after_save,
        });
        Some(document)
    }

    pub(crate) fn save_succeeded(
        &mut self,
        document: ConfigDocument,
        backup: Option<PathBuf>,
    ) -> Option<SaveCompletion> {
        if !self.is_saving() {
            return None;
        }
        let DocumentPhase::Saving(context) =
            std::mem::replace(&mut self.phase, DocumentPhase::Idle)
        else {
            return None;
        };
        self.loaded = Some(document);
        self.last_backup_path = backup;
        Some(SaveCompletion {
            validation: context.validation,
            after_save: context.after_save,
        })
    }

    pub(crate) fn save_failed(&mut self, document: Option<ConfigDocument>) {
        if !self.is_saving() {
            return;
        }
        // Dropping the active context discards both validation and continuation.
        self.phase = DocumentPhase::Idle;
        self.loaded = document;
    }

    #[cfg(test)]
    pub(crate) fn last_backup_path(&self) -> Option<&PathBuf> {
        self.last_backup_path.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn set_loading_for_test(&mut self, loading: bool) {
        self.phase = if loading {
            DocumentPhase::Loading
        } else {
            DocumentPhase::Idle
        };
    }
    #[cfg(test)]
    pub(crate) fn set_saving_for_test(&mut self, saving: bool) {
        self.phase = if saving {
            DocumentPhase::Saving(SaveContext {
                validation: Default::default(),
                after_save: None,
            })
        } else {
            DocumentPhase::Idle
        };
    }
}

#[derive(Debug)]
struct SaveContext {
    validation: ConfigValidationReport,
    after_save: Option<LeaveAction>,
}

pub(crate) struct SaveCompletion {
    pub(crate) validation: ConfigValidationReport,
    pub(crate) after_save: Option<LeaveAction>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_failure_cannot_finish_a_reload() {
        let mut workflow = DocumentWorkflow::loading();
        workflow.save_failed(None);
        assert!(workflow.is_loading());
        assert!(!workflow.allows_editing());
    }
}
