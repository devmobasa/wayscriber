use crate::input::state::{Toast, ToastPriority};
use log::{info, warn};
use std::time::{Duration, Instant};

use super::super::super::state::{OverlaySuppression, WaylandState};
use super::super::helpers::friendly_capture_error;
use crate::capture::file::{FileSaveConfig, expand_tilde};
use crate::capture::{CaptureOutcome, CapturePoll, ImageOperationKind};
use crate::config::Action;
use crate::input::state::PendingBackendAction;
use crate::notification;

pub(super) fn poll_portal_captures(state: &mut WaylandState, now: Instant) {
    // Apply any completed portal fallback captures without blocking.
    state
        .frozen
        .poll_portal_capture(&mut state.input_state, now);
    handle_pending_frozen_image(state, now);
    state.zoom.poll_portal_capture(&mut state.input_state, now);
    // Portal completion can make the capture controller idle before dispatch.
    // Release its overlay suppression now so the normal blocking dispatch does
    // not wait forever for a wake that has already been consumed.
    state.apply_capture_completion();
}

pub(super) fn poll_capture_deadlines(
    state: &mut WaylandState,
    qh: &wayland_client::QueueHandle<WaylandState>,
    now: Instant,
) {
    state.poll_overlay_capture_barrier_timeout(now);
    if let Some(backend) = state.frozen.take_timed_out_direct_capture(now) {
        warn!("{backend:?} frozen capture timed out; trying the next backend");
        state.continue_frozen_capture_after_failure(backend, qh);
    }
}

pub(super) fn capture_timeout(state: &WaylandState, now: Instant) -> Option<Duration> {
    super::min_timeout(
        state.overlay_capture_barrier_timeout(now),
        super::min_timeout(
            state.frozen.direct_capture_timeout(now),
            super::min_timeout(
                state.frozen.portal_timeout(now),
                super::min_timeout(
                    state.zoom.portal_timeout(now),
                    state.xdg_frozen_fullscreen_timeout(now),
                ),
            ),
        ),
    )
}

fn handle_pending_frozen_image(state: &mut WaylandState, now: Instant) {
    if !state.frozen.has_pending_image() {
        return;
    }
    if state.surface.is_xdg_window() {
        if state.xdg_fullscreen() {
            state.activate_pending_frozen_image_for_current_surface();
            return;
        }
        if !state.xdg_frozen_fullscreen_requested() && state.begin_xdg_frozen_fullscreen() {
            return;
        }
        if state.xdg_frozen_fullscreen_pending_configure() {
            if state.xdg_frozen_fullscreen_timed_out(now) {
                warn!("Frozen xdg fullscreen configure timed out; cancelling freeze");
                state.input_state.push_toast(
                    ToastPriority::Critical,
                    "capture",
                    Toast::error("Freeze failed because fullscreen was not confirmed"),
                );
                state.restore_xdg_after_frozen();
                state.frozen.cancel(&mut state.input_state);
            }
            return;
        }
        state.activate_pending_frozen_image_for_current_surface();
        return;
    }
    state.activate_pending_frozen_image_for_current_surface();
}

pub(super) fn handle_pending_actions(
    state: &mut WaylandState,
    qh: &wayland_client::QueueHandle<WaylandState>,
) {
    state.apply_capture_completion();
    state.poll_clipboard_publish_completion();
    state.poll_clipboard_paste_completion();
    state.poll_hex_copy_completion();
    state.poll_text_copy_completion();
    state.poll_text_paste_completion();
    state.poll_session_file_dialog_completion(qh);
    state.drain_clipboard_requests();
    state.handle_pending_eyedropper_toggle();
    // Copy/paste-hex requests from the color picker popup's pointer release are
    // drained here: unlike the toolbar/key paths, that release has no other
    // drain site. An accepted quick-color recolor is queued from the same
    // release (clicking OK), so its config write belongs here too — otherwise
    // the swatch would look saved until some later key or toolbar event.
    if let Some(color) = state.input_state.take_pending_copy_hex_request() {
        state.handle_copy_hex_color(color);
    }
    if let Some(target) = state.input_state.take_pending_paste_hex_request() {
        state.handle_paste_hex_color(target);
    }
    if let Some(edit) = state.input_state.take_pending_quick_color_edit() {
        state.handle_quick_color_edit(edit);
    }
    // Every shortcut edit recorded since the last pass, not just the newest.
    // One batch of input events can produce two of them — a captured chord and
    // then a correction — and each is its own write with its own answer, so
    // keeping only the last would lose an edit with no save and no toast.
    for request in state.input_state.take_pending_keybinding_edits() {
        state.handle_keybinding_edit(request);
    }
    // Config writes that finished on the worker since the last pass. A finished
    // write is what installs a shortcut edit and what decides every one of the
    // three gestures' toasts, so it is drained on the same cadence as the
    // requests that produced it; the worker also wakes the loop when it
    // completes, so the answer is not left waiting for unrelated input.
    state.drain_config_edit_completions();
    handle_frozen_toggle(state);
    state.drain_pending_board_runtime_ui_actions();

    if let Some(action) = state.input_state.take_pending_backend_action() {
        match action {
            PendingBackendAction::Screenshot(action) => state.handle_capture_action(action),
            PendingBackendAction::CanvasExport(action) => state.handle_canvas_export_action(action),
            PendingBackendAction::BoardPdfExport(action) => {
                state.handle_board_pdf_export_action(action);
            }
            PendingBackendAction::ClearSavedToolState => {
                state.handle_clear_saved_tool_state_action();
            }
        }
    }
    state.drain_pending_toolbar_persistence();
    if let Some(action) = state.input_state.take_pending_output_focus_action() {
        state.handle_output_focus_action(qh, action);
    }
    if let Some(action) = state.input_state.take_pending_zoom_action() {
        state.handle_zoom_action(action);
    }
    state.sync_zoom_board_mode();
    state.sync_input_monitor_if_changed();

    handle_capture_results(state);
}

fn handle_frozen_toggle(state: &mut WaylandState) {
    if !state.input_state.take_pending_frozen_toggle() {
        return;
    }

    if !state.frozen_enabled() {
        warn!(
            "Frozen mode unavailable: no direct capture backend and no screenshot portal backend; ignoring toggle"
        );
        state.input_state.push_toast(
            ToastPriority::Info,
            "capture",
            Toast::warning("Freeze is unavailable because screen capture is not available."),
        );
    } else if state.frozen.is_in_progress() {
        warn!("Frozen capture already in progress; ignoring toggle");
    } else if state.input_state.frozen_active() {
        state.restore_xdg_after_frozen();
        state.frozen.unfreeze(&mut state.input_state);
    } else {
        if !state.enter_overlay_suppression(OverlaySuppression::Frozen) {
            warn!("Frozen mode requested while overlay is suppressed; ignoring toggle");
            state.input_state.push_toast(
                ToastPriority::Info,
                "capture",
                Toast::warning("Freeze is already preparing another overlay operation."),
            );
            return;
        }
        if let Err(err) = state.frozen.start_capture() {
            warn!("Frozen capture failed to start: {}", err);
            state.exit_overlay_suppression(OverlaySuppression::Frozen);
            state.frozen.cancel(&mut state.input_state);
        }
    }
}

fn handle_capture_results(state: &mut WaylandState) {
    let (id, operation, outcome) = match state.capture.manager_mut().poll() {
        CapturePoll::Idle | CapturePoll::Pending { .. } => return,
        CapturePoll::Ready {
            id,
            operation,
            outcome,
        } => (id, operation, outcome),
        CapturePoll::WorkerFailed {
            active_id,
            operation,
            error,
        } => {
            if let Some(id) = active_id {
                let _ = state.capture.consume_accepted(id);
            }
            handle_capture_manager_failure(state, operation, &error);
            return;
        }
    };

    if !state.capture.consume_accepted(id) {
        let expected = state.capture.accepted_id();
        state.capture.manager_mut().mark_unhealthy();
        handle_capture_manager_failure(
            state,
            Some(operation),
            &format!("capture completion {id} did not match accepted identity {expected:?}"),
        );
        return;
    }

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
                info!(
                    "{} saved to: {}",
                    result.operation.saved_log_label(),
                    path.display()
                );
                if let Some(filename) = path.file_name() {
                    message_parts.push(format!("Saved as {}", filename.to_string_lossy()));
                }
            }

            if result.copied_to_clipboard {
                info!("{} copied to clipboard", result.operation.saved_log_label());
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
                let mut save_config = FileSaveConfig {
                    save_directory: expand_tilde(&state.config.capture.save_directory),
                    filename_template: state.config.capture.filename_template.clone(),
                    format: state.config.capture.format.clone(),
                };
                if let Some(format) = result.fallback_format_override.as_ref() {
                    save_config.format = format.extension.clone();
                }
                // Pass exit_after_capture so we can exit after successful fallback save
                state.input_state.set_clipboard_fallback(
                    result.image_data.clone(),
                    save_config,
                    result.operation,
                    exit_after_capture,
                );
                state.input_state.push_toast(
                    ToastPriority::Critical,
                    "capture",
                    Toast::error(result.operation.fallback_toast())
                        .action("Save to file", Action::SavePendingToFile),
                );

                notification::send_notification_async(
                    &state.tokio_handle,
                    result.operation.clipboard_failure_title().to_string(),
                    "Could not copy to clipboard. Use overlay to save to file.".to_string(),
                    Some("dialog-warning".to_string()),
                );
                // Don't set should_exit - keep overlay open for fallback action
            } else if let Some(save_err) = result.save_error.as_deref() {
                // The clipboard copy succeeded but the requested file was not
                // written. Announcing success would hide exactly the loss the
                // user asked to be protected from, so this reports the failure
                // and keeps the overlay open with the same save-to-file
                // fallback the clipboard-failure path offers.
                warn!(
                    "{} copied to clipboard but the file save failed: {}",
                    result.operation.saved_log_label(),
                    save_err
                );

                let mut save_config = FileSaveConfig {
                    save_directory: expand_tilde(&state.config.capture.save_directory),
                    filename_template: state.config.capture.filename_template.clone(),
                    format: state.config.capture.format.clone(),
                };
                if let Some(format) = result.fallback_format_override.as_ref() {
                    save_config.format = format.extension.clone();
                }
                state.input_state.set_clipboard_fallback(
                    result.image_data.clone(),
                    save_config,
                    result.operation,
                    exit_after_capture,
                );
                state.input_state.push_toast(
                    ToastPriority::Critical,
                    "capture",
                    Toast::warning("Copied to clipboard, but the file was not saved".to_string())
                        .action("Save to file", Action::SavePendingToFile),
                );

                notification::send_notification_async(
                    &state.tokio_handle,
                    result.operation.save_failure_title().to_string(),
                    format!("Copied to clipboard, but the file was not saved: {save_err}"),
                    Some("dialog-warning".to_string()),
                );
                // Don't set should_exit - keep overlay open for fallback action
            } else {
                // Send normal notification.
                let notification_body = if message_parts.is_empty() {
                    match result.operation {
                        crate::capture::ImageOperationKind::Screenshot => {
                            "Screenshot captured".to_string()
                        }
                        crate::capture::ImageOperationKind::CanvasExport => {
                            "Canvas exported".to_string()
                        }
                        crate::capture::ImageOperationKind::BoardPdfExport => {
                            "Board exported".to_string()
                        }
                        crate::capture::ImageOperationKind::AllBoardsPdfExport => {
                            "Boards exported".to_string()
                        }
                    }
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
                    result.operation.success_title().to_string(),
                    notification_body,
                    Some("camera-photo".to_string()),
                );

                // Only exit on actual success (not clipboard failure)
                should_exit = exit_after_capture;
            }
        }
        CaptureOutcome::DesktopBackdropSuccess(backdrop) => {
            state.finish_pending_board_pdf_export_with_backdrop(backdrop, exit_after_capture);
        }
        CaptureOutcome::Failed { operation, message } => {
            state.capture.clear_pending_pdf_export();
            let friendly_error =
                if matches!(operation, crate::capture::ImageOperationKind::Screenshot) {
                    friendly_capture_error(&message)
                } else {
                    message.clone()
                };

            warn!("{} failed: {}", operation.saved_log_label(), message);

            state.input_state.push_toast(
                ToastPriority::Critical,
                "capture",
                Toast::error(friendly_error.clone()),
            );
            notification::send_notification_async(
                &state.tokio_handle,
                operation.failure_title().to_string(),
                friendly_error,
                Some("dialog-error".to_string()),
            );
        }
        CaptureOutcome::Cancelled { operation, reason } => {
            state.capture.clear_pending_pdf_export();
            info!("{} cancelled: {}", operation.saved_log_label(), reason);
        }
    }
    if should_exit {
        state.input_state.should_exit = true;
    }
}

fn handle_capture_manager_failure(
    state: &mut WaylandState,
    operation: Option<ImageOperationKind>,
    error: &str,
) {
    state.capture.clear_preflight();
    state.capture.clear_pending_pdf_export();
    state.show_overlay();
    state.capture.clear_in_progress();
    state.capture.clear_exit_on_success();

    let message = match operation {
        Some(ImageOperationKind::Screenshot) => friendly_capture_error(error),
        Some(operation) => format!(
            "{} failed because the capture worker stopped.",
            operation.saved_log_label()
        ),
        None => "Capture services stopped unexpectedly.".to_string(),
    };
    warn!("Capture manager failure: {error}");
    state.input_state.push_toast(
        ToastPriority::Critical,
        "capture",
        Toast::error(message.clone()),
    );
    notification::send_notification_async(
        &state.tokio_handle,
        operation
            .map(ImageOperationKind::failure_title)
            .unwrap_or("Capture failed")
            .to_string(),
        message,
        Some("dialog-error".to_string()),
    );
}
