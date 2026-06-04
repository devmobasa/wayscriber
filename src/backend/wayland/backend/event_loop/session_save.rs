use super::super::super::state::WaylandState;
use crate::{
    backend::wayland::session::{self as runtime_session, SessionState},
    session,
    session::SaveSnapshotReport,
};
use std::time::{Duration, Instant};

const AUTOSAVE_ACTIVE_INTERACTION_DEFER_MS: u64 = 500;

mod notifications;

pub(super) use notifications::notify_session_failure;
#[cfg(test)]
use notifications::{
    SessionSaveNotification, pending_save_notifications, session_save_notification_text,
};
use notifications::{
    notify_session_save_report, record_autosave_failure, record_autosave_success,
    show_session_failure_toast,
};

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

#[cfg(test)]
mod tests;
