//! Owns document transfer to effects and mutually exclusive load/save phases.
use std::path::PathBuf;
use wayscriber::config::{ConfigDocument, ConfigValidationReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocumentPhase {
    Idle,
    Loading,
    Saving,
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
    pub(crate) pending_validation: ConfigValidationReport,
    pub(crate) last_backup_path: Option<PathBuf>,
    pub(crate) after_save: Option<LeaveAction>,
}
impl DocumentWorkflow {
    pub(crate) fn loading() -> Self {
        Self {
            loaded: None,
            phase: DocumentPhase::Loading,
            pending_validation: Default::default(),
            last_backup_path: None,
            after_save: None,
        }
    }
    pub(crate) fn loaded(&self) -> Option<&ConfigDocument> {
        self.loaded.as_ref()
    }
    pub(crate) fn is_loading(&self) -> bool {
        self.phase == DocumentPhase::Loading
    }
    pub(crate) fn is_saving(&self) -> bool {
        self.phase == DocumentPhase::Saving
    }
    pub(crate) fn allows_editing(&self) -> bool {
        self.phase == DocumentPhase::Idle
    }
    pub(crate) fn begin_reload(&mut self) -> bool {
        if self.phase != DocumentPhase::Idle {
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
    pub(crate) fn begin_save(&mut self) -> Option<ConfigDocument> {
        if self.phase != DocumentPhase::Idle {
            return None;
        }
        let document = self.loaded.take()?;
        self.phase = DocumentPhase::Saving;
        Some(document)
    }
    pub(crate) fn finish_save(&mut self, document: Option<ConfigDocument>) {
        self.loaded = document;
        self.phase = DocumentPhase::Idle;
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
            DocumentPhase::Saving
        } else {
            DocumentPhase::Idle
        };
    }
}
