use anyhow::Result;

use super::super::*;
use crate::backend::wayland::session::{
    ExplicitSessionTransaction, PersistenceCompletion, PersistenceController, SessionCommand,
    SessionCommandReport, SessionTransaction,
    driver::{self, SessionCommandRuntime},
};
use crate::session::ToolStateSnapshot;

impl SessionCommandRuntime for WaylandState {
    fn session_context(&mut self) -> SessionTransaction<'_> {
        SessionTransaction {
            input_state: &mut self.input_state,
            measurer: self.render.text_measurer(),
            session: &mut self.session,
        }
    }

    fn pending_command(&mut self) -> &mut Option<ExplicitSessionTransaction> {
        &mut self.session_transaction
    }

    fn persistence(&mut self) -> &mut PersistenceController {
        &mut self.persistence
    }

    fn session_config_failed(&self) -> bool {
        self.session_config_failed
    }

    fn refresh_session_ui_seeds(&mut self) {
        self.refresh_runtime_ui_config_seeds();
    }

    fn finish_session_command(&mut self, report: SessionCommandReport) {
        WaylandState::finish_session_command(self, report);
    }

    fn fail_session_command(&mut self, command: &SessionCommand, error: &anyhow::Error) {
        WaylandState::fail_session_command(self, command, error);
    }

    fn session_transport_failed(&mut self, error: &anyhow::Error) {
        crate::backend::wayland::backend::event_loop::session_save::handle_persistence_transport_failure(
            self, std::time::Instant::now(), error,
        );
    }

    fn apply_session_completion(&mut self, completion: PersistenceCompletion) -> Result<()> {
        crate::backend::wayland::backend::event_loop::session_save::apply_persistence_completion(
            self, completion,
        )
    }
}

impl WaylandState {
    /// Admit without waiting; an outstanding autosave completes before this command advances.
    pub(in crate::backend::wayland) fn start_session_command(
        &mut self,
        command: SessionCommand,
    ) -> Result<()> {
        driver::start_session_command(self, command)
    }

    pub(in crate::backend::wayland) fn poll_pending_session_command(&mut self) {
        driver::poll_pending_session_command(self);
    }

    pub(in crate::backend::wayland) fn handle_clear_saved_tool_state_action(&mut self) {
        let command =
            SessionCommand::ClearTools(Box::new(ToolStateSnapshot::from_config(&self.config)));
        if let Err(error) = self.start_session_command(command) {
            self.report_session_command_error("Failed to reset tool defaults", &error);
        }
    }
}
