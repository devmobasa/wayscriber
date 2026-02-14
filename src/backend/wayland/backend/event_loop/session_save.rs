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
        Ok(saved) => record_autosave_success(&mut state.session, now, saved),
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

fn record_autosave_success(
    session_state: &mut crate::backend::wayland::session::SessionState,
    now: Instant,
    saved: bool,
) {
    if saved {
        session_state.mark_saved(now);
    }
}

fn record_autosave_failure(
    session_state: &mut crate::backend::wayland::session::SessionState,
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
}
