use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn load_configured_session_for_options(
        &mut self,
        options: session::SessionOptions,
        context: &str,
    ) -> anyhow::Result<()> {
        let outcome = session_save::run_persistence_operation(
            self,
            PersistenceOperation::LoadConfigured {
                options: options.clone(),
            },
        )?;
        let PersistenceOutcome::Load(load_outcome) = outcome else {
            return Err(anyhow::anyhow!("unexpected configured-load worker outcome"));
        };
        let loaded_board_data = load_outcome.has_board_data();
        self.handle_session_load_outcome_for_options(load_outcome, &options, context)?;
        self.session
            .commit_output_options(options, loaded_board_data);
        Ok(())
    }

    /// After a launch-time session load, announce ink restored onto the
    /// transparent overlay board, once per launch. With per-output sessions
    /// the ink arrives with the first output transition rather than the
    /// initial load, so both report here; later output or named-session
    /// switches stay quiet. Overlays the daemon reopens on a toggle show what
    /// the user just had on screen, so they stay quiet too.
    pub(super) fn announce_launch_restore(&mut self, first_output_resolved: bool) {
        if self.session.launch_restore_notice_settled() {
            return;
        }

        let daemon_toggle =
            std::env::var_os(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV).is_some();
        let step = launch_restore_step(
            daemon_toggle,
            self.session.has_loaded_board_data(),
            first_output_resolved,
        );

        if step.announce {
            self.input_state.announce_restored_annotations();
        }
        if step.settle {
            self.session.settle_launch_restore_notice();
        }
    }

    pub(super) fn notify_output_transition_deferred(&mut self) {
        if !self.session.mark_output_transition_notified() {
            return;
        }
        self.input_state.push_toast(ToastPriority::Info, "output", Toast::warning("Session switch deferred until the active drawing is committed and the current session is saved."));
        self.input_state.needs_redraw = true;
    }

    pub(super) fn output_transition_failure_backoff(&self) -> Duration {
        self.session_options()
            .map_or(Duration::from_secs(1), |options| {
                options.autosave_failure_backoff
            })
    }

    pub(super) fn handle_session_load_outcome_for_options(
        &mut self,
        outcome: session::LoadSnapshotOutcome,
        options: &session::SessionOptions,
        context: &str,
    ) -> anyhow::Result<()> {
        match outcome {
            session::LoadSnapshotOutcome::Loaded(snapshot) => {
                debug!(
                    "Restoring session {} from {}",
                    context,
                    options.session_file_path().display()
                );
                replace_output_session_snapshot(
                    &mut self.input_state,
                    self.render.text_measurer(),
                    Some(*snapshot),
                    options,
                )?;
            }
            session::LoadSnapshotOutcome::LoadedFromBackup(snapshot) => {
                warn!(
                    "Restoring session {} from backup {} because the primary session had no board data",
                    context,
                    options.backup_file_path().display()
                );
                replace_output_session_snapshot(
                    &mut self.input_state,
                    self.render.text_measurer(),
                    Some(*snapshot),
                    options,
                )?;
                self.input_state.push_toast(ToastPriority::Info, "output", Toast::warning("Restored drawings from the session backup; the primary session had no board data."));
            }
            session::LoadSnapshotOutcome::LoadedFromRecovery(snapshot) => {
                debug!(
                    "Restoring session {} from recovery artifact {}",
                    context,
                    options.recovery_file_path().display()
                );
                replace_output_session_snapshot(
                    &mut self.input_state,
                    self.render.text_measurer(),
                    Some(*snapshot),
                    options,
                )?;
                self.input_state.push_toast(ToastPriority::Info, "output", Toast::warning("Restored session from recovery file; normal save previously exceeded the size limit."));
            }
            session::LoadSnapshotOutcome::Empty => {
                debug!(
                    "No session data found for {} ({})",
                    options.session_file_path().display(),
                    context
                );
                replace_output_session_snapshot(
                    &mut self.input_state,
                    self.render.text_measurer(),
                    None,
                    options,
                )?;
            }
            session::LoadSnapshotOutcome::EmptyAfterCorruption { backup_path } => {
                // An empty canvas here is indistinguishable from "no session
                // yet", so without this the user's drawings appear to have
                // vanished and only the log says the bytes were kept.
                warn!(
                    "Session {} could not be read for {}; its bytes were preserved at {}",
                    options.session_file_path().display(),
                    context,
                    backup_path.display()
                );
                replace_output_session_snapshot(
                    &mut self.input_state,
                    self.render.text_measurer(),
                    None,
                    options,
                )?;
                self.input_state.push_toast(
                    ToastPriority::Critical,
                    "session.corrupt",
                    Toast::error(format!(
                        "Previous session could not be read; a copy was saved to {}",
                        backup_path.display()
                    ))
                    .duration_ms(20_000),
                );
            }
            session::LoadSnapshotOutcome::NonRegularArtifact { path } => {
                debug!(
                    "Skipping non-regular session artifact {} for {}",
                    path.display(),
                    context
                );
                replace_output_session_snapshot(
                    &mut self.input_state,
                    self.render.text_measurer(),
                    None,
                    options,
                )?;
            }
            session::LoadSnapshotOutcome::ExpandedTooLarge {
                path,
                max_expanded_size,
            } => {
                replace_output_session_snapshot(
                    &mut self.input_state,
                    self.render.text_measurer(),
                    None,
                    options,
                )?;
                self.session.protect_session_path(path.clone());
                if self.session.mark_expanded_load_notified(&path) {
                    notification::send_notification_async(
                        &self.tokio_handle,
                        "Session Too Large to Restore".to_string(),
                        format!(
                            "The saved session was left unchanged because it expands beyond the {} MiB safety cap. Clear the session or move {} if it is no longer needed.",
                            max_expanded_size / 1024 / 1024,
                            path.display()
                        ),
                        Some("dialog-warning".to_string()),
                    );
                }
            }
        }
        self.refresh_runtime_ui_config_seeds();
        self.mark_clean_after_session_load();
        Ok(())
    }

    fn mark_clean_after_session_load(&mut self) {
        self.input_state.clear_session_dirty();
        self.session.mark_clean_after_load();
    }

    pub(super) fn should_skip_protected_session_save(
        &self,
        options: &session::SessionOptions,
    ) -> bool {
        let session_path = options.session_file_path();
        let skip = self.session.should_skip_save_for_protected_path(
            &session_path,
            self.input_state.is_session_dirty(),
        );
        if skip {
            info!(
                "Skipping session save to {} because a previous oversized compressed session was left protected and no session changes have been made",
                session_path.display()
            );
        }
        skip
    }

    pub(super) fn should_skip_unloaded_contentless_session_save(
        &mut self,
        options: &session::SessionOptions,
        snapshot: Option<&SessionSnapshot>,
    ) -> anyhow::Result<bool> {
        let has_board_data = snapshot.is_some_and(SessionSnapshot::has_board_data);
        if has_board_data
            || self.session.has_loaded_board_data()
            || self.session.is_dirty()
            || self.input_state.is_session_dirty()
        {
            return Ok(false);
        }
        let outcome = session_save::run_persistence_operation(
            self,
            PersistenceOperation::HasArtifacts {
                options: options.clone(),
            },
        )?;
        let PersistenceOutcome::HasArtifacts(has_artifacts) = outcome else {
            return Err(anyhow::anyhow!("unexpected artifact-inspection outcome"));
        };
        let skip = runtime_session::should_skip_unloaded_contentless_save(
            self.session.has_loaded_board_data(),
            self.session.is_dirty(),
            self.input_state.is_session_dirty(),
            has_board_data,
            has_artifacts,
        );
        if skip {
            info!(
                "Skipping session save to {} because no session was loaded, no session changes were recorded, and the current snapshot has no board data",
                options.session_file_path().display()
            );
        }
        Ok(skip)
    }

    pub(super) fn session_persistence_enabled(options: &session::SessionOptions) -> bool {
        options.any_enabled() || options.restore_tool_state || options.persist_history
    }
}

/// What one launch-time load does about the restored-ink notice.
#[derive(Debug, PartialEq, Eq)]
struct LaunchRestoreStep {
    /// Show the notice now.
    announce: bool,
    /// Stop considering later loads for this launch.
    settle: bool,
}

/// Only a fresh launch (not a daemon toggle) whose load brought board data
/// back announces. The launch settles once the notice is shown, on a daemon
/// toggle, or once the first output transition resolves without restoring
/// ink, so a later switch to another output stays quiet.
fn launch_restore_step(
    daemon_toggle: bool,
    loaded_board_data: bool,
    first_output_resolved: bool,
) -> LaunchRestoreStep {
    LaunchRestoreStep {
        announce: !daemon_toggle && loaded_board_data,
        settle: daemon_toggle || loaded_board_data || first_output_resolved,
    }
}

#[cfg(test)]
mod restore_notice_tests {
    use super::{LaunchRestoreStep, launch_restore_step};

    fn step(announce: bool, settle: bool) -> LaunchRestoreStep {
        LaunchRestoreStep { announce, settle }
    }

    #[test]
    fn a_fresh_launch_announces_the_load_that_restores_board_data() {
        assert_eq!(launch_restore_step(false, true, false), step(true, true));
        assert_eq!(launch_restore_step(false, true, true), step(true, true));
    }

    #[test]
    fn an_empty_initial_load_waits_for_the_first_output_transition() {
        assert_eq!(launch_restore_step(false, false, false), step(false, false));
        assert_eq!(launch_restore_step(false, false, true), step(false, true));
    }

    #[test]
    fn a_daemon_toggle_stays_quiet_and_settles() {
        assert_eq!(launch_restore_step(true, true, false), step(false, true));
        assert_eq!(launch_restore_step(true, false, false), step(false, true));
    }
}
