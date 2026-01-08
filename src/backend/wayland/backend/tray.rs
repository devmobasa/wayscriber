use log::warn;
use std::fs;

use super::super::state::WaylandState;
use crate::config::Action;
use crate::tray_action::TrayAction;

pub(super) fn process_tray_action(state: &mut WaylandState) {
    let action_path = crate::paths::tray_action_file();
    // Read-and-delete best effort; if a new action is written between read and delete
    // it will be picked up on the next signal/start.
    let action_str = match fs::read_to_string(&action_path) {
        Ok(content) => content.lines().next().unwrap_or("").trim().to_string(),
        Err(err) => {
            if err.kind() != std::io::ErrorKind::NotFound {
                warn!(
                    "Tray action signal received but failed to read {}: {}",
                    action_path.display(),
                    err
                );
            }
            return;
        }
    };

    if action_str.is_empty() {
        let _ = fs::remove_file(&action_path);
        return;
    }

    let action = match TrayAction::parse(action_str.as_str()) {
        Some(action) => action,
        None => {
            warn!("Unknown tray action '{}'", action_str);
            let _ = fs::remove_file(&action_path);
            return;
        }
    };

    match action {
        TrayAction::ToggleFreeze => {
            state.input_state.request_frozen_toggle();
            state.input_state.needs_redraw = true;
        }
        TrayAction::CaptureFull => state.handle_capture_action(Action::CaptureFullScreen),
        TrayAction::CaptureWindow => state.handle_capture_action(Action::CaptureActiveWindow),
        TrayAction::CaptureRegion => {
            // Honor clipboard preference for region captures
            if state.config.capture.copy_to_clipboard {
                state.handle_capture_action(Action::CaptureClipboardRegion);
            } else {
                state.handle_capture_action(Action::CaptureFileRegion);
            }
        }
        TrayAction::ToggleHelp => {
            state.input_state.toggle_help_overlay();
        }
    }

    let _ = fs::remove_file(&action_path);
}
