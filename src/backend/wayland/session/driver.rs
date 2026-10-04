//! Live explicit-command orchestration, shared by Wayland and headless runtime tests.
use anyhow::{Result, anyhow};
use std::time::{Duration, Instant};

use super::{
    ExplicitSessionTransaction, PersistenceCompletion, PersistenceController, PersistenceOutcome,
    SaveCompletion, SessionCommand, SessionCommandReport, SessionTransaction, TransactionStep,
};

/// The driver owns ordering and identity; adapters own UI publication and autosave feedback.
pub(in crate::backend::wayland) trait SessionCommandRuntime {
    fn session_context(&mut self) -> SessionTransaction<'_>;
    fn pending_command(&mut self) -> &mut Option<ExplicitSessionTransaction>;
    fn persistence(&mut self) -> &mut PersistenceController;
    fn session_config_failed(&self) -> bool;
    fn refresh_session_ui_seeds(&mut self);
    /// An explicit command committed its target, or finished without changing
    /// it. Called before the command's terminal report.
    fn session_target_committed(&mut self);
    fn finish_session_command(&mut self, report: SessionCommandReport);
    fn fail_session_command(&mut self, command: &SessionCommand, error: &anyhow::Error);
    fn autosave_succeeded(&mut self, save: SaveCompletion, execution_time: Duration);
    fn autosave_failed(&mut self, error: &anyhow::Error);
    fn session_transport_failed(&mut self, error: &anyhow::Error);
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
    if runtime.pending_command().is_some() {
        return Err(anyhow!("another session command is already pending"));
    }
    if matches!(
        command,
        SessionCommand::Clear | SessionCommand::ClearTools(_)
    ) && runtime.session_config_failed()
    {
        return Err(anyhow!(
            "config.toml [session] could not be read; refusing to modify saved session data that default settings may mistarget - fix the section and retry"
        ));
    }
    if !runtime.persistence().is_healthy() {
        return Err(anyhow!("session persistence worker is unhealthy"));
    }

    let context = runtime.session_context();
    let transaction = ExplicitSessionTransaction::new(
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
    let Some(transaction) = runtime.pending_command().take() else {
        return;
    };
    if transaction.request_id != Some(completion.id) {
        runtime.fail_session_command(
            transaction.command(),
            &anyhow!("explicit session completion identity mismatch"),
        );
        return;
    }

    advance_session_command(runtime, transaction, Some(completion.result));
}

fn advance_session_command(
    runtime: &mut impl SessionCommandRuntime,
    mut transaction: ExplicitSessionTransaction,
    result: Option<Result<PersistenceOutcome>>,
) {
    observe_input_dirty(runtime, Instant::now());

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
                }
            }
        }
        Ok(TransactionStep::Complete(report)) => {
            if matches!(
                *report,
                SessionCommandReport::Open(_) | SessionCommandReport::Clear(_)
            ) {
                runtime.refresh_session_ui_seeds();
            }

            runtime.session_target_committed();
            runtime.finish_session_command(*report);
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

/// Deliberate durability barrier, never called from normal dispatch.
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
    if let Err(error) = finish_pending_session_command(runtime) {
        if let Some(transaction) = runtime.pending_command().take() {
            runtime.fail_session_command(transaction.command(), &error);
        }
        log::warn!("Explicit session command failed during shutdown: {error:#}");
    }

    persist(runtime)
}

#[cfg(test)]
pub(super) mod tests;
