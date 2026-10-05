use super::super::super::state::WaylandState;
use crate::{
    backend::wayland::session::{
        self as runtime_session, PersistenceCompletion, PersistenceOperation, PersistenceOutcome,
        SaveStrategy, SessionState, SubmitFailure,
    },
    session,
    session::SaveSnapshotReport,
};
use std::time::{Duration, Instant};

mod diagnostics;
mod final_save;
mod interaction;
mod notifications;
use diagnostics::{SessionSaveReason, log_session_save_result, log_snapshot_capture};

use final_save::persist_final_session_and_shutdown;
pub(in crate::backend::wayland) use interaction::should_defer_for_interaction;
use interaction::{
    defer_autosave_for_active_interaction, finalize_spotlight_wheel_for_shutdown_persistence,
    min_optional_timeout,
};
#[cfg(test)]
use interaction::{
    defer_pending_autosave_for_interaction, input_persistence_interaction_active,
    persistence_interaction_active,
};

pub(super) use notifications::notify_session_failure;
#[cfg(test)]
pub(in crate::backend::wayland) use notifications::record_autosave_failure;
#[cfg(not(test))]
use notifications::record_autosave_failure;
#[cfg(test)]
use notifications::record_autosave_success;
#[cfg(test)]
use notifications::{
    SessionSaveNotification, pending_save_notifications, session_save_notification_text,
};
use notifications::{
    notify_persistence_worker_failure, notify_session_save_report,
    show_persistence_worker_failure_toast, show_session_failure_toast,
};

pub(super) fn persist_session(state: &mut WaylandState) -> Result<(), anyhow::Error> {
    finalize_spotlight_wheel_for_shutdown_persistence(
        &mut state.input_state,
        state.spotlight.wheel_idle_deadline_mut(),
    );
    runtime_session::driver::persist_after_pending_commands(
        state,
        persist_final_session_and_shutdown,
    )
}

pub(super) fn autosave_timeout(state: &WaylandState, now: Instant) -> Option<Duration> {
    if state.session_transaction.is_some() {
        // A queued command may outlive a failed autosave completion. Admit it
        // on the next tick even when no worker wake remains outstanding.
        return (!state.persistence.is_active()).then_some(Duration::ZERO);
    }
    let autosave = scheduled_autosave_timeout(
        &state.session,
        state.session_options(),
        state.persistence.is_healthy(),
        now,
    );
    let output_transition = state
        .persistence
        .is_healthy()
        .then(|| state.session.output_transition_timeout(now))
        .flatten();
    min_optional_timeout(autosave, output_transition)
}

fn scheduled_autosave_timeout(
    session: &SessionState,
    options: Option<&session::SessionOptions>,
    worker_healthy: bool,
    now: Instant,
) -> Option<Duration> {
    if !worker_healthy {
        return None;
    }
    options.and_then(|options| session.autosave_timeout(now, options))
}

pub(super) fn autosave_if_due(state: &mut WaylandState, now: Instant) -> Result<(), anyhow::Error> {
    let completion_result = drain_persistence_completion(state);
    observe_input_dirty(state, now);
    state.poll_pending_session_command();
    completion_result?;
    if state.session_transaction.is_some() {
        return Ok(());
    }

    if !state.persistence.is_healthy() {
        return Ok(());
    }

    if state.retry_pending_output_transition_if_due(now)? {
        return Ok(());
    }

    if state
        .reconcile_live_source_interaction_if_idle("post-interaction persistence reconciliation")
    {
        return Ok(());
    }

    let Some(options) = state.session_options().cloned() else {
        return Ok(());
    };

    let interaction_active = should_defer_for_interaction(state);
    if defer_autosave_for_active_interaction(&mut state.session, now, &options, interaction_active)
    {
        return Ok(());
    }

    if !state.session.autosave_due(now, &options) {
        return Ok(());
    }

    if !state
        .session
        .validate_source_write(state.input_state.is_session_dirty())?
    {
        return Ok(());
    }
    let started = Instant::now();
    let snapshot_started = Instant::now();
    let snapshot = state
        .input_state
        .snapshot_for_persistence_with(state.render.text_measurer(), &options);
    log_snapshot_capture(
        SessionSaveReason::Autosave,
        &options,
        snapshot.as_ref(),
        snapshot_started.elapsed(),
    );

    if should_skip_protected_session_save(state, &options) {
        return Ok(());
    }
    let snapshot = snapshot_or_empty(state, &options, snapshot)?;
    let operation = PersistenceOperation::Save {
        snapshot,
        options: options.clone(),
        strategy: SaveStrategy::Autosave,
        contentless_clear_boundary: state.session.has_loaded_board_data(),
    };
    let dirty_window = state.session.prepare_autosave_submission()?;
    match state
        .persistence
        .try_submit(state.session.target_epoch(), operation)
    {
        Ok(request_id) => {
            state
                .session
                .commit_autosave_submission(request_id, dirty_window);
            log::debug!(
                "Submitted autosave request {:?} for generation {} in {:?}",
                request_id,
                state.session.edit_generation(),
                started.elapsed()
            );
        }
        Err(SubmitFailure { error, operation }) => {
            let err = anyhow::anyhow!("failed to submit autosave: {error}");
            let drop_started = Instant::now();
            drop(operation);
            log::warn!(
                "Dropped rejected autosave request payload on the event-loop thread in {:?}",
                drop_started.elapsed()
            );
            let failed_at = Instant::now();
            if !state.persistence.is_healthy() {
                handle_persistence_transport_failure(state, failed_at, &err);
            } else if record_autosave_failure(&mut state.session, failed_at, &options) {
                show_session_failure_toast(state, &err);
                notify_session_failure(state, &err);
            }
            return Err(err);
        }
    }
    Ok(())
}

fn final_save_barrier_policy(
    barrier_result: Result<(), anyhow::Error>,
    worker_healthy: bool,
) -> Result<Option<anyhow::Error>, anyhow::Error> {
    match barrier_result {
        Ok(()) => Ok(None),
        Err(err) if worker_healthy => Ok(Some(err)),
        Err(err) => Err(err),
    }
}

fn snapshot_or_empty(
    state: &WaylandState,
    options: &session::SessionOptions,
    snapshot: Option<session::SessionSnapshot>,
) -> Result<session::SessionSnapshot, anyhow::Error> {
    if let Some(snapshot) = snapshot {
        return Ok(snapshot);
    }

    if !persistence_enabled(options) {
        return Err(anyhow::anyhow!(
            "session has pending persistence work but all persistence is disabled"
        ));
    }

    Ok(session::SessionSnapshot {
        active_board_id: state.input_state.board_id().to_string(),
        boards: Vec::new(),
        tool_state: None,
    })
}

pub(in crate::backend::wayland) use runtime_session::driver::{
    observe_input_dirty, persistence_barrier,
};

pub(in crate::backend::wayland) fn run_persistence_operation(
    state: &mut WaylandState,
    operation: PersistenceOperation,
) -> Result<PersistenceOutcome, anyhow::Error> {
    persistence_barrier(state)?;
    let result = state
        .persistence
        .run(state.session.target_epoch(), operation);
    state.session.observe_load_failure(&result);
    if let Err(err) = &result
        && !state.persistence.is_healthy()
    {
        handle_persistence_transport_failure(state, Instant::now(), err);
    }
    result
}

pub(in crate::backend::wayland) fn drain_persistence_completion(
    state: &mut WaylandState,
) -> Result<(), anyhow::Error> {
    drain_persistence_completion_for_runtime(state)
}

pub(in crate::backend::wayland) trait PersistenceCompletionRuntime {
    fn try_receive_persistence_completion(
        &mut self,
    ) -> Result<Option<PersistenceCompletion>, anyhow::Error>;

    fn apply_persistence_completion(
        &mut self,
        completion: PersistenceCompletion,
    ) -> Result<(), anyhow::Error>;

    fn persistence_session_options(&self) -> Option<session::SessionOptions>;

    fn persistence_session(&mut self) -> &mut SessionState;

    fn show_persistence_worker_failure(&mut self);

    fn notify_persistence_worker_failure(&mut self, err: &anyhow::Error);
}

impl PersistenceCompletionRuntime for WaylandState {
    fn try_receive_persistence_completion(
        &mut self,
    ) -> Result<Option<PersistenceCompletion>, anyhow::Error> {
        let result = self.persistence.try_receive();
        if let Err(error) = &result {
            runtime_session::driver::fail_pending_commands(self, error);
        }
        result
    }

    fn apply_persistence_completion(
        &mut self,
        completion: PersistenceCompletion,
    ) -> Result<(), anyhow::Error> {
        runtime_session::driver::apply_session_completion(self, completion)
    }

    fn persistence_session_options(&self) -> Option<session::SessionOptions> {
        self.session_options().cloned()
    }

    fn persistence_session(&mut self) -> &mut SessionState {
        &mut self.session
    }

    fn show_persistence_worker_failure(&mut self) {
        show_persistence_worker_failure_toast(self);
    }

    fn notify_persistence_worker_failure(&mut self, err: &anyhow::Error) {
        notify_persistence_worker_failure(self, err);
    }
}

pub(in crate::backend::wayland) fn drain_persistence_completion_for_runtime(
    state: &mut impl PersistenceCompletionRuntime,
) -> Result<(), anyhow::Error> {
    let completion = match state.try_receive_persistence_completion() {
        Ok(completion) => completion,
        Err(err) => {
            handle_persistence_transport_failure_for_runtime(state, Instant::now(), &err);
            return Err(err);
        }
    };
    if let Some(completion) = completion {
        state.apply_persistence_completion(completion)?;
    }
    Ok(())
}

pub(in crate::backend::wayland) fn report_autosave_success(
    state: &mut WaylandState,
    save: runtime_session::SaveCompletion,
    execution_time: Duration,
) {
    log_session_save_result(
        SessionSaveReason::Autosave,
        save.report.as_ref(),
        execution_time,
    );
    notify_session_save_report(state, save.report.as_ref());
}

pub(in crate::backend::wayland) fn handle_autosave_failure(
    state: &mut WaylandState,
    now: Instant,
    err: &anyhow::Error,
) {
    let Some(options) = state.session_options().cloned() else {
        return;
    };
    if record_autosave_failure(&mut state.session, now, &options) {
        show_session_failure_toast(state, err);
        notify_session_failure(state, err);
    }
}

pub(in crate::backend::wayland) fn handle_persistence_transport_failure(
    state: &mut WaylandState,
    now: Instant,
    err: &anyhow::Error,
) {
    handle_persistence_transport_failure_for_runtime(state, now, err);
}

fn handle_persistence_transport_failure_for_runtime(
    state: &mut impl PersistenceCompletionRuntime,
    now: Instant,
    err: &anyhow::Error,
) {
    let options = state.persistence_session_options();
    if record_persistence_transport_failure(state.persistence_session(), options.as_ref(), now) {
        state.show_persistence_worker_failure();
        state.notify_persistence_worker_failure(err);
    }
}

fn record_persistence_transport_failure(
    session: &mut SessionState,
    options: Option<&session::SessionOptions>,
    now: Instant,
) -> bool {
    let restored_autosave = session.restore_in_flight_autosave();
    let Some(options) = options else {
        return false;
    };
    if restored_autosave {
        let _ = record_autosave_failure(session, now, options);
    }
    session.mark_worker_failure_notified()
}

fn persistence_enabled(options: &session::SessionOptions) -> bool {
    options.any_enabled() || options.restore_tool_state || options.persist_history
}

fn should_skip_disabled_final_save(options: &session::SessionOptions) -> bool {
    let skip = !persistence_enabled(options);
    if skip {
        log::info!(
            "Skipping final session save because all session persistence options are disabled"
        );
    }
    skip
}

fn should_skip_protected_session_save(
    state: &WaylandState,
    options: &session::SessionOptions,
) -> bool {
    let session_path = options.session_file_path();
    let skip = state
        .session
        .should_skip_save_for_protected_path(&session_path, state.input_state.is_session_dirty());
    if skip {
        if state.session.refuses_source_write(&session_path) {
            log::warn!(
                "Skipping session save to {} because unreadable data could not be preserved; repair and reload the session or use Save As",
                session_path.display()
            );
        } else {
            log::info!(
                "Skipping session save to {} because an oversized session is protected and no session changes have been made",
                session_path.display()
            );
        }
    }
    skip
}

fn should_skip_unloaded_contentless_save(
    state: &mut WaylandState,
    options: &session::SessionOptions,
    snapshot: Option<&session::SessionSnapshot>,
) -> Result<bool, anyhow::Error> {
    let has_board_data = snapshot.is_some_and(session::SessionSnapshot::has_board_data);
    if has_board_data
        || state.session.has_loaded_board_data()
        || state.session.is_dirty()
        || state.input_state.is_session_dirty()
    {
        return Ok(false);
    }
    let outcome = run_persistence_operation(
        state,
        PersistenceOperation::HasArtifacts {
            options: options.clone(),
        },
    )?;
    let PersistenceOutcome::HasArtifacts(has_artifacts) = outcome else {
        return Err(anyhow::anyhow!(
            "unexpected session artifact inspection outcome"
        ));
    };
    let skip = runtime_session::should_skip_unloaded_contentless_save(
        state.session.has_loaded_board_data(),
        state.session.is_dirty(),
        state.input_state.is_session_dirty(),
        has_board_data,
        has_artifacts,
    );
    if skip {
        log::warn!(
            "Skipping session save to {} because no session was loaded, no session changes were recorded, and the current snapshot has no board data",
            options.session_file_path().display()
        );
    }
    Ok(skip)
}

#[cfg(test)]
mod tests;
