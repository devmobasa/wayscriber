use super::super::state::WaylandState;
use crate::config::Action;
use crate::daemon::protocol_v2::{ActionClaimOutcome, ActionFinishOutcome};
use crate::tray_action::TrayAction;
use std::time::{Duration, Instant};

const DURABLE_ACTION_RETRY_DELAY: Duration = Duration::from_millis(10);
const MAX_DURABLE_ACTIONS_PER_DRAIN: usize = 64;

pub(super) fn durable_action_retry_due(state: &WaylandState, now: Instant) -> bool {
    state
        .durable_action_retry_at
        .is_some_and(|deadline| now >= deadline)
}

pub(super) fn durable_action_retry_timeout(state: &WaylandState, now: Instant) -> Option<Duration> {
    state
        .durable_action_retry_at
        .map(|deadline| deadline.saturating_duration_since(now))
}

fn defer_durable_action(
    state: &mut WaylandState,
    action: Option<crate::daemon::protocol_v2::ClaimedAction>,
) {
    state.durable_action_finish = action;
    state.durable_action_retry_at = Some(Instant::now() + DURABLE_ACTION_RETRY_DELAY);
}

pub(super) fn process_tray_action(state: &mut WaylandState) -> bool {
    let actions = crate::tray_action::take_pending_actions();
    let mut processed = !actions.is_empty();
    for action in actions {
        apply_tray_action(state, action);
    }

    let now = Instant::now();
    if state
        .durable_action_retry_at
        .is_some_and(|deadline| now < deadline)
    {
        return processed;
    }
    state.durable_action_retry_at = None;

    if let Some(action) = state.durable_action_finish.take() {
        match action.try_finish(true, None) {
            Ok(ActionFinishOutcome::Complete) => {}
            Ok(ActionFinishOutcome::Deferred(action)) => {
                defer_durable_action(state, Some(action));
                return processed;
            }
            Err(error) => {
                log::error!("Failed to finish durable daemon action: {error:#}");
                defer_durable_action(state, None);
                return processed;
            }
        }
    }

    for _ in 0..MAX_DURABLE_ACTIONS_PER_DRAIN {
        match crate::daemon::try_claim_overlay_action() {
            Ok(ActionClaimOutcome::Claimed(action)) => {
                apply_tray_action(state, action.action());
                processed = true;
                match action.try_finish(true, None) {
                    Ok(ActionFinishOutcome::Complete) => {}
                    Ok(ActionFinishOutcome::Deferred(action)) => {
                        defer_durable_action(state, Some(action));
                        return processed;
                    }
                    Err(error) => {
                        log::error!("Failed to finish durable daemon action: {error:#}");
                        defer_durable_action(state, None);
                        return processed;
                    }
                }
            }
            Ok(ActionClaimOutcome::Idle) => return processed,
            Ok(ActionClaimOutcome::Deferred) => {
                defer_durable_action(state, None);
                return processed;
            }
            Err(error) => {
                log::error!("Failed to claim durable daemon action: {error:#}");
                return processed;
            }
        }
    }
    defer_durable_action(state, None);
    processed
}

fn apply_tray_action(state: &mut WaylandState, action: TrayAction) {
    match action {
        TrayAction::ToggleFreeze => {
            state.input_state.request_frozen_toggle();
            state.input_state.needs_redraw = true;
        }
        TrayAction::CaptureFull => state.handle_capture_action(Action::CaptureFullScreen),
        TrayAction::CaptureWindow => state.handle_capture_action(Action::CaptureActiveWindow),
        TrayAction::CaptureRegion => {
            // Honor clipboard preference for region captures.
            if state.config.capture.copy_to_clipboard {
                state.handle_capture_action(Action::CaptureClipboardRegion);
            } else {
                state.handle_capture_action(Action::CaptureFileRegion);
            }
        }
        TrayAction::ToggleHelp => {
            state.input_state.toggle_help_overlay();
        }
        TrayAction::ToggleBoardPicker => {
            state.input_state.toggle_board_picker();
            state.input_state.needs_redraw = true;
        }
        TrayAction::ToggleLightMode => {
            state.input_state.toggle_light_mode();
            state.input_state.needs_redraw = true;
        }
        TrayAction::LightDrawToggle => {
            state.input_state.toggle_light_mode_drawing();
            state.input_state.needs_redraw = true;
        }
        TrayAction::LightDrawOn => {
            state.input_state.set_light_mode_drawing(true);
            state.input_state.needs_redraw = true;
        }
        TrayAction::LightDrawOff => {
            state.input_state.set_light_mode_drawing(false);
            state.input_state.needs_redraw = true;
        }
    }
}
