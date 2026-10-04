use super::*;
use crate::backend::wayland::session::RuntimeHomeSessionReport;
use crate::session::{SessionOptions, SessionSnapshot};

impl WaylandState {
    /// Returns to the home session the way Open switches session: the current
    /// session is saved first, and nothing changes unless home loads.
    pub(super) fn handle_toolbar_open_home_session(&mut self) {
        self.clear_toolbar_save_as_overwrite_prompt();
        let command = SessionCommand::OpenHome(self.home_session_options().map(Box::new));
        if let Err(error) = self.start_session_command(command) {
            self.report_session_command_error("Return to the home session failed", &error);
        }
    }

    /// Home's options for the output the overlay is on now, not those of the
    /// named session it is leaving.
    fn home_session_options(&self) -> Option<SessionOptions> {
        let mut options = self.session_home.options()?.clone();
        let output_identity = self
            .surface
            .current_output()
            .as_ref()
            .and_then(|output| self.output_identity_for(output));
        options.set_output_identity(output_identity.as_deref());
        Some(options)
    }

    pub(super) fn finish_open_home_session(&mut self, report: RuntimeHomeSessionReport) {
        let applied = match (report.options, report.outcome) {
            (Some(options), Some(outcome)) => {
                let loaded_board_data = outcome.has_board_data();
                self.handle_session_load_outcome_for_options(outcome, &options, "home session")
                    .map(|()| {
                        self.input_state
                            .set_session_preflight_options(Some(options.clone()));
                        self.session
                            .commit_output_options(options, loaded_board_data);
                    })
            }
            _ => self.leave_persistence_for_home(),
        };
        if let Err(error) = applied {
            self.report_session_command_error("Return to the home session failed", &error);
            return;
        }

        self.session_target_committed();
        self.set_session_toolbar_info(format!("Returned to {}", self.session_home.label()));
    }

    /// Home has persistence disabled: the run continues on an empty canvas
    /// that is not saved, as it would have started.
    fn leave_persistence_for_home(&mut self) -> Result<()> {
        let current = self
            .session_options()
            .cloned()
            .context("no session to return home from")?;
        let empty = SessionSnapshot {
            active_board_id: self.input_state.board_id().to_string(),
            boards: Vec::new(),
            tool_state: None,
        };
        crate::session::apply_snapshot_replacing_boards(
            &mut self.input_state,
            self.render.text_measurer(),
            empty,
            &current,
        )?;
        self.input_state.set_session_preflight_options(None);
        self.input_state.clear_session_dirty();
        self.session.commit_without_persistence();
        self.refresh_runtime_ui_config_seeds();
        Ok(())
    }
}
