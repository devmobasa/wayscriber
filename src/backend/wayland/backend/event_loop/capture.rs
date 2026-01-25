use log::{info, warn};
use wayland_client::Connection;

use super::super::super::state::{OverlaySuppression, WaylandState};
use super::super::helpers::friendly_capture_error;
use crate::capture::CaptureOutcome;
use crate::capture::file::{FileSaveConfig, expand_tilde};
use crate::config::Action;
use crate::input::state::UiToastKind;
use crate::notification;

pub(super) fn poll_portal_captures(state: &mut WaylandState) {
    // Apply any completed portal fallback captures without blocking.
    state.frozen.poll_portal_capture(&mut state.input_state);
    state.zoom.poll_portal_capture(&mut state.input_state);
}

pub(super) fn flush_if_capture_active(conn: &Connection, capture_active: bool) {
    if capture_active {
        let _ = conn.flush();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

pub(super) fn handle_pending_actions(state: &mut WaylandState) {
    state.apply_capture_completion();
    handle_frozen_toggle(state);

    if let Some(action) = state.input_state.take_pending_zoom_action() {
        state.handle_zoom_action(action);
    }
    if let Some(boards) = state.input_state.take_pending_board_config() {
        state.apply_board_config_update(boards);
    }
    state.sync_zoom_board_mode();

    handle_capture_results(state);
}

fn handle_frozen_toggle(state: &mut WaylandState) {
    if !state.input_state.take_pending_frozen_toggle() {
        return;
    }

    if !state.frozen_enabled() {
        warn!("Frozen mode disabled on this compositor (xdg fallback); ignoring toggle");
    } else if state.frozen.is_in_progress() {
        warn!("Frozen capture already in progress; ignoring toggle");
    } else if state.input_state.frozen_active() {
        state.frozen.unfreeze(&mut state.input_state);
    } else {
        let use_fallback = !state.frozen.manager_available();
        if use_fallback {
            warn!("Frozen mode: screencopy unavailable, using portal fallback");
        } else {
            info!("Frozen mode: using screencopy fast path");
        }
        state.enter_overlay_suppression(OverlaySuppression::Frozen);
        if let Err(err) = state
            .frozen
            .start_capture(use_fallback, &state.tokio_handle)
        {
            warn!("Frozen capture failed to start: {}", err);
            state.exit_overlay_suppression(OverlaySuppression::Frozen);
            state.frozen.cancel(&mut state.input_state);
        }
    }
}

fn handle_capture_results(state: &mut WaylandState) {
    if !state.capture.is_in_progress() {
        return;
    }

    let Some(outcome) = state.capture.manager_mut().try_take_result() else {
        return;
    };

    info!("Capture completed");

    // Restore overlay.
    state.show_overlay();
    state.capture.clear_in_progress();

    let exit_after_capture = state.capture.take_exit_on_success();
    let mut should_exit = false;

    match outcome {
        CaptureOutcome::Success(result) => {
            // Build notification message.
            let mut message_parts = Vec::new();

            if let Some(ref path) = result.saved_path {
                info!("Screenshot saved to: {}", path.display());
                if let Some(filename) = path.file_name() {
                    message_parts.push(format!("Saved as {}", filename.to_string_lossy()));
                }
            }

            if result.copied_to_clipboard {
                info!("Screenshot copied to clipboard");
                message_parts.push("Copied to clipboard".to_string());
            }

            // Handle clipboard failure with fallback option
            let clipboard_failed = !result.copied_to_clipboard
                && result.saved_path.is_none()
                && !result.image_data.is_empty();

            if clipboard_failed {
                // Clipboard was the only destination and it failed - don't exit,
                // keep overlay open so user can click "Save to file"
                warn!("Clipboard copy failed, offering save-to-file fallback");

                // Build save config from user preferences for fallback save
                let save_config = FileSaveConfig {
                    save_directory: expand_tilde(&state.config.capture.save_directory),
                    filename_template: state.config.capture.filename_template.clone(),
                    format: state.config.capture.format.clone(),
                };
                // Pass exit_after_capture so we can exit after successful fallback save
                state.input_state.set_clipboard_fallback(
                    result.image_data.clone(),
                    save_config,
                    exit_after_capture,
                );
                state.input_state.set_ui_toast_with_action(
                    UiToastKind::Error,
                    "Clipboard failed",
                    "Save to file",
                    Action::SavePendingToFile,
                );

                notification::send_notification_async(
                    &state.tokio_handle,
                    "Screenshot Clipboard Failed".to_string(),
                    "Could not copy to clipboard. Use overlay to save to file.".to_string(),
                    Some("dialog-warning".to_string()),
                );
                // Don't set should_exit - keep overlay open for fallback action
            } else {
                // Send normal notification.
                let notification_body = if message_parts.is_empty() {
                    "Screenshot captured".to_string()
                } else {
                    message_parts.join(" - ")
                };

                let open_folder_binding = state
                    .config
                    .keybindings
                    .capture
                    .open_capture_folder
                    .first()
                    .map(|binding| binding.as_str());
                state.input_state.set_capture_feedback(
                    result.saved_path.as_deref(),
                    result.copied_to_clipboard,
                    open_folder_binding,
                );

                notification::send_notification_async(
                    &state.tokio_handle,
                    "Screenshot Captured".to_string(),
                    notification_body,
                    Some("camera-photo".to_string()),
                );

                // Only exit on actual success (not clipboard failure)
                should_exit = exit_after_capture;
            }
        }
        CaptureOutcome::Failed(error) => {
            let friendly_error = friendly_capture_error(&error);

            warn!("Screenshot capture failed: {}", error);

            notification::send_notification_async(
                &state.tokio_handle,
                "Screenshot Failed".to_string(),
                friendly_error,
                Some("dialog-error".to_string()),
            );
        }
        CaptureOutcome::Cancelled(reason) => {
            info!("Capture cancelled: {}", reason);
        }
    }
    if should_exit {
        state.input_state.should_exit = true;
    }
}
