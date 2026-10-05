//! What one artifact's load result means: a snapshot, an empty session, a
//! preserved unreadable copy, or a refusal to continue.
use super::*;

#[derive(Debug)]
pub(crate) struct CorruptArtifactPreservationFailed {
    pub(crate) path: PathBuf,
    pub(crate) session_path: PathBuf,
    cause: anyhow::Error,
}

impl fmt::Display for CorruptArtifactPreservationFailed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "could not preserve unreadable session {}; automatic writes are refused to keep its backup intact: {:#}",
            self.path.display(),
            self.cause
        )
    }
}

impl std::error::Error for CorruptArtifactPreservationFailed {}

#[derive(Clone, Copy)]
pub(super) enum CorruptLoadAction {
    Quarantine,
    Preserve,
}

pub(super) fn artifact_load_outcome(
    result: Result<Option<LoadedSnapshot>>,
    session_path: &Path,
    options: &SessionOptions,
    max_expanded_size: u64,
    label: &str,
    corrupt_load_action: CorruptLoadAction,
) -> Result<LoadSnapshotOutcome> {
    match result {
        Ok(Some(loaded)) => {
            let tool_state = loaded.snapshot.tool_state.is_some();
            info!(
                "Loaded {} from {} (version {}, compressed={}, boards={}, active_board={}, tool_state={})",
                label,
                session_path.display(),
                loaded.version,
                loaded.compressed,
                loaded.snapshot.boards.len(),
                loaded.snapshot.active_board_id,
                tool_state
            );
            Ok(LoadSnapshotOutcome::Loaded(Box::new(loaded.snapshot)))
        }
        Ok(None) => {
            info!(
                "{} file {} contained no usable data; continuing with defaults",
                label,
                session_path.display()
            );
            Ok(LoadSnapshotOutcome::Empty)
        }
        Err(err) if err.downcast_ref::<ExpandedSessionTooLarge>().is_some() => {
            warn!(
                "Refusing to load session {}; expanded payload exceeds safety limit ({} bytes). The session file is left unchanged; clear the session or move the file if it is no longer needed: {}",
                session_path.display(),
                max_expanded_size,
                err
            );
            Ok(LoadSnapshotOutcome::ExpandedTooLarge {
                path: session_path.to_path_buf(),
                max_expanded_size,
            })
        }
        Err(err) if is_non_regular_session_artifact(&err) => {
            warn!(
                "Refusing to load non-regular {} {}; continuing with defaults: {}",
                label,
                session_path.display(),
                err
            );
            Ok(LoadSnapshotOutcome::NonRegularArtifact {
                path: session_path.to_path_buf(),
            })
        }
        Err(err)
            if err
                .downcast_ref::<NewerVersionPreservationFailed>()
                .is_some() =>
        {
            // Fail closed. Treating this as corruption would back it up and
            // then continue with an empty, saveable session — exactly the
            // destructive downgrade path this error reports.
            Err(err)
        }
        Err(err) => {
            warn!(
                "Failed to load {} {}; continuing with defaults: {}",
                label,
                session_path.display(),
                err
            );
            match corrupt_load_action {
                CorruptLoadAction::Quarantine => {
                    match quarantine_corrupt_artifact(session_path, options, max_expanded_size) {
                        Ok(corrupt_copy) => {
                            return Ok(LoadSnapshotOutcome::EmptyAfterCorruption { corrupt_copy });
                        }
                        Err(cause) => {
                            return Err(CorruptArtifactPreservationFailed {
                                path: session_path.to_path_buf(),
                                session_path: options.session_file_path(),
                                cause,
                            }
                            .into());
                        }
                    }
                }
                CorruptLoadAction::Preserve => {
                    debug!(
                        "Leaving unloadable {} {} in place because it is suppressed by the session clear marker",
                        label,
                        session_path.display()
                    );
                }
            }
            Ok(LoadSnapshotOutcome::Empty)
        }
    }
}
