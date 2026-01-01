use super::super::super::state::WaylandState;
use crate::{notification, session};

pub(super) fn persist_session(state: &WaylandState) -> Result<(), anyhow::Error> {
    if let Some(options) = state.session_options()
        && let Some(snapshot) = session::snapshot_from_input(&state.input_state, options)
    {
        session::save_snapshot(&snapshot, options)?;
    }
    Ok(())
}

pub(super) fn notify_session_failure(state: &WaylandState, err: &anyhow::Error) {
    notification::send_notification_async(
        &state.tokio_handle,
        "Failed to Save Session".to_string(),
        format!("Your drawings may not persist: {}", err),
        Some("dialog-error".to_string()),
    );
}
