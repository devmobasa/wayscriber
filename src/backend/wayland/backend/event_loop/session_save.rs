use super::super::super::state::WaylandState;
use crate::{
    backend::wayland::session::{
        self as runtime_session, PersistenceCompletion, PersistenceOperation, PersistenceOutcome,
        SaveCompletion, SaveStrategy, SessionState, SubmitFailure,
    },
    session,
    session::SaveSnapshotReport,
};
use std::time::{Duration, Instant};

const AUTOSAVE_ACTIVE_INTERACTION_DEFER_MS: u64 = 500;

mod notifications;

pub(super) use notifications::notify_session_failure;
#[cfg(test)]
use notifications::record_autosave_success;
#[cfg(test)]
use notifications::{
    SessionSaveNotification, pending_save_notifications, session_save_notification_text,
};
use notifications::{
    notify_persistence_worker_failure, notify_session_save_report, record_autosave_failure,
    show_persistence_worker_failure_toast, show_session_failure_toast,
};

pub(super) fn persist_session(state: &mut WaylandState) -> Result<(), anyhow::Error> {
    if let Some(pending) = state.session.cancel_pending_output_transition() {
        log::info!(
            "Canceling staged output transition to {:?} during shutdown; persisting active epoch {}",
            pending.physical_output_identity,
            state.session.target_epoch()
        );
    }
    let save_result = persist_final_session(state);
    let worker_failed = !state.persistence.is_healthy();
    let shutdown_result = state.persistence.shutdown(state.session.target_epoch());
    if save_result.is_err() && worker_failed && state.persistence.is_stopped() {
        log::warn!(
            "Persistence worker failed before the final save; attempting joined event-thread fallback"
        );
        match persist_final_session_direct(state) {
            Ok(()) => {
                if let Err(shutdown) = shutdown_result {
                    log::warn!(
                        "Persistence worker shutdown failed before successful direct fallback: {shutdown:#}"
                    );
                }
                return Ok(());
            }
            Err(fallback) => {
                let original = save_result.expect_err("fallback requires failed worker save");
                return Err(anyhow::anyhow!(
                    "worker final save failed: {original:#}; joined direct fallback also failed: {fallback:#}"
                ));
            }
        }
    }
    match (save_result, shutdown_result) {
        (Err(save), Err(shutdown)) => Err(anyhow::anyhow!(
            "final session save failed: {save:#}; persistence worker shutdown also failed: {shutdown:#}"
        )),
        (Err(save), Ok(())) => Err(save),
        (Ok(()), Err(shutdown)) => Err(shutdown),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn persist_final_session_direct(state: &mut WaylandState) -> Result<(), anyhow::Error> {
    observe_input_dirty(state, Instant::now());
    let Some(options) = state.session_options().cloned() else {
        return Ok(());
    };
    if should_skip_disabled_final_save(&options) {
        return Ok(());
    }
    if should_skip_protected_session_save(state, &options) {
        return Ok(());
    }
    let snapshot = session::snapshot_from_input(&state.input_state, &options);
    let has_board_data = snapshot
        .as_ref()
        .is_some_and(session::SessionSnapshot::has_board_data);
    if runtime_session::should_skip_unloaded_contentless_save(
        state.session.has_loaded_board_data(),
        state.session.is_dirty(),
        state.input_state.is_session_dirty(),
        has_board_data,
        runtime_session::has_session_artifact(&options),
    ) {
        return Ok(());
    }
    let snapshot = snapshot_or_empty(state, &options, snapshot)?;
    let snapshot_board_data = snapshot.has_board_data();
    let report = session::save_snapshot_with_report_and_clear_boundary(
        &snapshot,
        &options,
        state.session.has_loaded_board_data(),
    )?;
    let Some(report) = report else {
        return Err(anyhow::anyhow!(
            "joined direct fallback produced no committed session write"
        ));
    };
    let committed_board_data =
        !matches!(report.outcome, session::SaveSnapshotOutcome::ClearedEmpty)
            && snapshot_board_data;
    log_session_save_result(SessionSaveReason::Shutdown, Some(&report), Duration::ZERO);
    state
        .session
        .mark_saved(Instant::now(), committed_board_data);
    Ok(())
}

fn persist_final_session(state: &mut WaylandState) -> Result<(), anyhow::Error> {
    let barrier_result = persistence_barrier(state);
    if let Some(err) = final_save_barrier_policy(barrier_result, state.persistence.is_healthy())? {
        log::warn!(
            "Autosave failed while preparing the final session save; retrying the current live state with the normal save strategy: {err:#}"
        );
    }
    let Some(options) = state.session_options().cloned() else {
        return Ok(());
    };
    if should_skip_disabled_final_save(&options) {
        return Ok(());
    }

    if should_skip_protected_session_save(state, &options) {
        return Ok(());
    }

    let started = Instant::now();
    log::info!(
        "Starting {} session persistence to {}",
        SessionSaveReason::Shutdown.label(),
        options.session_file_path().display()
    );
    let snapshot_started = Instant::now();
    let snapshot = session::snapshot_from_input(&state.input_state, &options);
    log_snapshot_capture(
        SessionSaveReason::Shutdown,
        &options,
        snapshot.as_ref(),
        snapshot_started.elapsed(),
    );
    if should_skip_unloaded_contentless_save(state, &options, snapshot.as_ref())? {
        return Ok(());
    }
    let snapshot = snapshot_or_empty(state, &options, snapshot)?;
    let outcome = run_persistence_operation(
        state,
        PersistenceOperation::Save {
            snapshot,
            options,
            strategy: SaveStrategy::Normal,
            contentless_clear_boundary: state.session.has_loaded_board_data(),
        },
    )?;
    let PersistenceOutcome::Save(save) = outcome else {
        return Err(anyhow::anyhow!("unexpected final-save worker outcome"));
    };
    if !save.committed() {
        return Err(anyhow::anyhow!(
            "final session save produced no committed write"
        ));
    }
    log_session_save_result(
        SessionSaveReason::Shutdown,
        save.report.as_ref(),
        started.elapsed(),
    );
    notify_session_save_report(state, save.report.as_ref());
    state
        .session
        .mark_saved(Instant::now(), save.committed_board_data);
    Ok(())
}

pub(super) fn autosave_timeout(state: &WaylandState, now: Instant) -> Option<Duration> {
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
    drain_persistence_completion(state)?;
    observe_input_dirty(state, now);

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

    if should_defer_for_interaction(state)
        && defer_pending_autosave_for_interaction(&mut state.session, now, &options)
    {
        return Ok(());
    }

    if !state.session.autosave_due(now, &options) {
        return Ok(());
    }

    let started = Instant::now();
    let snapshot_started = Instant::now();
    let snapshot = session::snapshot_from_input(&state.input_state, &options);
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
                show_session_failure_toast(state);
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

pub(in crate::backend::wayland) fn observe_input_dirty(state: &mut WaylandState, now: Instant) {
    let input_dirty = state.input_state.take_session_dirty();
    state.session.record_input_dirty(now, input_dirty);
}

pub(in crate::backend::wayland) fn persistence_barrier(
    state: &mut WaylandState,
) -> Result<(), anyhow::Error> {
    observe_input_dirty(state, Instant::now());
    if state.persistence.is_active() {
        let completion = match state.persistence.wait_for_completion() {
            Ok(Some(completion)) => completion,
            Ok(None) => {
                return Err(anyhow::anyhow!(
                    "active persistence request had no completion"
                ));
            }
            Err(err) => {
                handle_persistence_transport_failure(state, Instant::now(), &err);
                return Err(err);
            }
        };
        apply_persistence_completion(state, completion)?;
    }
    if !state.persistence.is_healthy() {
        return Err(anyhow::anyhow!("session persistence worker is unhealthy"));
    }
    Ok(())
}

pub(in crate::backend::wayland) fn run_persistence_operation(
    state: &mut WaylandState,
    operation: PersistenceOperation,
) -> Result<PersistenceOutcome, anyhow::Error> {
    persistence_barrier(state)?;
    let result = state
        .persistence
        .run(state.session.target_epoch(), operation);
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
        self.persistence.try_receive()
    }

    fn apply_persistence_completion(
        &mut self,
        completion: PersistenceCompletion,
    ) -> Result<(), anyhow::Error> {
        apply_persistence_completion(self, completion)
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

fn apply_persistence_completion(
    state: &mut WaylandState,
    completion: PersistenceCompletion,
) -> Result<(), anyhow::Error> {
    observe_input_dirty(state, Instant::now());
    let id = completion.id;
    let save_result: Result<SaveCompletion, anyhow::Error> = match completion.result {
        Ok(PersistenceOutcome::Save(save)) => Ok(save),
        Ok(other) => Err(anyhow::anyhow!(
            "unexpected asynchronous persistence outcome: {other:?}"
        )),
        Err(err) => Err(err),
    };
    let completed_at = Instant::now();
    let committed = state
        .session
        .complete_autosave(id, completed_at, &save_result)?;
    match save_result {
        Ok(save) if committed => {
            log_session_save_result(
                SessionSaveReason::Autosave,
                save.report.as_ref(),
                completion.execution_time,
            );
            notify_session_save_report(state, save.report.as_ref());
        }
        Ok(_) => {
            let err = anyhow::anyhow!("autosave worker completed without writing session data");
            handle_autosave_failure(state, completed_at, &err);
            return Err(err);
        }
        Err(err) => {
            handle_autosave_failure(state, completed_at, &err);
            return Err(err);
        }
    }
    Ok(())
}

fn handle_autosave_failure(state: &mut WaylandState, now: Instant, err: &anyhow::Error) {
    let Some(options) = state.session_options().cloned() else {
        return;
    };
    if record_autosave_failure(&mut state.session, now, &options) {
        show_session_failure_toast(state);
        notify_session_failure(state, err);
    }
}

fn handle_persistence_transport_failure(
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

pub(in crate::backend::wayland) fn should_defer_for_interaction(state: &WaylandState) -> bool {
    persistence_interaction_active(
        state.input_state.has_active_pointer_interaction(),
        state.toolbar_dragging(),
        state.is_move_dragging(),
        state.board_panning_active(),
        state.zoom_panning_active(),
        stylus_tip_down(state),
    )
}

fn persistence_interaction_active(
    pointer: bool,
    toolbar_drag: bool,
    move_drag: bool,
    board_pan: bool,
    zoom_pan: bool,
    stylus_tip: bool,
) -> bool {
    pointer || toolbar_drag || move_drag || board_pan || zoom_pan || stylus_tip
}

pub(in crate::backend::wayland) fn interaction_defer_interval() -> Duration {
    Duration::from_millis(AUTOSAVE_ACTIVE_INTERACTION_DEFER_MS)
}

fn min_optional_timeout(a: Option<Duration>, b: Option<Duration>) -> Option<Duration> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn defer_pending_autosave_for_interaction(
    session: &mut SessionState,
    now: Instant,
    options: &session::SessionOptions,
) -> bool {
    if session.autosave_timeout(now, options).is_none() {
        return false;
    }

    let delay = Duration::from_millis(AUTOSAVE_ACTIVE_INTERACTION_DEFER_MS);
    session.defer_autosave(now, delay);
    log::debug!(
        "Deferring autosave for {:?} while pointer/stylus interaction is active",
        delay
    );
    true
}

#[cfg(tablet)]
fn stylus_tip_down(state: &WaylandState) -> bool {
    state.stylus_tip_down
}

#[cfg(not(tablet))]
fn stylus_tip_down(_state: &WaylandState) -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionSaveReason {
    Autosave,
    Shutdown,
}

impl SessionSaveReason {
    fn label(self) -> &'static str {
        match self {
            Self::Autosave => "autosave",
            Self::Shutdown => "shutdown",
        }
    }
}

fn log_snapshot_capture(
    reason: SessionSaveReason,
    options: &session::SessionOptions,
    snapshot: Option<&session::SessionSnapshot>,
    elapsed: Duration,
) {
    let Some(_snapshot) = snapshot else {
        log::info!(
            "Captured {} session snapshot for {} in {:?}: no persistable data",
            reason.label(),
            options.session_file_path().display(),
            elapsed
        );
        return;
    };

    log::info!(
        "Captured {} session snapshot for {} in {:?}; diagnostics and payload preparation will run on the persistence worker",
        reason.label(),
        options.session_file_path().display(),
        elapsed
    );
}

fn log_session_save_result(
    reason: SessionSaveReason,
    report: Option<&SaveSnapshotReport>,
    elapsed: Duration,
) {
    let Some(report) = report else {
        log::info!(
            "Finished {} session persistence in {:?}: no file write needed",
            reason.label(),
            elapsed
        );
        return;
    };

    log::info!(
        "Finished {} session persistence in {:?}: outcome={:?}, written={} bytes, raw={} bytes, compression={}, path={}",
        reason.label(),
        elapsed,
        report.outcome,
        report.written_size,
        report.raw_size,
        report.compressed,
        report.path.display()
    );
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
        log::info!(
            "Skipping session save to {} because a previous oversized compressed session was left protected and no session changes have been made",
            session_path.display()
        );
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
