use super::super::state::WaylandState;
use crate::config::Action;
use crate::tray_action::TrayAction;

pub(super) fn process_tray_action(state: &mut WaylandState) -> bool {
    let actions = crate::tray_action::take_pending_actions();
    let processed = !actions.is_empty();
    for action in actions {
        apply_tray_action(state, action);
    }
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
