use super::*;
use crate::input::state::{Toast, ToastPriority};
use crate::{
    config::{Action, Config},
    notification,
    session::SaveSnapshotOutcome,
};

const SESSION_SAVE_NOTIFICATION_TIMEOUT_MS: i32 = 15_000;
const SESSION_SAVE_WARNING_TOAST_MS: u64 = 20_000;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SessionSaveNotification {
    NearLimit,
    TrimmedHistory { depth: usize },
    VisibleOnly,
}

pub(super) fn pending_save_notifications(
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

pub(super) fn notify_session_save_report(
    state: &mut WaylandState,
    report: Option<&SaveSnapshotReport>,
) {
    let Some(report) = report else {
        return;
    };

    for notification in pending_save_notifications(&mut state.session, report) {
        let (summary, body) = session_save_notification_text(notification, report);
        let toast = session_save_toast_text(notification, report);
        state.input_state.push_toast(
            ToastPriority::Action,
            "session.save",
            Toast::warning(toast)
                .action("Settings", Action::OpenConfigurator)
                .duration_ms(SESSION_SAVE_WARNING_TOAST_MS),
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

pub(super) fn session_save_notification_text(
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

#[cfg(test)]
pub(super) fn record_autosave_success(
    session_state: &mut SessionState,
    now: Instant,
    saved: bool,
    saved_board_data: bool,
) {
    if saved {
        session_state.mark_saved(now, saved_board_data);
    }
}

pub(in crate::backend::wayland) fn record_autosave_failure(
    session_state: &mut SessionState,
    now: Instant,
    options: &session::SessionOptions,
) -> bool {
    session_state.mark_autosave_failure(now, options.autosave_failure_backoff)
}

pub(in crate::backend::wayland::backend::event_loop) fn notify_session_failure(
    state: &WaylandState,
    err: &anyhow::Error,
) {
    notification::send_notification_with_timeout_async(
        &state.tokio_handle,
        "Failed to Save Session".to_string(),
        session_failure_text(err, true),
        Some("dialog-error".to_string()),
        SESSION_SAVE_NOTIFICATION_TIMEOUT_MS,
    );
}

pub(super) fn notify_persistence_worker_failure(state: &WaylandState, err: &anyhow::Error) {
    notification::send_notification_with_timeout_async(
        &state.tokio_handle,
        "Session Persistence Stopped".to_string(),
        format!(
            "Automatic session persistence stopped because its background worker failed. Wayscriber will retry the final save during shutdown. Details: {err}"
        ),
        Some("dialog-error".to_string()),
        SESSION_SAVE_NOTIFICATION_TIMEOUT_MS,
    );
}

pub(super) fn show_session_failure_toast(state: &mut WaylandState, error: &anyhow::Error) {
    state.input_state.push_toast(
        ToastPriority::Action,
        "session.save",
        Toast::warning(session_failure_text(error, false))
            .action("Settings", Action::OpenConfigurator)
            .duration_ms(SESSION_SAVE_WARNING_TOAST_MS),
    );
}

pub(super) fn show_persistence_worker_failure_toast(state: &mut WaylandState) {
    state.input_state.push_toast(
        ToastPriority::Action,
        "session.save",
        Toast::warning(
            "Automatic session persistence stopped; final save will be retried on exit.",
        )
        .action("Settings", Action::OpenConfigurator)
        .duration_ms(SESSION_SAVE_WARNING_TOAST_MS),
    );
}

/// Advice follows the typed storage error and whether shutdown wrote recovery.
pub(super) fn session_failure_text(error: &anyhow::Error, shutdown: bool) -> String {
    if let Some(saved) = error.downcast_ref::<super::final_save::RefusedDrawingsSaved>() {
        let kept = match &saved.recovery {
            Some(recovery) => format!(
                "were saved to {} instead, in its recovery file {} because they exceed the session size limit",
                saved.saved_to.display(),
                recovery.display()
            ),
            None => format!("were saved to {} instead", saved.saved_to.display()),
        };
        return format!(
            "New drawings were not written to {} and {kept}. Open that file to continue them.",
            saved.session.display()
        );
    }
    if let Some(failure) = error.downcast_ref::<session::SavePayloadTooLarge>() {
        let advice = match failure.limit {
            session::SaveLimitExceeded::WrittenSize { .. } => {
                "Raise Settings > Session > Max file size, or remove images."
            }
            session::SaveLimitExceeded::ExpandedSize { .. } => {
                "Reduce images or undo history to fit the expanded session safety limit."
            }
        };
        let saved = failure
            .recovery_path
            .as_ref()
            .map(|path| {
                format!(
                    "Drawings were preserved in the recovery file {}. ",
                    path.display()
                )
            })
            .unwrap_or_default();
        return format!("{saved}Session exceeds its size limit. {advice} Details: {error}");
    }
    let advice = if shutdown {
        "Check session files, disk space and permissions."
    } else {
        "Check session files, disk space and permissions, or use Save As to keep the current drawings."
    };
    format!("Session save failed. {advice} Details: {error}")
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

    #[test]
    fn real_size_failures_name_the_limit_and_durable_recovery_without_save_as_advice() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut options = session::SessionOptions::new(temp.path().to_path_buf(), "size-notice");
        options.persist_transparent = true;
        options.max_file_size_bytes = 1;
        let measurer = crate::draw::TextMeasurer::default();
        let mut input = crate::input::InputState::from_config(&Config::default());
        input.on_mouse_press(crate::input::MouseButton::Left, 300, 300);
        input.on_mouse_motion(400, 400);
        input.on_mouse_release(crate::input::MouseButton::Left, 400, 400);
        let snapshot = input
            .snapshot_for_persistence_with(&measurer, &options)
            .unwrap();
        let autosave =
            session::save_snapshot_autosave_with_report(&snapshot, &options).unwrap_err();
        let notice = session_failure_text(&autosave, false);
        assert!(notice.contains("Max file size"));
        assert!(!notice.contains("Save As"));
        let shutdown = session::save_snapshot_with_report(&snapshot, &options).unwrap_err();
        assert!(options.recovery_file_path().is_file());
        let notice = session_failure_text(&shutdown, true);
        assert!(notice.contains("preserved in the recovery file"));
        assert!(!notice.contains("may not persist"));
        assert!(!notice.contains("Save As"));
        let expanded: anyhow::Error = session::SavePayloadTooLarge {
            limit: session::SaveLimitExceeded::ExpandedSize {
                raw_size: 200,
                max_expanded_size: 100,
            },
            recovery_path: None,
            raw_size: 200,
            written_size: 10,
            compressed: true,
        }
        .into();
        let notice = session_failure_text(&expanded, false);
        assert!(notice.contains("Reduce images or undo history"));
        assert!(!notice.contains("Max file size"));
        assert!(!notice.contains("Save As"));
        let generic = anyhow::anyhow!("disk full");
        assert!(session_failure_text(&generic, false).contains("Save As"));
        assert!(!session_failure_text(&generic, true).contains("Save As"));
    }
}
