use super::*;
use crate::backend::wayland::session::{PersistenceController, PersistenceOperation};
use crate::session::SaveSnapshotOutcome;
use std::path::PathBuf;

#[test]
fn final_save_retries_after_failed_autosave_while_worker_is_healthy() {
    let error = anyhow::anyhow!("autosave failed");
    let recovered = final_save_barrier_policy(Err(error), true)
        .expect("healthy worker should remain available for the final save");
    assert!(recovered.is_some());
}

#[test]
fn final_save_does_not_retry_through_an_unhealthy_worker() {
    let error = anyhow::anyhow!("worker disconnected");
    let propagated = final_save_barrier_policy(Err(error), false)
        .expect_err("unhealthy worker must enter shutdown fallback handling");
    assert!(propagated.to_string().contains("worker disconnected"));
}

#[test]
fn pointer_and_stylus_both_gate_persistence_transitions() {
    assert!(persistence_interaction_active(
        true, false, false, false, false, false
    ));
    assert!(persistence_interaction_active(
        false, false, false, false, false, true
    ));
    assert!(!persistence_interaction_active(
        false, false, false, false, false, false
    ));
}

#[test]
fn unhealthy_worker_removes_dirty_session_from_automatic_schedule() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display-1");
    options.persist_transparent = true;
    options.autosave_enabled = true;
    options.autosave_idle = Duration::from_millis(1);
    options.autosave_interval = Duration::from_millis(1);
    let now = Instant::now();
    let mut state = SessionState::new(Some(options.clone()));
    state.record_input_dirty(now, true);
    let due = now + Duration::from_millis(2);

    assert_eq!(
        scheduled_autosave_timeout(&state, Some(&options), false, due),
        None
    );
    assert_eq!(
        scheduled_autosave_timeout(&state, Some(&options), true, due),
        Some(Duration::ZERO)
    );
}

#[test]
fn accepted_autosave_worker_panic_restores_dirty_and_requests_one_notification() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "disconnect-test");
    options.persist_transparent = true;
    options.autosave_enabled = true;
    options.autosave_idle = Duration::from_millis(1);
    options.autosave_interval = Duration::from_millis(1);
    options.autosave_failure_backoff = Duration::from_millis(50);
    let started = Instant::now();
    let mut session = SessionState::new(Some(options.clone()));
    session.record_input_dirty(started, true);
    let dirty_window = session.prepare_autosave_submission().unwrap();
    let mut controller = PersistenceController::start().unwrap();
    let request_id = controller
        .try_submit(0, PersistenceOperation::PanicForTest)
        .unwrap();
    session.commit_autosave_submission(request_id, dirty_window);

    let err = controller
        .wait_for_completion()
        .expect_err("worker panic must disconnect the completion channel");
    assert!(err.to_string().contains("disconnected"));
    assert!(!controller.is_healthy());

    let failure_at = Instant::now();
    let notification_requests = [
        record_persistence_transport_failure(&mut session, Some(&options), failure_at),
        record_persistence_transport_failure(&mut session, Some(&options), failure_at),
    ]
    .into_iter()
    .filter(|requested| *requested)
    .count();
    assert_eq!(notification_requests, 1);
    assert!(session.is_dirty());
    assert_eq!(
        scheduled_autosave_timeout(&session, Some(&options), false, failure_at),
        None
    );

    assert!(controller.shutdown(0).is_err());
    assert!(controller.is_stopped());
}

#[test]
fn synchronous_worker_loss_without_autosave_ticket_requests_one_notification() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "sync-disconnect");
    options.persist_transparent = true;
    let mut session = SessionState::new(Some(options.clone()));
    let failure_at = Instant::now();

    assert!(record_persistence_transport_failure(
        &mut session,
        Some(&options),
        failure_at,
    ));
    assert!(!record_persistence_transport_failure(
        &mut session,
        Some(&options),
        failure_at,
    ));
    assert!(!session.is_dirty());
}

#[test]
fn persistence_enabled_respects_any_enabled_boards() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display-1");
    options.persist_transparent = true;
    options.persist_history = false;
    options.restore_tool_state = false;

    assert!(persistence_enabled(&options));
}

#[test]
fn persistence_enabled_respects_restore_tool_state() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display-1");
    options.persist_transparent = false;
    options.persist_whiteboard = false;
    options.persist_blackboard = false;
    options.persist_history = false;
    options.restore_tool_state = true;

    assert!(persistence_enabled(&options));
}

#[test]
fn persistence_enabled_is_false_when_nothing_is_enabled() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display-1");
    options.persist_transparent = false;
    options.persist_whiteboard = false;
    options.persist_blackboard = false;
    options.persist_history = false;
    options.restore_tool_state = false;

    assert!(!persistence_enabled(&options));
}

#[test]
fn final_save_skips_fully_disabled_persistence() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display-1");
    options.persist_transparent = false;
    options.persist_whiteboard = false;
    options.persist_blackboard = false;
    options.persist_history = false;
    options.restore_tool_state = false;

    assert!(should_skip_disabled_final_save(&options));
}

#[test]
fn record_autosave_failure_notifies_only_once_until_saved() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display-1");
    options.persist_transparent = true;
    options.autosave_enabled = true;
    options.autosave_idle = Duration::from_millis(1);
    options.autosave_interval = Duration::from_millis(1);
    options.autosave_failure_backoff = Duration::from_millis(50);

    let mut state = crate::backend::wayland::session::SessionState::new(Some(options.clone()));
    let now = Instant::now();
    state.record_input_dirty(now, true);

    assert!(record_autosave_failure(&mut state, now, &options));
    assert!(!record_autosave_failure(&mut state, now, &options));
    assert!(!state.autosave_due(now, &options));

    record_autosave_success(&mut state, now, true, false);
    state.record_input_dirty(now, true);
    assert!(record_autosave_failure(&mut state, now, &options));
}

#[test]
fn record_autosave_success_clears_dirty_state_when_saved() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display-1");
    options.persist_transparent = true;
    options.autosave_enabled = true;
    options.autosave_idle = Duration::from_millis(1);
    options.autosave_interval = Duration::from_millis(1);

    let mut state = crate::backend::wayland::session::SessionState::new(Some(options.clone()));
    let now = Instant::now();
    state.record_input_dirty(now, true);
    assert!(state.autosave_due(now + Duration::from_millis(2), &options));

    record_autosave_success(&mut state, now + Duration::from_millis(2), true, false);
    assert!(!state.autosave_due(now + Duration::from_millis(2), &options));
}

#[test]
fn record_autosave_success_tracks_saved_board_data_for_future_clear_boundaries() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display-1");
    options.persist_transparent = true;
    options.autosave_enabled = true;

    let mut state = SessionState::new(Some(options));
    assert!(!state.has_loaded_board_data());

    record_autosave_success(&mut state, Instant::now(), true, true);

    assert!(
        state.has_loaded_board_data(),
        "board data saved during this run must count when the next contentless save decides whether it is a clear"
    );
}

#[test]
fn record_autosave_success_without_saved_report_keeps_dirty_state() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display-1");
    options.persist_transparent = true;
    options.autosave_enabled = true;
    options.autosave_idle = Duration::from_millis(1);
    options.autosave_interval = Duration::from_millis(1);

    let mut state = SessionState::new(Some(options.clone()));
    let now = Instant::now();
    state.record_input_dirty(now, true);
    let due_at = now + Duration::from_millis(2);
    assert!(state.autosave_due(due_at, &options));

    record_autosave_success(&mut state, due_at, false, false);

    assert!(state.autosave_due(due_at, &options));
}

#[test]
fn autosave_failure_after_deferral_respects_backoff() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display-1");
    options.persist_transparent = true;
    options.autosave_enabled = true;
    options.autosave_idle = Duration::from_millis(1);
    options.autosave_interval = Duration::from_millis(1);
    options.autosave_failure_backoff = Duration::from_millis(75);

    let mut state = SessionState::new(Some(options.clone()));
    let now = Instant::now();
    state.record_input_dirty(now, true);
    let due_at = now + Duration::from_millis(2);
    assert!(defer_pending_autosave_for_interaction(
        &mut state, due_at, &options
    ));

    let after_deferral = due_at + Duration::from_millis(AUTOSAVE_ACTIVE_INTERACTION_DEFER_MS);
    assert!(state.autosave_due(after_deferral, &options));

    assert!(record_autosave_failure(
        &mut state,
        after_deferral,
        &options
    ));

    assert!(!state.autosave_due(after_deferral, &options));
    assert_eq!(
        state.autosave_timeout(after_deferral, &options),
        Some(options.autosave_failure_backoff)
    );
}

#[test]
fn interaction_deferral_refreshes_existing_autosave_deferral() {
    let mut options = session::SessionOptions::new(PathBuf::from("/tmp"), "display-1");
    options.persist_transparent = true;
    options.autosave_enabled = true;
    options.autosave_idle = Duration::from_millis(1);
    options.autosave_interval = Duration::from_millis(1);

    let mut state = SessionState::new(Some(options.clone()));
    let now = Instant::now();
    state.record_input_dirty(now, true);
    let due_at = now + Duration::from_millis(2);
    assert!(state.autosave_due(due_at, &options));

    let defer_for = Duration::from_millis(AUTOSAVE_ACTIVE_INTERACTION_DEFER_MS);
    assert!(defer_pending_autosave_for_interaction(
        &mut state, due_at, &options
    ));

    let first_deferred_until = due_at + defer_for;
    let later_interaction = first_deferred_until - Duration::from_millis(100);
    assert_eq!(
        state.autosave_timeout(later_interaction, &options),
        Some(Duration::from_millis(100))
    );

    assert!(defer_pending_autosave_for_interaction(
        &mut state,
        later_interaction,
        &options
    ));

    assert_eq!(
        state.autosave_timeout(later_interaction, &options),
        Some(defer_for)
    );
    assert!(
        !state.autosave_due(first_deferred_until, &options),
        "autosave should stay deferred after activity inside the original quiet window"
    );
    assert!(state.autosave_due(later_interaction + defer_for, &options));
}

#[test]
fn pending_save_notifications_warns_near_limit_once_per_path() {
    let mut state = SessionState::new(None);
    let path = PathBuf::from("/tmp/session-a.json");
    let report = save_report(path.clone(), SaveSnapshotOutcome::Full, 90, 100);

    assert_eq!(
        pending_save_notifications(&mut state, &report),
        vec![SessionSaveNotification::NearLimit]
    );
    assert!(
        pending_save_notifications(&mut state, &report).is_empty(),
        "same near-limit session path should not notify twice"
    );

    let other_report = save_report(
        PathBuf::from("/tmp/session-b.json"),
        SaveSnapshotOutcome::Full,
        90,
        100,
    );
    assert_eq!(
        pending_save_notifications(&mut state, &other_report),
        vec![SessionSaveNotification::NearLimit]
    );

    let below_limit = save_report(path, SaveSnapshotOutcome::Full, 89, 100);
    assert!(pending_save_notifications(&mut state, &below_limit).is_empty());
}

#[test]
fn pending_save_notifications_reports_trimmed_history_once() {
    let mut state = SessionState::new(None);
    let report = save_report(
        PathBuf::from("/tmp/session.json"),
        SaveSnapshotOutcome::TrimmedHistory { depth: 2 },
        50,
        100,
    );

    assert_eq!(
        pending_save_notifications(&mut state, &report),
        vec![SessionSaveNotification::TrimmedHistory { depth: 2 }]
    );
    assert!(
        pending_save_notifications(&mut state, &report).is_empty(),
        "trimmed-history notification should be once per run"
    );
}

#[test]
fn pending_save_notifications_reports_visible_only_once() {
    let mut state = SessionState::new(None);
    let report = save_report(
        PathBuf::from("/tmp/session.json"),
        SaveSnapshotOutcome::VisibleOnly,
        50,
        100,
    );

    assert_eq!(
        pending_save_notifications(&mut state, &report),
        vec![SessionSaveNotification::VisibleOnly]
    );
    assert!(
        pending_save_notifications(&mut state, &report).is_empty(),
        "visible-only notification should be once per run"
    );
}

#[test]
fn pending_save_notifications_ignores_full_save_below_limit_and_empty_clear() {
    let mut state = SessionState::new(None);
    let full = save_report(
        PathBuf::from("/tmp/session.json"),
        SaveSnapshotOutcome::Full,
        10,
        100,
    );
    let cleared = save_report(
        PathBuf::from("/tmp/session.json"),
        SaveSnapshotOutcome::ClearedEmpty,
        0,
        100,
    );

    assert!(pending_save_notifications(&mut state, &full).is_empty());
    assert!(pending_save_notifications(&mut state, &cleared).is_empty());
}

#[test]
fn history_fallback_notifications_do_not_blame_only_file_size_cap() {
    let report = save_report(
        PathBuf::from("/tmp/session.json"),
        SaveSnapshotOutcome::TrimmedHistory { depth: 2 },
        50,
        100,
    );
    let (_, trimmed_body) = session_save_notification_text(
        SessionSaveNotification::TrimmedHistory { depth: 2 },
        &report,
    );
    assert!(trimmed_body.contains("save and restore safety limits"));
    assert!(!trimmed_body.contains("session.max_file_size_mb"));

    let report = save_report(
        PathBuf::from("/tmp/session.json"),
        SaveSnapshotOutcome::VisibleOnly,
        50,
        100,
    );
    let (_, visible_body) =
        session_save_notification_text(SessionSaveNotification::VisibleOnly, &report);
    assert!(visible_body.contains("save and restore safety limits"));
    assert!(!visible_body.contains("session.max_file_size_mb"));
}

fn save_report(
    path: PathBuf,
    outcome: SaveSnapshotOutcome,
    written_size: usize,
    max_file_size_bytes: u64,
) -> SaveSnapshotReport {
    SaveSnapshotReport {
        path,
        outcome,
        raw_size: written_size,
        written_size,
        max_file_size_bytes,
        compressed: false,
    }
}
