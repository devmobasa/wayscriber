use anyhow::Result;

use super::super::*;
use crate::backend::wayland::backend::event_loop::session_save::{
    handle_autosave_failure, handle_persistence_transport_failure, report_autosave_success,
};
use crate::backend::wayland::session::{
    PersistenceController, SaveCompletion, SessionCommand, SessionCommandReport,
    SessionCommandTransaction, SessionTransaction,
    driver::{self, SessionCommandRuntime},
};
use crate::session::ToolStateSnapshot;
use std::time::{Duration, Instant};

impl SessionCommandRuntime for WaylandState {
    fn session_context(&mut self) -> SessionTransaction<'_> {
        SessionTransaction {
            input_state: &mut self.input_state,
            measurer: self.render.text_measurer(),
            session: &mut self.session,
        }
    }

    fn pending_command(&mut self) -> &mut Option<SessionCommandTransaction> {
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

    fn session_command_queued(&mut self) {
        self.input_state.push_toast(crate::input::state::ToastPriority::Action, "session.command", crate::input::state::Toast::info("Session command queued for the visible session. If the session changes, retry it on the intended session."));
    }

    fn session_target_committed(&mut self) {
        self.report_session_to_daemon();
    }

    fn finish_session_command(&mut self, report: SessionCommandReport) {
        WaylandState::finish_session_command(self, report);
    }

    fn fail_session_command(&mut self, command: &SessionCommand, error: &anyhow::Error) {
        WaylandState::fail_session_command(self, command, error);
    }

    fn fail_queued_commands(
        &mut self,
        failures: Vec<(SessionCommand, anyhow::Error)>,
        closing: bool,
    ) {
        WaylandState::fail_queued_session_commands(self, failures, closing);
    }

    fn session_transport_failed(&mut self, error: &anyhow::Error) {
        handle_persistence_transport_failure(self, Instant::now(), error);
    }

    fn autosave_succeeded(&mut self, save: SaveCompletion, execution_time: Duration) {
        report_autosave_success(self, save, execution_time);
    }

    fn autosave_failed(&mut self, error: &anyhow::Error) {
        handle_autosave_failure(self, Instant::now(), error);
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

    /// Runs the pending command and any queued behind it to completion,
    /// blocking dispatch, as startup and shutdown do.
    pub(in crate::backend::wayland) fn finish_session_command_blocking(&mut self) -> Result<()> {
        let finished = driver::finish_pending_session_command(self);
        if let Err(error) = &finished {
            driver::fail_pending_commands(self, error);
        }
        finished
    }

    pub(in crate::backend::wayland) fn handle_clear_saved_tool_state_action(&mut self) {
        let command =
            SessionCommand::ClearTools(Box::new(ToolStateSnapshot::from_config(&self.config)));
        if let Err(error) = self.start_session_command(command) {
            self.report_session_command_error("Failed to reset tool defaults", &error);
        }
    }
}
