use crate::input::state::{Toast, ToastPriority};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Result, anyhow};

use super::super::*;
use crate::backend::wayland::{
    backend::event_loop::session_save,
    session::{
        PersistenceOperation, PersistenceOutcome, RuntimeClearSessionReport,
        RuntimeClearToolStateReport, RuntimeOpenSessionReport, RuntimeSaveAsSessionReport,
        SaveStrategy,
    },
};
use crate::session::{
    self as stored_session, ClearToolStateOutcome, LoadSnapshotOutcome, SaveAsOverwrite,
    SessionOptions, SessionSnapshot, ToolStateSnapshot,
};

impl WaylandState {
    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn open_named_session_runtime(
        &mut self,
        target_path: &Path,
    ) -> Result<RuntimeOpenSessionReport> {
        let current_options = self
            .session_options()
            .cloned()
            .ok_or_else(|| anyhow!("cannot open session without active session options"))?;
        let previous_path = current_options.session_file_path();

        let validation = session_save::run_persistence_operation(
            self,
            PersistenceOperation::ValidateNamedOpen {
                path: target_path.to_path_buf(),
            },
        )?;
        accept_open_preflight(&mut self.session, validation)?;

        let saved_current = self.save_current_before_explicit_target_change(&current_options)?;
        let mut candidate_options = current_options;
        candidate_options.set_named_file_target(target_path.to_path_buf());
        candidate_options.force_resume_persistence();

        let outcome = session_save::run_persistence_operation(
            self,
            PersistenceOperation::LoadNamedCandidate {
                options: candidate_options.clone(),
            },
        )?;
        let PersistenceOutcome::Load(load_outcome) = outcome else {
            return Err(anyhow!("unexpected named-session load outcome"));
        };
        let candidate_snapshot = named_candidate_snapshot(load_outcome, &candidate_options)?;
        let loaded_board_data = candidate_snapshot.has_board_data();
        stored_session::apply_snapshot_replacing_boards(
            &mut self.input_state,
            candidate_snapshot,
            &candidate_options,
        )?;
        self.refresh_runtime_ui_config_seeds();
        self.input_state
            .set_session_preflight_options(Some(candidate_options.clone()));
        self.input_state.clear_session_dirty();
        let opened_path = candidate_options.session_file_path();
        self.session
            .commit_runtime_open(candidate_options.clone(), loaded_board_data);

        match session_save::run_persistence_operation(
            self,
            PersistenceOperation::RecordNamedOpened {
                options: candidate_options,
            },
        ) {
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

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn save_named_session_as_runtime(
        &mut self,
        target_path: &Path,
        overwrite: SaveAsOverwrite,
    ) -> Result<RuntimeSaveAsSessionReport> {
        let current_options = self
            .session_options()
            .cloned()
            .ok_or_else(|| anyhow!("cannot save session as without active session options"))?;
        let previous_path = current_options.session_file_path();
        let mut target_options = current_options.clone();
        target_options.set_named_file_target(target_path.to_path_buf());
        target_options.force_resume_persistence();
        let preflight = session_save::run_persistence_operation(
            self,
            PersistenceOperation::SaveAsOverwritePreflight {
                current_path: previous_path.clone(),
                options: target_options.clone(),
            },
        )?;
        match accept_save_as_preflight(&mut self.session, preflight, overwrite, target_path)? {
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
            .with_active_interaction_canceled_for_capture(|input_state| {
                stored_session::snapshot_from_input(input_state, &target_options)
            })
            .ok_or_else(|| anyhow!("Save Session As has no session data to write"))?;
        let outcome = session_save::run_persistence_operation(
            self,
            PersistenceOperation::SaveAs {
                snapshot,
                options: target_options.clone(),
                overwrite,
            },
        )?;
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

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn save_named_session_as_requires_overwrite(
        &mut self,
        target_path: &Path,
    ) -> Result<bool> {
        let current_options = self
            .session_options()
            .cloned()
            .ok_or_else(|| anyhow!("cannot save session as without active session options"))?;
        let previous_path = current_options.session_file_path();
        let mut target_options = current_options;
        target_options.set_named_file_target(target_path.to_path_buf());
        target_options.force_resume_persistence();
        let outcome = session_save::run_persistence_operation(
            self,
            PersistenceOperation::SaveAsOverwritePreflight {
                current_path: previous_path,
                options: target_options,
            },
        )?;
        let PersistenceOutcome::SaveAsPreflight {
            same_target,
            overwrite_required,
        } = outcome
        else {
            return Err(anyhow!("unexpected Save As preflight outcome"));
        };
        Ok(!same_target && overwrite_required)
    }

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn clear_current_session_runtime(
        &mut self,
    ) -> Result<RuntimeClearSessionReport> {
        session_save::persistence_barrier(self)?;
        let options = self
            .session_options()
            .cloned()
            .ok_or_else(|| anyhow!("cannot clear session without active session options"))?;
        let cleared_path = options.session_file_path();
        let empty_snapshot = SessionSnapshot {
            active_board_id: self.input_state.board_id().to_string(),
            boards: Vec::new(),
            tool_state: None,
        };
        let outcome = session_save::run_persistence_operation(
            self,
            PersistenceOperation::Save {
                snapshot: empty_snapshot.clone(),
                options: options.clone(),
                strategy: SaveStrategy::Normal,
                contentless_clear_boundary: true,
            },
        )?;
        let PersistenceOutcome::Save(save) = outcome else {
            return Err(anyhow!("unexpected clear-session worker outcome"));
        };
        if !save.committed() {
            return Err(anyhow!(
                "current session clear did not write a committed clear boundary"
            ));
        }
        stored_session::apply_snapshot_replacing_boards(
            &mut self.input_state,
            empty_snapshot,
            &options,
        )?;
        self.refresh_runtime_ui_config_seeds();
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

    #[allow(dead_code)]
    pub(in crate::backend::wayland) fn clear_saved_tool_state_runtime(
        &mut self,
    ) -> Result<RuntimeClearToolStateReport> {
        let default_tool_state = ToolStateSnapshot::from_config(&self.config);
        let (session_path, outcome) = if let Some(options) = self.session_options().cloned() {
            let path = options.session_file_path();
            let outcome = session_save::run_persistence_operation(
                self,
                PersistenceOperation::ClearToolState { options },
            )?;
            let PersistenceOutcome::ToolStateCleared(outcome) = outcome else {
                return Err(anyhow!("unexpected clear-tool-state worker outcome"));
            };
            (Some(path), Some(outcome))
        } else {
            (None, None)
        };
        stored_session::apply_tool_state_snapshot(&mut self.input_state, default_tool_state);
        self.input_state.mark_session_dirty();
        Ok(RuntimeClearToolStateReport {
            session_path,
            outcome,
        })
    }

    pub(in crate::backend::wayland) fn handle_clear_saved_tool_state_action(&mut self) {
        match self.clear_saved_tool_state_runtime() {
            Ok(report) => {
                let message = clear_tool_state_runtime_message(&report);
                log::info!("{message}");
                self.input_state
                    .push_toast(ToastPriority::Info, "session", Toast::info(message));
            }
            Err(err) => {
                let message = format!("Failed to reset tool defaults: {err:#}");
                log::warn!("{message}");
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    "session",
                    Toast::error(message),
                );
            }
        }
    }

    pub(in crate::backend::wayland) fn inspect_active_session(
        &mut self,
    ) -> Result<stored_session::SessionInspection> {
        let options = self
            .session_options()
            .cloned()
            .ok_or_else(|| anyhow!("no active persisted session target"))?;
        let outcome = session_save::run_persistence_operation(
            self,
            PersistenceOperation::Inspect { options },
        )?;
        let PersistenceOutcome::Inspection(inspection) = outcome else {
            return Err(anyhow!("unexpected session-inspection worker outcome"));
        };
        Ok(inspection)
    }

    pub(in crate::backend::wayland) fn forget_named_session_by_path(
        &mut self,
        path: PathBuf,
    ) -> Result<bool> {
        let outcome = session_save::run_persistence_operation(
            self,
            PersistenceOperation::ForgetNamedSessionByPath { path },
        )?;
        let PersistenceOutcome::CatalogForgotten(forgotten) = outcome else {
            return Err(anyhow!("unexpected catalog-forget worker outcome"));
        };
        Ok(forgotten)
    }

    fn save_current_before_explicit_target_change(
        &mut self,
        options: &SessionOptions,
    ) -> Result<bool> {
        session_save::persistence_barrier(self)?;
        if !self.input_state.is_session_dirty() && !self.session.is_dirty() {
            return Ok(false);
        }
        let snapshot = self
            .input_state
            .with_active_interaction_canceled_for_capture(|input_state| {
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
        let outcome = session_save::run_persistence_operation(
            self,
            PersistenceOperation::Save {
                snapshot,
                options: options.clone(),
                strategy: SaveStrategy::Normal,
                contentless_clear_boundary: self.session.has_loaded_board_data(),
            },
        )?;
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

fn clear_tool_state_runtime_message(report: &RuntimeClearToolStateReport) -> String {
    match report.outcome {
        Some(ClearToolStateOutcome::Cleared {
            preserved_board_data: true,
        }) => {
            "Tool defaults reset from config. Saved boards and history were preserved.".to_string()
        }
        Some(ClearToolStateOutcome::Cleared {
            preserved_board_data: false,
        }) => "Tool defaults reset from config. No board data was present.".to_string(),
        Some(ClearToolStateOutcome::NoToolState) => {
            "Tool defaults reset from config. No saved tool state was stored.".to_string()
        }
        Some(ClearToolStateOutcome::NoSession) => {
            "Tool defaults reset from config. No saved session file was present.".to_string()
        }
        None => "Tool defaults reset from config for this run. No active session file to edit."
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

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
