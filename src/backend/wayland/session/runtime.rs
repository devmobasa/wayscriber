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
    pub persistence: &'a mut PersistenceController,
}

impl SessionTransaction<'_> {
    fn run(&mut self, operation: PersistenceOperation) -> Result<PersistenceOutcome> {
        self.persistence.run(self.session.target_epoch(), operation)
    }

    pub(in crate::backend::wayland) fn open_named_session_runtime(
        &mut self,
        target_path: &Path,
    ) -> Result<RuntimeOpenSessionReport> {
        let current_options = self
            .session
            .options()
            .cloned()
            .ok_or_else(|| anyhow!("cannot open session without active session options"))?;
        let previous_path = current_options.session_file_path();

        let validation = self.run(PersistenceOperation::ValidateNamedOpen {
            path: target_path.to_path_buf(),
        })?;
        accept_open_preflight(self.session, validation)?;

        let saved_current = self.save_current_before_explicit_target_change(&current_options)?;
        let mut candidate_options = current_options;
        candidate_options.set_named_file_target(target_path.to_path_buf());
        candidate_options.force_resume_persistence();

        let outcome = self.run(PersistenceOperation::LoadNamedCandidate {
            options: candidate_options.clone(),
        })?;
        let PersistenceOutcome::Load(load_outcome) = outcome else {
            return Err(anyhow!("unexpected named-session load outcome"));
        };
        let candidate_snapshot = named_candidate_snapshot(load_outcome, &candidate_options)?;
        let loaded_board_data = candidate_snapshot.has_board_data();
        stored_session::apply_snapshot_replacing_boards(
            self.input_state,
            self.measurer,
            candidate_snapshot,
            &candidate_options,
        )?;
        self.input_state
            .set_session_preflight_options(Some(candidate_options.clone()));
        self.input_state.clear_session_dirty();
        let opened_path = candidate_options.session_file_path();
        self.session
            .commit_runtime_open(candidate_options.clone(), loaded_board_data);

        match self.run(PersistenceOperation::RecordNamedOpened {
            options: candidate_options,
        }) {
            Ok(PersistenceOutcome::Unit) => {}
            Ok(other) => log::warn!(
                "Named session opened, but catalog worker returned an unexpected outcome: {other:?}"
            ),
            Err(err) => log::warn!(
                "Named session opened, but recording it in the recent-session catalog failed: {err:#}"
            ),
        }

        Ok(RuntimeOpenSessionReport {
            previous_path,
            opened_path,
            saved_current,
            loaded_board_data,
        })
    }

    pub(in crate::backend::wayland) fn save_named_session_as_runtime(
        &mut self,
        target_path: &Path,
        overwrite: SaveAsOverwrite,
    ) -> Result<RuntimeSaveAsSessionReport> {
        let current_options = self
            .session
            .options()
            .cloned()
            .ok_or_else(|| anyhow!("cannot save session as without active session options"))?;
        let previous_path = current_options.session_file_path();
        let mut target_options = current_options.clone();
        target_options.set_named_file_target(target_path.to_path_buf());
        target_options.force_resume_persistence();
        let preflight = self.run(PersistenceOperation::SaveAsOverwritePreflight {
            current_path: previous_path.clone(),
            options: target_options.clone(),
        })?;
        match accept_save_as_preflight(self.session, preflight, overwrite, target_path)? {
            SaveAsPreflightDecision::SameTarget => {
                let saved = self.save_current_before_explicit_target_change(&current_options)?;
                return Ok(RuntimeSaveAsSessionReport {
                    previous_path: previous_path.clone(),
                    saved_path: previous_path,
                    switched_target: false,
                    saved,
                    saved_board_data: self.session.has_loaded_board_data(),
                    outcome: None,
                    written_size: None,
                });
            }
            SaveAsPreflightDecision::SwitchTarget => {}
        }

        let snapshot = self
            .input_state
            .with_active_interaction_canceled_for_capture_with(self.measurer, |input_state| {
                stored_session::snapshot_from_input(input_state, &target_options)
            })
            .ok_or_else(|| anyhow!("Save Session As has no session data to write"))?;
        let outcome = self.run(PersistenceOperation::SaveAs {
            snapshot,
            options: target_options.clone(),
            overwrite,
        })?;
        let PersistenceOutcome::SaveAs {
            report,
            committed_board_data,
        } = outcome
        else {
            return Err(anyhow!("unexpected Save As worker outcome"));
        };

        self.input_state
            .set_session_preflight_options(Some(target_options.clone()));
        let _ = self.input_state.take_session_dirty();
        self.input_state.clear_session_dirty();
        let saved_path = target_options.session_file_path();
        self.session
            .commit_runtime_save_as(target_options, Instant::now(), committed_board_data);

        Ok(RuntimeSaveAsSessionReport {
            previous_path,
            saved_path,
            switched_target: true,
            saved: true,
            saved_board_data: committed_board_data,
            outcome: Some(report.outcome),
            written_size: Some(report.written_size),
        })
    }

    pub(in crate::backend::wayland) fn save_named_session_as_requires_overwrite(
        &mut self,
        target_path: &Path,
    ) -> Result<bool> {
        let current_options = self
            .session
            .options()
            .cloned()
            .ok_or_else(|| anyhow!("cannot save session as without active session options"))?;
        let previous_path = current_options.session_file_path();
        let mut target_options = current_options;
        target_options.set_named_file_target(target_path.to_path_buf());
        target_options.force_resume_persistence();
        let outcome = self.run(PersistenceOperation::SaveAsOverwritePreflight {
            current_path: previous_path,
            options: target_options,
        })?;
        let PersistenceOutcome::SaveAsPreflight {
            same_target,
            overwrite_required,
        } = outcome
        else {
            return Err(anyhow!("unexpected Save As preflight outcome"));
        };
        Ok(!same_target && overwrite_required)
    }

    pub(in crate::backend::wayland) fn clear_current_session_runtime(
        &mut self,
    ) -> Result<RuntimeClearSessionReport> {
        let options = self
            .session
            .options()
            .cloned()
            .ok_or_else(|| anyhow!("cannot clear session without active session options"))?;
        let cleared_path = options.session_file_path();
        let empty_snapshot = SessionSnapshot {
            active_board_id: self.input_state.board_id().to_string(),
            boards: Vec::new(),
            tool_state: None,
        };
        let outcome = self.run(PersistenceOperation::Save {
            snapshot: empty_snapshot.clone(),
            options: options.clone(),
            strategy: SaveStrategy::Normal,
            contentless_clear_boundary: true,
        })?;
        let PersistenceOutcome::Save(save) = outcome else {
            return Err(anyhow!("unexpected clear-session worker outcome"));
        };
        if !save.committed() {
            return Err(anyhow!(
                "current session clear did not write a committed clear boundary"
            ));
        }
        stored_session::apply_snapshot_replacing_boards(
            self.input_state,
            self.measurer,
            empty_snapshot,
            &options,
        )?;
        self.input_state
            .set_session_preflight_options(Some(options));
        let _ = self.input_state.take_session_dirty();
        self.input_state.clear_session_dirty();
        self.session.commit_runtime_clear(Instant::now());
        Ok(RuntimeClearSessionReport {
            cleared_path,
            persisted: true,
        })
    }

    pub(in crate::backend::wayland) fn clear_saved_tool_state_runtime(
        &mut self,
        default_tool_state: ToolStateSnapshot,
    ) -> Result<RuntimeClearToolStateReport> {
        let (session_path, outcome) = if let Some(options) = self.session.options().cloned() {
            let path = options.session_file_path();
            let outcome = self.run(PersistenceOperation::ClearToolState { options })?;
            let PersistenceOutcome::ToolStateCleared(outcome) = outcome else {
                return Err(anyhow!("unexpected clear-tool-state worker outcome"));
            };
            (Some(path), Some(outcome))
        } else {
            (None, None)
        };
        stored_session::apply_tool_state_snapshot(
            self.input_state,
            self.measurer,
            default_tool_state,
        );
        self.input_state.mark_session_dirty();
        self.session.record_input_dirty(Instant::now(), true);

        Ok(RuntimeClearToolStateReport {
            session_path,
            outcome,
        })
    }

    fn save_current_before_explicit_target_change(
        &mut self,
        options: &SessionOptions,
    ) -> Result<bool> {
        if !self.input_state.is_session_dirty() && !self.session.is_dirty() {
            return Ok(false);
        }
        let snapshot = self
            .input_state
            .with_active_interaction_canceled_for_capture_with(self.measurer, |input_state| {
                stored_session::snapshot_from_input(input_state, options)
            });
        let snapshot = if let Some(snapshot) = snapshot {
            snapshot
        } else if session_persistence_enabled(options) {
            SessionSnapshot {
                active_board_id: self.input_state.board_id().to_string(),
                boards: Vec::new(),
                tool_state: None,
            }
        } else {
            return Err(anyhow!(
                "current session has unsaved changes but persistence is disabled"
            ));
        };
        let outcome = self.run(PersistenceOperation::Save {
            snapshot,
            options: options.clone(),
            strategy: SaveStrategy::Normal,
            contentless_clear_boundary: self.session.has_loaded_board_data(),
        })?;
        let PersistenceOutcome::Save(save) = outcome else {
            return Err(anyhow!("unexpected save-before-target-change outcome"));
        };
        if !save.committed() {
            return Err(anyhow!(
                "current session had unsaved changes but no session file was written"
            ));
        }
        let _ = self.input_state.take_session_dirty();
        self.session
            .mark_saved(Instant::now(), save.committed_board_data);
        Ok(true)
    }
}

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
