//! Migration offers are tied to a document destination, independent of status text.
use crate::models::{ConfigDraft, KeybindingField};
use std::path::PathBuf;
use wayscriber::config::{ConfigDocument, MigrationPreview};

#[derive(Debug, Default)]
pub(crate) struct MigrationWorkflow {
    preview: Option<MigrationPreview>,
    dismissed: Option<PathBuf>,
}

pub(crate) struct MigrationApplied {
    pub(crate) changed: usize,
    pub(crate) kept: Vec<&'static str>,
}

impl MigrationWorkflow {
    pub(crate) fn refresh(&mut self, document: &ConfigDocument) {
        if self.dismissed.as_deref() != Some(document.destination()) {
            self.dismissed = None;
        }
        self.preview = MigrationPreview::for_authored_config(document.authored_config());
    }
    pub(crate) fn pending(&self) -> Option<&MigrationPreview> {
        if self.dismissed.is_some() {
            None
        } else {
            self.preview.as_ref()
        }
    }
    pub(crate) fn dismiss(&mut self, document: Option<&ConfigDocument>) {
        if let Some(document) = document {
            self.dismissed = Some(document.destination().to_path_buf());
        }
    }
    pub(crate) fn apply(&mut self, draft: &mut ConfigDraft) -> Option<MigrationApplied> {
        self.pending()?;
        let preview = self.preview.take()?;
        let mut result = MigrationApplied {
            changed: 0,
            kept: Vec::new(),
        };
        for change in preview.changes() {
            let Some(field) = KeybindingField::from_field_key(change.config_key()) else {
                continue;
            };
            if draft.keybindings.parses_to(field, change.before()) {
                draft.keybindings.set(field, change.after().join(", "));
                result.changed += 1;
            } else if !draft.keybindings.parses_to(field, change.after()) {
                result.kept.push(change.action_label());
            }
        }
        draft.config_revision = Some(preview.proposed_revision());
        Some(result)
    }
}
