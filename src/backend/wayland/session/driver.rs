//! Live session-command orchestration, shared by Wayland and headless runtime tests.
use anyhow::{Result, anyhow};
use std::time::{Duration, Instant};

use super::{
    PersistenceCompletion, PersistenceController, PersistenceOutcome, QueuedSessionCommand,
    SaveCompletion, SessionCommand, SessionCommandReport, SessionCommandTransaction,
    SessionTransaction, TransactionStep,
};

/// The driver owns ordering and identity; adapters own UI publication and autosave feedback.
pub(in crate::backend::wayland) trait SessionCommandRuntime {
    fn session_context(&mut self) -> SessionTransaction<'_>;
    fn pending_command(&mut self) -> &mut Option<SessionCommandTransaction>;
    fn persistence(&mut self) -> &mut PersistenceController;
    fn session_config_failed(&self) -> bool;
    fn refresh_session_ui_seeds(&mut self);
    /// An explicit command committed its target, or finished without changing
    /// it. Called before the command's terminal report.
    fn session_target_committed(&mut self);
    fn session_command_queued(&mut self);
    fn finish_session_command(&mut self, report: SessionCommandReport);
    fn fail_session_command(&mut self, command: &SessionCommand, error: &anyhow::Error);
    /// Queued commands that will not run, reported together so that one
    /// refusal does not hide another. `closing` says Wayscriber is exiting.
    fn fail_queued_commands(
        &mut self,
        failures: Vec<(SessionCommand, anyhow::Error)>,
        _closing: bool,
    ) {
        for (command, error) in &failures {
            self.fail_session_command(command, error);
        }
    }
    fn autosave_succeeded(&mut self, save: SaveCompletion, execution_time: Duration);
    fn autosave_failed(&mut self, error: &anyhow::Error);
    fn session_transport_failed(&mut self, error: &anyhow::Error);
}

/// Guard aborts retain the latest destination without treating normal input as an I/O failure.
pub(in crate::backend::wayland) fn defer_failed_output(
    session: &mut super::SessionState,
    error: &anyhow::Error,
    now: Instant,
    failure_backoff: Duration,
) -> bool {
    let guard_abort = error
        .downcast_ref::<super::SessionCommandAborted>()
        .is_some();
    let delay = if guard_abort {
        super::interaction_defer_interval()
    } else {
        failure_backoff
    };
    session.defer_output_transition(now, delay);
    !guard_abort
}

pub(in crate::backend::wayland) fn observe_input_dirty(
    runtime: &mut impl SessionCommandRuntime,
    now: Instant,
) {
    let context = runtime.session_context();
    let dirty = context.input_state.take_session_dirty();
    context.session.record_input_dirty(now, dirty);
}

pub(in crate::backend::wayland) fn start_session_command(
    runtime: &mut impl SessionCommandRuntime,
    command: SessionCommand,
) -> Result<()> {
    if matches!(
        command,
        SessionCommand::Clear | SessionCommand::ClearTools(_)
    ) && runtime.session_config_failed()
    {
        return Err(anyhow!(
            "config.toml [session] could not be read; refusing to modify saved session data that default settings may mistarget - fix the section and retry"
        ));
    }

    // Edits made before the request are part of what it was asked over.
    observe_input_dirty(runtime, Instant::now());
    let context = runtime.session_context();
    let epoch = context.session.target_epoch();
    let generation = context.session.edit_generation();

    if let Some(pending) = runtime.pending_command().as_mut() {
        if matches!(pending.command(), SessionCommand::Output { .. })
            && !matches!(command, SessionCommand::Output { .. })
        {
            if !pending
                .queued_commands
                .iter()
                .any(|queued| queued.epoch == epoch && queued.command.matches_request(&command))
            {
                if pending.queued_commands.len() >= 8 {
                    return Err(anyhow!(
                        "session command queue is full; wait for the output load and retry"
                    ));
                }
                pending.queued_commands.push_back(QueuedSessionCommand {
                    command,
                    epoch,
                    generation,
                });
            }
            runtime.session_command_queued();
            return Ok(());
        }
        return Err(anyhow!("another session command is already pending"));
    }
    if !runtime.persistence().is_healthy() {
        return Err(anyhow!("session persistence worker is unhealthy"));
    }

    let context = runtime.session_context();
    let transaction = SessionCommandTransaction::new(
        command,
        context.session.target_epoch(),
        context.input_state.session_interaction_state(),
    );
    *runtime.pending_command() = Some(transaction);
    poll_pending_session_command(runtime);
    Ok(())
}

pub(in crate::backend::wayland) fn poll_pending_session_command(
    runtime: &mut impl SessionCommandRuntime,
) {
    if runtime.persistence().is_active() {
        return;
    }

    let Some(transaction) = runtime.pending_command().take() else {
        return;
    };

    advance_session_command(runtime, transaction, None);
}

pub(in crate::backend::wayland) fn complete_session_command(
    runtime: &mut impl SessionCommandRuntime,
    completion: PersistenceCompletion,
) {
    let Some(mut transaction) = runtime.pending_command().take() else {
        return;
    };
    if transaction.request_id != Some(completion.id) {
        runtime.fail_session_command(
            transaction.command(),
            &anyhow!("explicit session completion identity mismatch"),
        );
        start_queued_commands(runtime, std::mem::take(&mut transaction.queued_commands));
        return;
    }

    advance_session_command(runtime, transaction, Some(completion.result));
}

fn advance_session_command(
    runtime: &mut impl SessionCommandRuntime,
    mut transaction: SessionCommandTransaction,
    result: Option<Result<PersistenceOutcome>>,
) {
    observe_input_dirty(runtime, Instant::now());
    if let Some(result) = &result {
        runtime
            .session_context()
            .session
            .observe_load_failure(result);
    }

    // A submitted board or tool-state clear can change disk before its receipt fails.
    let clear_attempted = matches!(
        transaction.command(),
        SessionCommand::Clear | SessionCommand::ClearTools(_)
    ) && result.is_some();

    // Catalog failure follows an already committed open; never roll back that canvas.
    let step = match result {
        Some(Err(error)) if transaction.has_committed_open() => Ok(TransactionStep::Complete(
            Box::new(transaction.catalog_failure_report(error)),
        )),
        result => transaction.advance(&mut runtime.session_context(), result),
    };

    match step {
        Ok(TransactionStep::Work(operation)) => {
            if transaction.has_committed_open() {
                runtime.refresh_session_ui_seeds();
                runtime.session_target_committed();
            }

            let epoch = runtime.session_context().session.target_epoch();
            match runtime.persistence().try_submit(epoch, *operation) {
                Ok(id) => {
                    transaction.request_id = Some(id);
                    *runtime.pending_command() = Some(transaction);
                }
                Err(failure) => {
                    let error = anyhow::Error::new(failure.error)
                        .context("failed to submit session command");
                    if transaction.has_committed_open() {
                        runtime.finish_session_command(transaction.catalog_failure_report(error));
                    } else {
                        runtime.fail_session_command(transaction.command(), &error);
                    }
                    start_queued_commands(
                        runtime,
                        std::mem::take(&mut transaction.queued_commands),
                    );
                }
            }
        }
        Ok(TransactionStep::Complete(report)) => {
            if matches!(
                *report,
                SessionCommandReport::Open(_)
                    | SessionCommandReport::Output { .. }
                    | SessionCommandReport::Home
                    | SessionCommandReport::Clear(_)
            ) {
                runtime.refresh_session_ui_seeds();
            }

            runtime.session_target_committed();
            runtime.finish_session_command(*report);
            start_queued_commands(runtime, std::mem::take(&mut transaction.queued_commands));
        }
        Err(error) => {
            // Resave retained state if a clear may have changed disk, without postponing
            // already-dirty work or creating an undo entry for a persistence invalidation.
            let context = runtime.session_context();
            let needs_recovery = clear_attempted && !context.session.is_dirty();
            context
                .session
                .record_input_dirty(Instant::now(), needs_recovery);

            runtime.fail_session_command(transaction.command(), &error);
            start_queued_commands(runtime, std::mem::take(&mut transaction.queued_commands));
        }
    }
}

fn start_queued_commands(
    runtime: &mut impl SessionCommandRuntime,
    mut commands: std::collections::VecDeque<QueuedSessionCommand>,
) {
    if let Some(pending) = runtime.pending_command().as_mut() {
        pending.queued_commands.extend(commands);
        return;
    }

    let mut refused = Vec::new();
    while let Some(queued) = commands.pop_front() {
        if let Some(refusal) = queued_command_refusal(runtime, &queued) {
            refused.push((queued.command, refusal));
            continue;
        }

        let context = runtime.session_context();
        let mut transaction = SessionCommandTransaction::new(
            queued.command,
            context.session.target_epoch(),
            context.input_state.session_interaction_state(),
        );
        transaction.queued_commands = commands;
        *runtime.pending_command() = Some(transaction);

        if !refused.is_empty() {
            runtime.fail_queued_commands(refused, false);
        }
        poll_pending_session_command(runtime);
        return;
    }

    if !refused.is_empty() {
        runtime.fail_queued_commands(refused, false);
    }
}

fn queued_command_refusal(
    runtime: &mut impl SessionCommandRuntime,
    queued: &QueuedSessionCommand,
) -> Option<anyhow::Error> {
    if queued.epoch != runtime.session_context().session.target_epoch() {
        return Some(anyhow!(
            "the visible session changed while this command was queued; no queued edit was performed; retry on the intended session"
        ));
    }

    if matches!(
        queued.command,
        SessionCommand::Clear | SessionCommand::ClearTools(_)
    ) {
        observe_input_dirty(runtime, Instant::now());
        if runtime.session_context().session.edit_generation() != queued.generation {
            return Some(anyhow!(
                "the session changed while this command was queued; nothing was cleared; run it again to include the newer changes"
            ));
        }
    }

    if !runtime.persistence().is_healthy() {
        return Some(anyhow!("session persistence worker is unhealthy"));
    }

    None
}

pub(in crate::backend::wayland) fn fail_pending_commands(
    runtime: &mut impl SessionCommandRuntime,
    error: &anyhow::Error,
) {
    if let Some(mut transaction) = runtime.pending_command().take() {
        runtime.fail_session_command(transaction.command(), error);

        let queued = transaction
            .queued_commands
            .drain(..)
            .map(|queued| (queued.command, anyhow!("{error:#}")))
            .collect::<Vec<_>>();
        if !queued.is_empty() {
            runtime.fail_queued_commands(queued, false);
        }
    }
}

/// Apply a receipt and publish its feedback. Ownership errors return before
/// autosave failure bookkeeping; only an owned failed save incurs retry backoff.
pub(in crate::backend::wayland) fn apply_session_completion(
    runtime: &mut impl SessionCommandRuntime,
    completion: PersistenceCompletion,
) -> Result<()> {
    observe_input_dirty(runtime, Instant::now());

    let execution_time = completion.execution_time;

    if runtime
        .pending_command()
        .as_ref()
        .is_some_and(|command| command.request_id.is_some())
    {
        complete_session_command(runtime, completion);
        return Ok(());
    }

    let save_result = match completion.result {
        Ok(PersistenceOutcome::Save(save)) => Ok(save),
        Ok(other) => Err(anyhow!(
            "unexpected asynchronous persistence outcome: {other:?}"
        )),
        Err(error) => Err(error),
    };
    let committed = runtime.session_context().session.complete_autosave(
        completion.id,
        Instant::now(),
        &save_result,
    )?;

    let write_result = save_result.and_then(|save| {
        if committed {
            Ok(save)
        } else {
            Err(anyhow!(
                "autosave worker completed without writing session data"
            ))
        }
    });

    let save = match write_result {
        Ok(save) => save,
        Err(error) => {
            runtime.autosave_failed(&error);
            return Err(error);
        }
    };

    runtime.autosave_succeeded(save, execution_time);

    Ok(())
}

/// Deliberate durability barrier for startup and shutdown; event-loop output
/// switches and explicit commands advance through nonblocking transactions.
pub(in crate::backend::wayland) fn persistence_barrier(
    runtime: &mut impl SessionCommandRuntime,
) -> Result<()> {
    observe_input_dirty(runtime, Instant::now());

    if runtime.persistence().is_active() {
        let completion = match runtime.persistence().wait_for_completion() {
            Ok(Some(completion)) => completion,
            Ok(None) => return Err(anyhow!("active persistence request had no completion")),
            Err(error) => {
                runtime.session_transport_failed(&error);
                return Err(error);
            }
        };
        apply_session_completion(runtime, completion)?;
    }

    if !runtime.persistence().is_healthy() {
        return Err(anyhow!("session persistence worker is unhealthy"));
    }

    Ok(())
}

pub(in crate::backend::wayland) fn finish_pending_session_command(
    runtime: &mut impl SessionCommandRuntime,
) -> Result<()> {
    while runtime.pending_command().is_some() {
        poll_pending_session_command(runtime);
        persistence_barrier(runtime)?;
    }

    Ok(())
}

/// Shutdown must clear a failed command before attempting final persistence.
pub(in crate::backend::wayland) fn persist_after_pending_commands<R: SessionCommandRuntime, T>(
    runtime: &mut R,
    persist: impl FnOnce(&mut R) -> Result<T>,
) -> Result<T> {
    if let Some(transaction) = runtime.pending_command().as_mut() {
        let canceled = std::mem::take(&mut transaction.queued_commands)
            .into_iter()
            .map(|queued| {
                (
                    queued.command,
                    anyhow!("queued session command canceled because Wayscriber is shutting down"),
                )
            })
            .collect::<Vec<_>>();
        if !canceled.is_empty() {
            runtime.fail_queued_commands(canceled, true);
        }
    }

    if let Err(error) = finish_pending_session_command(runtime) {
        fail_pending_commands(runtime, &error);
        log::warn!("Explicit session command failed during shutdown: {error:#}");
    }

    // A running remembered-home fallback must finish and publish its target
    // before cancellation, or the final save could recreate the old file.
    runtime
        .session_context()
        .session
        .cancel_pending_output_transition();

    persist(runtime)
}

#[cfg(test)]
pub(super) mod tests;
