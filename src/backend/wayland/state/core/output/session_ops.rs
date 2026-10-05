use super::*;
use crate::backend::wayland::session::{
    ExpandedTooLarge, OutputSessionLoad, commit_output_load, load_output_session,
};

impl WaylandState {
    /// Loads and commits `staged` as the session for the output identified as
    /// `physical_output_identity`. Until a session has loaded, a remembered
    /// session this run continues must still be usable; otherwise the overlay
    /// starts at home instead, says so, and the daemon hears that it is home,
    /// so the next show does not try that session again.
    pub(in crate::backend::wayland) fn load_configured_session_for_options(
        &mut self,
        staged: session::SessionOptions,
        physical_output_identity: Option<&str>,
        context: &str,
    ) -> anyhow::Result<()> {
        let remembered = self.unloaded_remembered_session();
        let home = self
            .session_home
            .options_for_output(physical_output_identity);

        let load = load_output_session(staged, remembered.as_deref(), home, |operation| {
            session_save::run_persistence_operation(self, operation)
        })?;

        match load {
            OutputSessionLoad::Loaded(options, outcome) => {
                self.commit_output_session(options, outcome, context)?;
            }
            OutputSessionLoad::WentHome {
                remembered,
                reason,
                home,
            } => {
                match home {
                    Some((options, outcome)) => {
                        self.commit_output_session(options.clone(), outcome, context)?;
                        self.input_state
                            .set_session_preflight_options(Some(options));
                    }
                    None => {
                        self.session.commit_without_persistence();
                        self.input_state.set_session_preflight_options(None);
                    }
                }
                self.notify_remembered_session_abandoned(&remembered, &reason);
            }
        }

        self.report_session_to_daemon();
        Ok(())
    }

    fn commit_output_session(
        &mut self,
        options: session::SessionOptions,
        outcome: session::LoadSnapshotOutcome,
        context: &str,
    ) -> anyhow::Result<()> {
        if let Some(too_large) = commit_output_load(
            &mut self.input_state,
            self.render.text_measurer(),
            &mut self.session,
            options,
            outcome,
            context,
        )? {
            self.protect_too_large_session(too_large);
        }
        self.refresh_runtime_ui_config_seeds();
        Ok(())
    }

    /// The remembered session this run continues, while no session has loaded.
    pub(in crate::backend::wayland) fn unloaded_remembered_session(
        &self,
    ) -> Option<std::path::PathBuf> {
        self.session_home
            .remembered()
            .filter(|_| !self.session.is_loaded())
            .map(std::path::Path::to_path_buf)
    }

    /// After a launch-time session load, announce ink restored onto the
    /// transparent overlay board, once per launch. With per-output sessions
    /// the ink arrives with the first output transition rather than the
    /// initial load, so both report here; later output or named-session
    /// switches stay quiet. Overlays the daemon reopens on a toggle show what
    /// the user just had on screen, so they stay quiet too.
    pub(in crate::backend::wayland::state) fn announce_launch_restore(
        &mut self,
        first_output_resolved: bool,
    ) {
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

    pub(in crate::backend::wayland) fn notify_session_load_failure(
        &mut self,
        error: &anyhow::Error,
    ) {
        if self.session.mark_output_transition_notified() {
            self.input_state.push_toast(ToastPriority::Critical, "session.load", Toast::error(format!("Session could not be loaded: {error:#}. Repair the session files or use Save As to keep new drawings elsewhere.")).duration_ms(20_000));
        }
    }

    pub(in crate::backend::wayland::state) fn notify_output_transition_deferred(&mut self) {
        if !self.session.mark_output_transition_notified() {
            return;
        }
        self.input_state.push_toast(ToastPriority::Info, "output", Toast::warning("Session switch deferred until the active drawing is committed and the current session is saved."));
        self.input_state.needs_redraw = true;
    }

    pub(in crate::backend::wayland::state) fn output_transition_failure_backoff(&self) -> Duration {
        self.session_options()
            .map_or(Duration::from_secs(1), |options| {
                options.autosave_failure_backoff
            })
    }

    pub(in crate::backend::wayland::state) fn protect_too_large_session(
        &mut self,
        too_large: ExpandedTooLarge,
    ) {
        let ExpandedTooLarge {
            path,
            max_expanded_size,
        } = too_large;
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
