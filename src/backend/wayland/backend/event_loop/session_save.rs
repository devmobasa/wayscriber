use super::super::super::state::WaylandState;
use crate::{
    backend::wayland::session::SessionState,
    notification, session,
    session::{SaveSnapshotOutcome, SaveSnapshotReport},
};
use std::time::{Duration, Instant};

pub(super) fn persist_session(state: &WaylandState) -> Result<(), anyhow::Error> {
    let Some(options) = state.session_options() else {
        return Ok(());
    };

    if should_skip_protected_session_save(state, options) {
        return Ok(());
    }

    let snapshot = session::snapshot_from_input(&state.input_state, options);
    let _ = save_snapshot_or_clear(state, options, snapshot)?;
    Ok(())
}

pub(super) fn autosave_timeout(state: &WaylandState, now: Instant) -> Option<Duration> {
    let options = state.session_options()?;
    state.session.autosave_timeout(now, options)
}

pub(super) fn autosave_if_due(state: &mut WaylandState, now: Instant) -> Result<(), anyhow::Error> {
    let Some(options) = state.session_options().cloned() else {
        return Ok(());
    };

    let input_dirty = state.input_state.take_session_dirty();
    state.session.record_input_dirty(now, input_dirty);

    if !state.session.autosave_due(now, &options) {
        return Ok(());
    }

    match save_snapshot_or_clear(
        state,
        &options,
        session::snapshot_from_input(&state.input_state, &options),
    ) {
        Ok(report) => {
            notify_session_save_report(state, report.as_ref());
            record_autosave_success(&mut state.session, now, report.is_some());
        }
        Err(err) => {
            if record_autosave_failure(&mut state.session, now, &options) {
                notify_session_failure(state, &err);
            }
            return Err(err);
        }
    }
    Ok(())
}

fn save_snapshot_or_clear(
    state: &WaylandState,
    options: &session::SessionOptions,
    snapshot: Option<session::SessionSnapshot>,
) -> Result<Option<SaveSnapshotReport>, anyhow::Error> {
    if should_skip_protected_session_save(state, options) {
        return Ok(None);
    }

    if let Some(snapshot) = snapshot {
        return session::save_snapshot_with_report(&snapshot, options);
    }

    if !persistence_enabled(options) {
        return Ok(None);
    }

    let empty_snapshot = session::SessionSnapshot {
        active_board_id: state.input_state.board_id().to_string(),
        boards: Vec::new(),
        tool_state: None,
    };
    session::save_snapshot_with_report(&empty_snapshot, options)
}

fn persistence_enabled(options: &session::SessionOptions) -> bool {
    options.any_enabled() || options.restore_tool_state || options.persist_history
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionSaveNotification {
    NearLimit,
    TrimmedHistory { depth: usize },
    VisibleOnly,
}

fn pending_save_notifications(
    session_state: &mut SessionState,
    report: &SaveSnapshotReport,
) -> Vec<SessionSaveNotification> {
    let mut notifications = Vec::new();
    match report.outcome {
        SaveSnapshotOutcome::Full => {
            if report.is_near_limit() && session_state.mark_near_limit_notified(&report.path) {
                notifications.push(SessionSaveNotification::NearLimit);
            }
        }
        SaveSnapshotOutcome::TrimmedHistory { depth } => {
            if session_state.mark_trimmed_history_notified() {
                notifications.push(SessionSaveNotification::TrimmedHistory { depth });
            }
        }
        SaveSnapshotOutcome::VisibleOnly => {
            if session_state.mark_visible_only_notified() {
                notifications.push(SessionSaveNotification::VisibleOnly);
            }
        }
        SaveSnapshotOutcome::ClearedEmpty => {}
    }
    notifications
}

fn notify_session_save_report(state: &mut WaylandState, report: Option<&SaveSnapshotReport>) {
    let Some(report) = report else {
        return;
    };

    for notification in pending_save_notifications(&mut state.session, report) {
        let (summary, body) = session_save_notification_text(notification, report);
        notification::send_notification_async(
            &state.tokio_handle,
            summary,
            body,
            Some("dialog-warning".to_string()),
        );
    }
}

fn session_save_notification_text(
    notification: SessionSaveNotification,
    report: &SaveSnapshotReport,
) -> (String, String) {
    let written = format_bytes(report.written_size as u64);
    let limit = format_bytes(report.max_file_size_bytes);
    match notification {
        SessionSaveNotification::NearLimit => (
            "Session Storage Nearly Full".to_string(),
            format!(
                "Session save is using {written} of the {limit} cap. Image-heavy sessions can grow quickly; consider disabling persisted history or increasing session.max_file_size_mb."
            ),
        ),
        SessionSaveNotification::TrimmedHistory { depth } => (
            "Session Undo History Trimmed".to_string(),
            format!(
                "Your drawings were saved, but undo/redo history was trimmed to {depth} entries per stack to keep the session within save and restore safety limits."
            ),
        ),
        SessionSaveNotification::VisibleOnly => (
            "Session Undo History Not Saved".to_string(),
            "Your drawings were saved, but undo/redo history was omitted to keep the session within save and restore safety limits.".to_string(),
        ),
    }
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} bytes")
    }
}

fn record_autosave_success(session_state: &mut SessionState, now: Instant, saved: bool) {
    if saved {
        session_state.mark_saved(now);
    }
}

fn record_autosave_failure(
    session_state: &mut SessionState,
    now: Instant,
    options: &session::SessionOptions,
) -> bool {
    session_state.mark_autosave_failure(now, options.autosave_failure_backoff)
}

pub(super) fn notify_session_failure(state: &WaylandState, err: &anyhow::Error) {
    notification::send_notification_async(
        &state.tokio_handle,
        "Failed to Save Session".to_string(),
        format!("Your drawings may not persist: {}", err),
        Some("dialog-error".to_string()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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

        record_autosave_success(&mut state, now, true);
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

        record_autosave_success(&mut state, now + Duration::from_millis(2), true);
        assert!(!state.autosave_due(now + Duration::from_millis(2), &options));
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
}
