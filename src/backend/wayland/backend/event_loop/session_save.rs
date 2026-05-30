use super::super::super::state::WaylandState;
use crate::{
    backend::wayland::session::{self as runtime_session, SessionState},
    config::{Action, Config},
    input::state::UiToastKind,
    notification, session,
    session::{SaveSnapshotOutcome, SaveSnapshotReport},
};
use std::time::{Duration, Instant};

const SESSION_SAVE_WARNING_TOAST_MS: u64 = 20_000;
const SESSION_SAVE_NOTIFICATION_TIMEOUT_MS: i32 = 15_000;
const AUTOSAVE_ACTIVE_INTERACTION_DEFER_MS: u64 = 500;

pub(super) fn persist_session(state: &WaylandState) -> Result<(), anyhow::Error> {
    let Some(options) = state.session_options() else {
        return Ok(());
    };

    if should_skip_protected_session_save(state, options) {
        return Ok(());
    }

    let started = Instant::now();
    log::info!(
        "Starting {} session persistence to {}",
        SessionSaveReason::Shutdown.label(),
        options.session_file_path().display()
    );
    let snapshot_started = Instant::now();
    let snapshot = session::snapshot_from_input(&state.input_state, options);
    log_snapshot_capture(
        SessionSaveReason::Shutdown,
        options,
        snapshot.as_ref(),
        snapshot_started.elapsed(),
    );
    if should_skip_unloaded_contentless_save(state, options, snapshot.as_ref()) {
        return Ok(());
    }
    let save = save_snapshot_or_clear(state, options, snapshot, SessionSaveReason::Shutdown)?;
    log_session_save_result(
        SessionSaveReason::Shutdown,
        save.report.as_ref(),
        started.elapsed(),
    );
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

    if should_defer_autosave_for_interaction(state)
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

    match save_snapshot_or_clear(state, &options, snapshot, SessionSaveReason::Autosave) {
        Ok(save) => {
            log_session_save_result(
                SessionSaveReason::Autosave,
                save.report.as_ref(),
                started.elapsed(),
            );
            notify_session_save_report(state, save.report.as_ref());
            record_autosave_success(
                &mut state.session,
                now,
                save.report.is_some(),
                save.saved_board_data,
            );
        }
        Err(err) => {
            if record_autosave_failure(&mut state.session, now, &options) {
                show_session_failure_toast(state);
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
    reason: SessionSaveReason,
) -> Result<SessionSaveAttempt, anyhow::Error> {
    if should_skip_protected_session_save(state, options) {
        return Ok(SessionSaveAttempt::skipped());
    }

    if should_skip_unloaded_contentless_save(state, options, snapshot.as_ref()) {
        return Ok(SessionSaveAttempt::skipped());
    }

    if let Some(snapshot) = snapshot {
        let saved_board_data = snapshot.has_board_data();
        let report = save_snapshot_with_reason(
            &snapshot,
            options,
            reason,
            state.session.has_loaded_board_data(),
        )?;
        return Ok(SessionSaveAttempt {
            saved_board_data: report.is_some() && saved_board_data,
            report,
        });
    }

    if !persistence_enabled(options) {
        return Ok(SessionSaveAttempt::skipped());
    }

    let empty_snapshot = session::SessionSnapshot {
        active_board_id: state.input_state.board_id().to_string(),
        boards: Vec::new(),
        tool_state: None,
    };
    let report = save_snapshot_with_reason(
        &empty_snapshot,
        options,
        reason,
        state.session.has_loaded_board_data(),
    )?;
    Ok(SessionSaveAttempt {
        report,
        saved_board_data: false,
    })
}

#[derive(Debug)]
struct SessionSaveAttempt {
    report: Option<SaveSnapshotReport>,
    saved_board_data: bool,
}

impl SessionSaveAttempt {
    fn skipped() -> Self {
        Self {
            report: None,
            saved_board_data: false,
        }
    }
}

fn save_snapshot_with_reason(
    snapshot: &session::SessionSnapshot,
    options: &session::SessionOptions,
    reason: SessionSaveReason,
    contentless_clear_boundary: bool,
) -> Result<Option<SaveSnapshotReport>, anyhow::Error> {
    match reason {
        SessionSaveReason::Autosave => {
            session::save_snapshot_autosave_with_report_and_clear_boundary(
                snapshot,
                options,
                contentless_clear_boundary,
            )
        }
        SessionSaveReason::Shutdown => session::save_snapshot_with_report_and_clear_boundary(
            snapshot,
            options,
            contentless_clear_boundary,
        ),
    }
}

fn should_defer_autosave_for_interaction(state: &WaylandState) -> bool {
    state.input_state.has_active_pointer_interaction()
        || state.toolbar_dragging()
        || state.is_move_dragging()
        || state.board_panning_active()
        || state.zoom_panning_active()
        || stylus_tip_down(state)
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

#[derive(Debug, Default)]
struct SnapshotSummary {
    boards: usize,
    pages: usize,
    shapes: usize,
    undo_entries: usize,
    redo_entries: usize,
    visible_image_shapes: usize,
    visible_image_bytes: usize,
    max_history_depth: usize,
    tool_state: bool,
}

impl SnapshotSummary {
    fn from_snapshot(snapshot: &session::SessionSnapshot) -> Self {
        let mut summary = Self {
            tool_state: snapshot.tool_state.is_some(),
            ..Self::default()
        };
        for board in &snapshot.boards {
            summary.boards += 1;
            summary.pages += board.pages.pages.len();
            for frame in &board.pages.pages {
                let undo = frame.undo_stack_len();
                let redo = frame.redo_stack_len();
                summary.shapes += frame.shapes.len();
                summary.undo_entries += undo;
                summary.redo_entries += redo;
                summary.max_history_depth = summary.max_history_depth.max(undo.max(redo));
                for drawn in &frame.shapes {
                    if let crate::draw::Shape::Image { data, .. } = &drawn.shape {
                        summary.visible_image_shapes += 1;
                        summary.visible_image_bytes =
                            summary.visible_image_bytes.saturating_add(data.bytes.len());
                    }
                }
            }
        }
        summary
    }
}

fn log_snapshot_capture(
    reason: SessionSaveReason,
    options: &session::SessionOptions,
    snapshot: Option<&session::SessionSnapshot>,
    elapsed: Duration,
) {
    let Some(snapshot) = snapshot else {
        log::info!(
            "Captured {} session snapshot for {} in {:?}: no persistable data",
            reason.label(),
            options.session_file_path().display(),
            elapsed
        );
        return;
    };

    let summary = SnapshotSummary::from_snapshot(snapshot);
    log::info!(
        "Captured {} session snapshot for {} in {:?}: boards={}, pages={}, shapes={}, undo_entries={}, redo_entries={}, max_history_depth={}, visible_images={} ({} bytes), tool_state={}",
        reason.label(),
        options.session_file_path().display(),
        elapsed,
        summary.boards,
        summary.pages,
        summary.shapes,
        summary.undo_entries,
        summary.redo_entries,
        summary.max_history_depth,
        summary.visible_image_shapes,
        summary.visible_image_bytes,
        summary.tool_state
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
    state: &WaylandState,
    options: &session::SessionOptions,
    snapshot: Option<&session::SessionSnapshot>,
) -> bool {
    let skip = runtime_session::should_skip_unloaded_contentless_save(
        state.session.has_loaded_board_data(),
        state.session.is_dirty(),
        state.input_state.is_session_dirty(),
        snapshot.is_some_and(session::SessionSnapshot::has_board_data),
        runtime_session::has_session_artifact(options),
    );
    if skip {
        log::warn!(
            "Skipping session save to {} because no session was loaded, no session changes were recorded, and the current snapshot has no board data",
            options.session_file_path().display()
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
        let toast = session_save_toast_text(notification, report);
        state.input_state.set_ui_toast_with_action_and_duration(
            UiToastKind::Warning,
            toast,
            "Settings",
            Action::OpenConfigurator,
            SESSION_SAVE_WARNING_TOAST_MS,
        );
        notification::send_notification_with_timeout_async(
            &state.tokio_handle,
            summary,
            body,
            Some("dialog-warning".to_string()),
            SESSION_SAVE_NOTIFICATION_TIMEOUT_MS,
        );
    }
}

fn session_save_toast_text(
    notification: SessionSaveNotification,
    report: &SaveSnapshotReport,
) -> String {
    let written = format_bytes(report.written_size as u64);
    let limit = format_bytes(report.max_file_size_bytes);
    let suggested_limit_mb =
        suggested_limit_mb(report.written_size as u64, report.max_file_size_bytes);
    match notification {
        SessionSaveNotification::NearLimit => {
            format!("Session near limit: {written}/{limit}. Set {suggested_limit_mb} MiB.")
        }
        SessionSaveNotification::TrimmedHistory { depth } => {
            format!("Session saved. Undo history trimmed to {depth} entries.")
        }
        SessionSaveNotification::VisibleOnly => "Session saved without undo history.".to_string(),
    }
}

fn session_save_notification_text(
    notification: SessionSaveNotification,
    report: &SaveSnapshotReport,
) -> (String, String) {
    let written = format_bytes(report.written_size as u64);
    let limit = format_bytes(report.max_file_size_bytes);
    let suggested_limit_mb =
        suggested_limit_mb(report.written_size as u64, report.max_file_size_bytes);
    match notification {
        SessionSaveNotification::NearLimit => (
            "Session Storage Nearly Full".to_string(),
            format!(
                "Session save is using {written} of {limit}. Open Settings > Session > Max file size and set {suggested_limit_mb} MiB, or edit {}.",
                config_path_display()
            ),
        ),
        SessionSaveNotification::TrimmedHistory { depth } => (
            "Session Undo History Trimmed".to_string(),
            format!(
                "Drawings were saved, but undo/redo history was trimmed to {depth} entries per stack to meet save and restore safety limits."
            ),
        ),
        SessionSaveNotification::VisibleOnly => (
            "Session Undo History Not Saved".to_string(),
            "Drawings were saved, but undo/redo history was omitted to meet save and restore safety limits.".to_string(),
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

fn record_autosave_success(
    session_state: &mut SessionState,
    now: Instant,
    saved: bool,
    saved_board_data: bool,
) {
    if saved {
        session_state.mark_saved(now, saved_board_data);
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
    notification::send_notification_with_timeout_async(
        &state.tokio_handle,
        "Failed to Save Session".to_string(),
        format!(
            "Drawings may not persist. Raise Session > Max file size, remove images, or disable persisted history. Details: {}",
            err
        ),
        Some("dialog-error".to_string()),
        SESSION_SAVE_NOTIFICATION_TIMEOUT_MS,
    );
}

fn show_session_failure_toast(state: &mut WaylandState) {
    state.input_state.set_ui_toast_with_action_and_duration(
        UiToastKind::Warning,
        "Session save failed; drawings may not restore. Check max_file_size_mb.",
        "Settings",
        Action::OpenConfigurator,
        SESSION_SAVE_WARNING_TOAST_MS,
    );
}

fn suggested_limit_mb(projected_written_size: u64, current_limit_bytes: u64) -> u64 {
    const MIB: u64 = 1024 * 1024;
    let projected_mb = projected_written_size.div_ceil(MIB);
    let current_mb = current_limit_bytes.div_ceil(MIB);
    projected_mb
        .saturating_mul(3)
        .div_ceil(2)
        .max(current_mb.saturating_mul(3).div_ceil(2))
        .clamp(1, 1024)
}

fn config_path_display() -> String {
    Config::get_config_path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "~/.config/wayscriber/config.toml".to_string())
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
}
