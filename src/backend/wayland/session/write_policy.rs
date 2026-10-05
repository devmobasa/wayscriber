//! Which drawings may be written to the source session file.

use anyhow::Result;
use std::path::Path;

use super::{PersistenceOutcome, SessionState, stored_session};

impl SessionState {
    pub(in crate::backend::wayland) fn observe_load_failure(
        &mut self,
        result: &Result<PersistenceOutcome>,
    ) {
        match result {
            Err(error) | Ok(PersistenceOutcome::RememberedUnavailable(error)) => {
                self.block_unpreserved_corruption(error)
            }
            _ => {}
        }
    }

    pub(in crate::backend::wayland) fn block_unpreserved_corruption(
        &mut self,
        error: &anyhow::Error,
    ) {
        if let Some(failure) =
            error.downcast_ref::<stored_session::CorruptArtifactPreservationFailed>()
        {
            self.blocked_session_paths
                .insert(failure.session_path.clone());
        }
    }

    /// A clean unresolved source needs no write. Dirty unresolved or blocked
    /// ink stays on screen until an explicit recovery action resolves its target.
    pub(in crate::backend::wayland) fn validate_source_write(
        &self,
        input_dirty: bool,
    ) -> Result<bool> {
        let dirty = self.is_dirty() || input_dirty;
        let blocked = self
            .options
            .as_ref()
            .is_some_and(|options| self.refuses_source_write(&options.session_file_path()));
        if (!self.loaded || blocked) && dirty {
            let refusal = if !self.loaded {
                SourceWriteRefused::NotLoaded
            } else {
                SourceWriteRefused::NotPreserved
            };
            return Err(refusal.into());
        }
        Ok(self.loaded && !blocked)
    }

    pub(in crate::backend::wayland) fn refuses_source_write(&self, path: &Path) -> bool {
        self.blocked_session_paths.contains(path)
    }

    pub(in crate::backend::wayland) fn confirm_successful_load(&mut self, path: &Path) {
        self.blocked_session_paths.remove(path);
        self.protected_session_paths.remove(path);
    }
}

/// Why drawings may not be written to the source session file.
#[derive(Debug, Clone, Copy)]
pub(in crate::backend::wayland) enum SourceWriteRefused {
    /// The session never loaded, so its file still holds what it held.
    NotLoaded,
    /// Its unreadable bytes could not be copied aside before a replacement.
    NotPreserved,
}

impl std::fmt::Display for SourceWriteRefused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::NotLoaded => {
                "the source session has not loaded; new drawings are kept on screen and saved drawings are untouched; use Save As to keep new drawings elsewhere, or Clear before reloading"
            }
            Self::NotPreserved => {
                "unreadable session data was not preserved; repair and reload its files or use Save As before replacing these drawings"
            }
        })
    }
}

impl std::error::Error for SourceWriteRefused {}
