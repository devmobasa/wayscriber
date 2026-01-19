use super::super::super::state::WaylandState;
use crate::{notification, session};
use std::time::{Duration, Instant};

pub(super) fn persist_session(state: &WaylandState) -> Result<(), anyhow::Error> {
    let Some(options) = state.session_options() else {
        return Ok(());
    };

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
        Ok(saved) => {
            if saved {
                state.session.mark_saved(now);
            }
        }
        Err(err) => {
            if state.session.mark_autosave_failure() {
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
) -> Result<bool, anyhow::Error> {
    if let Some(snapshot) = snapshot {
        session::save_snapshot(&snapshot, options)?;
        return Ok(true);
    }

    if !persistence_enabled(options) {
        return Ok(false);
    }

    let empty_snapshot = session::SessionSnapshot {
        active_board_id: state.input_state.board_id().to_string(),
        boards: Vec::new(),
        tool_state: None,
    };
    session::save_snapshot(&empty_snapshot, options)?;
    Ok(true)
}

fn persistence_enabled(options: &session::SessionOptions) -> bool {
    options.any_enabled() || options.restore_tool_state || options.persist_history
}

pub(super) fn notify_session_failure(state: &WaylandState, err: &anyhow::Error) {
    notification::send_notification_async(
        &state.tokio_handle,
        "Failed to Save Session".to_string(),
        format!("Your drawings may not persist: {}", err),
        Some("dialog-error".to_string()),
    );
}
