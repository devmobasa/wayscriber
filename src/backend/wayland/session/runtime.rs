use super::*;
use crate::session::{SaveAsOverwrite, SessionSnapshot, ToolStateSnapshot};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) struct RuntimeOpenSessionReport {
    pub previous_path: PathBuf,
    pub opened_path: PathBuf,
    pub saved_current: bool,
    pub loaded_board_data: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) struct RuntimeSaveAsSessionReport {
    pub previous_path: PathBuf,
    pub saved_path: PathBuf,
    pub switched_target: bool,
    pub saved: bool,
    pub saved_board_data: bool,
    pub outcome: Option<stored_session::SaveSnapshotOutcome>,
    pub written_size: Option<usize>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) struct RuntimeClearSessionReport {
    pub cleared_path: PathBuf,
    pub persisted: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) struct RuntimeClearToolStateReport {
    pub session_path: Option<PathBuf>,
    pub outcome: Option<stored_session::ClearToolStateOutcome>,
}

pub(in crate::backend::wayland) struct SessionTransaction<'a> {
    pub input_state: &'a mut InputState,
    pub measurer: &'a crate::draw::TextMeasurer,
    pub session: &'a mut SessionState,
}

mod transaction;
pub(in crate::backend::wayland) use transaction::{
    ExplicitSessionTransaction, SessionCommand, SessionCommandReport, TransactionStep,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SaveAsPreflightDecision {
    SameTarget,
    SwitchTarget,
}

fn accept_open_preflight(
    session: &mut crate::backend::wayland::session::SessionState,
    validation: PersistenceOutcome,
) -> Result<()> {
    if !matches!(validation, PersistenceOutcome::Unit) {
        return Err(anyhow!("unexpected named-session validation outcome"));
    }
    cancel_pending_output_transition_for_explicit_target(session, "Open");
    Ok(())
}

fn accept_save_as_preflight(
    session: &mut crate::backend::wayland::session::SessionState,
    preflight: PersistenceOutcome,
    overwrite: SaveAsOverwrite,
    target_path: &Path,
) -> Result<SaveAsPreflightDecision> {
    let PersistenceOutcome::SaveAsPreflight {
        same_target,
        overwrite_required,
    } = preflight
    else {
        return Err(anyhow!("unexpected Save As preflight outcome"));
    };
    if same_target {
        return Ok(SaveAsPreflightDecision::SameTarget);
    }
    if overwrite_required && matches!(overwrite, SaveAsOverwrite::Deny) {
        return Err(anyhow!(
            "Save Session As target already has session artifacts; overwrite confirmation required for {}",
            target_path.display()
        ));
    }
    cancel_pending_output_transition_for_explicit_target(session, "Save As");
    Ok(SaveAsPreflightDecision::SwitchTarget)
}

fn cancel_pending_output_transition_for_explicit_target(
    session: &mut crate::backend::wayland::session::SessionState,
    operation: &str,
) {
    if let Some(pending) = session.cancel_pending_output_transition() {
        log::info!(
            "{operation} superseded pending output transition from epoch {} to {:?}",
            pending.source_epoch,
            pending.physical_output_identity
        );
    }
}

fn named_candidate_snapshot(
    outcome: LoadSnapshotOutcome,
    options: &SessionOptions,
) -> Result<SessionSnapshot> {
    match outcome {
        LoadSnapshotOutcome::Loaded(snapshot)
        | LoadSnapshotOutcome::LoadedFromBackup(snapshot)
        | LoadSnapshotOutcome::LoadedFromRecovery(snapshot) => Ok(*snapshot),
        LoadSnapshotOutcome::Empty => Err(anyhow!(
            "named session file contains no usable session data: {}",
            options.session_file_path().display()
        )),
        LoadSnapshotOutcome::EmptyAfterCorruption { backup_path } => Err(anyhow!(
            "named session file could not be read: {}; a copy of it was saved to {}",
            options.session_file_path().display(),
            backup_path.display()
        )),
        LoadSnapshotOutcome::NonRegularArtifact { path } => Err(anyhow!(
            "named session file is not a regular file: {}",
            path.display()
        )),
        LoadSnapshotOutcome::ExpandedTooLarge {
            path,
            max_expanded_size,
        } => Err(anyhow!(
            "named session file expands beyond the {} byte safety limit: {}",
            max_expanded_size,
            path.display()
        )),
    }
}

fn session_persistence_enabled(options: &SessionOptions) -> bool {
    options.any_enabled() || options.restore_tool_state || options.persist_history
}

pub(in crate::backend::wayland) fn has_session_artifact(options: &SessionOptions) -> bool {
    options.session_file_path().exists()
        || options.backup_file_path().exists()
        || options.backup_recovery_marker_file_path().exists()
        || options.clear_marker_file_path().exists()
        || options.recovery_recoverable_marker_file_path().exists()
        || has_recovery_artifact(options)
}

fn has_recovery_artifact(options: &SessionOptions) -> bool {
    let recovery_path = options.recovery_file_path();
    if recovery_path.exists() {
        return true;
    }
    let Some(parent) = recovery_path.parent() else {
        return false;
    };
    let Some(recovery_name) = recovery_path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let preserved_prefix = format!("{recovery_name}.");
    let Ok(entries) = std::fs::read_dir(parent) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        let path = entry.path();
        path.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&preserved_prefix))
    })
}

pub(in crate::backend::wayland) fn should_skip_unloaded_contentless_save(
    loaded_board_data: bool,
    session_dirty: bool,
    input_dirty: bool,
    has_board_data: bool,
    session_artifact_exists: bool,
) -> bool {
    !has_board_data
        && !loaded_board_data
        && !session_dirty
        && !input_dirty
        && session_artifact_exists
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session_with_pending_transition() -> crate::backend::wayland::session::SessionState {
        let options = SessionOptions::new(PathBuf::from("/tmp"), "source-output");
        let mut staged = options.clone();
        staged.set_output_identity(Some("target-output"));
        let mut session = crate::backend::wayland::session::SessionState::new(Some(options));
        session.stage_output_transition(staged, Some("target-output".to_string()), Instant::now());
        session
    }

    #[test]
    fn open_cancels_pending_transition_only_after_accepted_preflight() {
        let mut rejected = session_with_pending_transition();
        assert!(
            accept_open_preflight(&mut rejected, PersistenceOutcome::HasArtifacts(false)).is_err()
        );
        assert!(rejected.pending_output_transition().is_some());

        let mut accepted = session_with_pending_transition();
        accept_open_preflight(&mut accepted, PersistenceOutcome::Unit).unwrap();
        assert!(accepted.pending_output_transition().is_none());
    }

    #[test]
    fn save_as_keeps_pending_transition_until_preflight_is_accepted() {
        let target = Path::new("/tmp/target.wayscriber-session");
        let mut denied = session_with_pending_transition();
        assert!(
            accept_save_as_preflight(
                &mut denied,
                PersistenceOutcome::SaveAsPreflight {
                    same_target: false,
                    overwrite_required: true,
                },
                SaveAsOverwrite::Deny,
                target,
            )
            .is_err()
        );
        assert!(denied.pending_output_transition().is_some());

        let mut accepted = session_with_pending_transition();
        assert_eq!(
            accept_save_as_preflight(
                &mut accepted,
                PersistenceOutcome::SaveAsPreflight {
                    same_target: false,
                    overwrite_required: false,
                },
                SaveAsOverwrite::Deny,
                target,
            )
            .unwrap(),
            SaveAsPreflightDecision::SwitchTarget
        );
        assert!(accepted.pending_output_transition().is_none());
    }

    #[test]
    fn save_as_same_target_keeps_unrelated_pending_output_transition() {
        let mut session = session_with_pending_transition();
        assert_eq!(
            accept_save_as_preflight(
                &mut session,
                PersistenceOutcome::SaveAsPreflight {
                    same_target: true,
                    overwrite_required: true,
                },
                SaveAsOverwrite::Deny,
                Path::new("/tmp/current.wayscriber-session"),
            )
            .unwrap(),
            SaveAsPreflightDecision::SameTarget
        );
        assert!(session.pending_output_transition().is_some());
    }
}
