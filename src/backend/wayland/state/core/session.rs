use anyhow::{Result, anyhow};
use std::time::Instant;

use super::super::*;
use crate::backend::wayland::{
    backend::event_loop::session_save,
    session::{
        ExplicitSessionTransaction, PersistenceCompletion, SessionCommand, SessionCommandReport,
        SessionTransaction, TransactionStep,
    },
};
use crate::session::ToolStateSnapshot;

impl WaylandState {
    /// Admit one explicit command. An existing autosave finishes first without
    /// waiting in dispatch; the command captures its live input when admitted.
    pub(in crate::backend::wayland) fn start_session_command(
        &mut self,
        command: SessionCommand,
    ) -> Result<()> {
        if self.session_transaction.is_some() {
            return Err(anyhow!("another session command is already pending"));
        }
        if matches!(
            command,
            SessionCommand::Clear | SessionCommand::ClearTools(_)
        ) {
            ensure_destructive_session_config_available(self.session_config_failed)?;
        }
        if !self.persistence.is_healthy() {
            return Err(anyhow!("session persistence worker is unhealthy"));
        }
        self.session_transaction = Some(ExplicitSessionTransaction::new(
            command,
            self.session.target_epoch(),
            self.input_state.session_interaction_state(),
        ));
        self.poll_pending_session_command();
        Ok(())
    }

    pub(in crate::backend::wayland) fn poll_pending_session_command(&mut self) {
        if self.persistence.is_active() {
            return;
        }
        let Some(transaction) = self.session_transaction.take() else {
            return;
        };
        self.advance_session_command(transaction, None);
    }

    pub(in crate::backend::wayland) fn complete_session_command(
        &mut self,
        completion: PersistenceCompletion,
    ) {
        let Some(transaction) = self.session_transaction.take() else {
            return;
        };
        if transaction.request_id != Some(completion.id) {
            self.fail_session_command(
                transaction.command(),
                &anyhow!("explicit session completion identity mismatch"),
            );
            return;
        }
        self.advance_session_command(transaction, Some(completion.result));
    }

    fn advance_session_command(
        &mut self,
        mut transaction: ExplicitSessionTransaction,
        result: Option<Result<crate::backend::wayland::session::PersistenceOutcome>>,
    ) {
        session_save::observe_input_dirty(self, Instant::now());
        // A catalog failure follows an already committed open. It is reported
        // independently, without reverting the canvas or its current edits.
        let step = if let Some(Err(error)) = result.as_ref()
            && let Some(report) = transaction.accept_catalog_failure(error)
        {
            Ok(TransactionStep::Complete(Box::new(report)))
        } else {
            transaction.advance(
                &mut SessionTransaction {
                    input_state: &mut self.input_state,
                    measurer: self.render.text_measurer(),
                    session: &mut self.session,
                },
                result,
            )
        };
        match step {
            Ok(TransactionStep::Work(operation)) => {
                // Applying an open refreshes consumer seeds before catalog work.
                if transaction.has_committed_open() {
                    self.refresh_runtime_ui_config_seeds();
                }
                match self
                    .persistence
                    .try_submit(self.session.target_epoch(), *operation)
                {
                    Ok(id) => {
                        transaction.request_id = Some(id);
                        self.session_transaction = Some(transaction);
                    }
                    Err(failure) => {
                        let error = anyhow!("failed to submit session command: {}", failure.error);
                        if let Some(report) = transaction.accept_catalog_failure(&error) {
                            self.finish_session_command(report);
                        } else {
                            self.fail_session_command(transaction.command(), &error);
                        }
                    }
                }
            }
            Ok(TransactionStep::Complete(report)) => {
                if matches!(
                    *report,
                    SessionCommandReport::Open(_) | SessionCommandReport::Clear(_)
                ) {
                    self.refresh_runtime_ui_config_seeds();
                }
                self.finish_session_command(*report);
            }
            Err(error) => self.fail_session_command(transaction.command(), &error),
        }
    }

    pub(in crate::backend::wayland) fn handle_clear_saved_tool_state_action(&mut self) {
        let command =
            SessionCommand::ClearTools(Box::new(ToolStateSnapshot::from_config(&self.config)));
        if let Err(error) = self.start_session_command(command) {
            self.report_session_command_error("Failed to reset tool defaults", &error);
        }
    }

    /// Shutdown may wait for durable work; normal dispatch never calls this.
    pub(in crate::backend::wayland) fn finish_pending_session_command(&mut self) -> Result<()> {
        while self.session_transaction.is_some() {
            if self.persistence.is_active() {
                session_save::persistence_barrier(self)?;
            } else {
                self.poll_pending_session_command();
            }
        }
        Ok(())
    }
}

fn ensure_destructive_session_config_available(section_failed: bool) -> Result<()> {
    if section_failed {
        return Err(anyhow!(
            "config.toml [session] could not be read; refusing to modify saved session data that default settings may mistarget - fix the section and retry"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn destructive_session_actions_fail_closed_after_session_config_fallback() {
        let err = ensure_destructive_session_config_available(true).unwrap_err();
        assert!(format!("{err:#}").contains("refusing to modify saved session data"));
        ensure_destructive_session_config_available(false).unwrap();
    }
}
