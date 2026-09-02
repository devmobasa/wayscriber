use crate::backend::wayland::acquisition::{
    AcquisitionRecord, AcquisitionStage, ScreenAcquisitionOutcome, ScreenAcquisitionOwner,
};
use crate::input::state::{Toast, ToastPriority};
use log::{info, warn};
use std::time::{Duration, Instant};

use super::super::super::state::{OverlaySuppression, WaylandState};
use super::super::helpers::friendly_capture_error;
use crate::capture::file::{FileSaveConfig, expand_tilde};
use crate::capture::{CaptureOutcome, CapturePoll, CaptureRequestId, ImageOperationKind};
use crate::config::Action;
use crate::input::state::{InputEffect, InputEffectDrain, PendingBackendAction};
use crate::notification;

pub(super) fn poll_portal_captures(state: &mut WaylandState, now: Instant) {
    // Apply any completed portal fallback captures without blocking.
    state
        .frozen
        .poll_portal_capture(&mut state.input_state, now);
    handle_pending_frozen_image(state, now);
    let live_output_count = state.live_output_count();
    state
        .zoom
        .poll_portal_capture(&mut state.input_state, now, live_output_count);
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
                state.restore_xdg_after_frozen();
                if state.frozen.has_acquisition_attempt() {
                    state.frozen.finish_acquisition(
                        ScreenAcquisitionOutcome::Failed(
                            "Freeze failed because fullscreen was not confirmed".to_string(),
                        ),
                        &mut state.input_state,
                    );
                } else {
                    state.input_state.push_toast(
                        ToastPriority::Critical,
                        "capture",
                        Toast::error("Freeze failed because fullscreen was not confirmed"),
                    );
                    state.frozen.cancel(&mut state.input_state);
                }
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
    state.poll_region_window_query_completion();
    state.poll_region_cut_preview_completion();
    state.poll_ocr_completion();
    state.poll_session_file_dialog_completion(qh);
    state.poll_desktop_open_completion();
    state.drain_clipboard_requests();
    let effects = state
        .input_state
        .drain_input_effects(InputEffectDrain::Runtime);
    let mut config_completions_drained = false;
    let mut toolbar_persistence_drained = false;
    for effect in effects {
        if !config_completions_drained
            && matches!(
                effect,
                InputEffect::Backend(_)
                    | InputEffect::FrozenPass { .. }
                    | InputEffect::BoardRuntimeUi(_)
                    | InputEffect::SpotlightMagnifierFeedback
                    | InputEffect::OutputFocus(_)
                    | InputEffect::Zoom(_)
            )
        {
            state.drain_config_edit_completions();
            config_completions_drained = true;
        }
        if !toolbar_persistence_drained
            && matches!(effect, InputEffect::OutputFocus(_) | InputEffect::Zoom(_))
        {
            state.drain_pending_toolbar_persistence();
            toolbar_persistence_drained = true;
        }
        match effect {
            InputEffect::EyedropperToggle => state.handle_eyedropper_toggle(),
            InputEffect::OcrPass {
                requested,
                dismissed_by_toolbar,
            } => {
                if requested {
                    state.handle_ocr_request(dismissed_by_toolbar);
                }
            }
            InputEffect::CopyHex(color) => state.handle_copy_hex_color(color),
            InputEffect::PasteHex(target) => state.handle_paste_hex_color(target),
            InputEffect::QuickColor(edit) => state.handle_quick_color_edit(edit),
            InputEffect::KeybindingEdit(request) => state.handle_keybinding_edit(request),
            InputEffect::Backend(action) => apply_backend_effect(state, action),
            InputEffect::FrozenPass { user_requested } => {
                handle_frozen_toggle(state, user_requested);
            }
            InputEffect::BoardRuntimeUi(action) => state.apply_board_runtime_ui_action(action),
            InputEffect::SpotlightMagnifierFeedback => {
                state.show_spotlight_magnifier_feedback_if_unavailable();
            }
            InputEffect::OutputFocus(action) => state.handle_output_focus_action(qh, action),
            InputEffect::Zoom(action) => state.handle_zoom_action(action),
            effect @ (InputEffect::ToolbarPersistence(_)
            | InputEffect::TextCopy(_)
            | InputEffect::TextPaste(_)
            | InputEffect::SelectionClipboardPublish(_)
            | InputEffect::ClipboardPaste(_)
            | InputEffect::Preset(_)) => {
                unreachable!("runtime drain returned {effect:?}")
            }
        }
    }
    // Config writes that finished on the worker since the last pass. A finished
    // write is what installs a shortcut edit and what decides every one of the
    // three gestures' toasts, so it is drained on the same cadence as the
    // requests that produced it; the worker also wakes the loop when it
    // completes, so the answer is not left waiting for unrelated input.
    if !config_completions_drained {
        state.drain_config_edit_completions();
    }
    if !toolbar_persistence_drained {
        state.drain_pending_toolbar_persistence();
    }
    state.sync_zoom_board_mode();
    state.resolve_pending_zoom_terminal();
    state.sync_input_monitor_if_changed();

    handle_capture_results(state);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrozenUserToggleAction {
    None,
    AbsorbQueuedModal,
    IgnoreInProgress,
    Unfreeze,
    ReportUnavailable,
    RequestUserFreeze,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FrozenTogglePassDecision {
    user_action: FrozenUserToggleAction,
    queued_to_start: Option<AcquisitionRecord>,
}

fn apply_backend_effect(state: &mut WaylandState, action: PendingBackendAction) {
    match action {
        PendingBackendAction::Screenshot(action) => state.handle_capture_action(action),
        PendingBackendAction::MeasureMode => state.handle_measure_mode_action(),
        PendingBackendAction::CanvasExport(action) => state.handle_canvas_export_action(action),
        PendingBackendAction::BoardPdfExport(action) => {
            state.handle_board_pdf_export_action(action);
        }
        PendingBackendAction::DesktopOpen(request) => state.handle_desktop_open(request),
        PendingBackendAction::HelperLaunch(request) => state.handle_helper_launch(request),
        PendingBackendAction::ClearSavedToolState => {
            state.handle_clear_saved_tool_state_action();
        }
    }
}

fn frozen_toggle_pass_decision(
    user_toggle: bool,
    slot: Option<AcquisitionRecord>,
    frozen_active: bool,
    frozen_enabled: bool,
) -> FrozenTogglePassDecision {
    let user_action = if !user_toggle {
        FrozenUserToggleAction::None
    } else {
        match slot {
            Some(record)
                if record.stage == AcquisitionStage::Queued
                    && record.owner != ScreenAcquisitionOwner::UserFreeze =>
            {
                FrozenUserToggleAction::AbsorbQueuedModal
            }
            Some(_) => FrozenUserToggleAction::IgnoreInProgress,
            None if frozen_active => FrozenUserToggleAction::Unfreeze,
            None if !frozen_enabled => FrozenUserToggleAction::ReportUnavailable,
            None => FrozenUserToggleAction::RequestUserFreeze,
        }
    };
    FrozenTogglePassDecision {
        user_action,
        queued_to_start: slot.filter(|record| record.stage == AcquisitionStage::Queued),
    }
}

fn handle_frozen_toggle(state: &mut WaylandState, user_requested: bool) {
    let decision = frozen_toggle_pass_decision(
        user_requested,
        state.screen_acquisition_slot(),
        state.input_state.frozen_active(),
        state.frozen_enabled(),
    );
    match decision.user_action {
        FrozenUserToggleAction::None => {}
        FrozenUserToggleAction::AbsorbQueuedModal => {
            let record = decision
                .queued_to_start
                .expect("absorbed modal acquisition remains queued");
            log::debug!(
                "User freeze toggle absorbed by queued {:?} acquisition",
                record.owner
            );
        }
        FrozenUserToggleAction::IgnoreInProgress => {
            warn!("Frozen capture already in progress; ignoring toggle");
        }
        FrozenUserToggleAction::Unfreeze => {
            state.restore_xdg_after_frozen();
            state.frozen.unfreeze(&mut state.input_state);
            state.cancel_screen_modals_if_source_changed();
        }
        FrozenUserToggleAction::ReportUnavailable => {
            warn!(
                "Frozen mode unavailable: no direct capture backend and no screenshot portal backend; ignoring toggle"
            );
            state.input_state.push_toast(
                ToastPriority::Info,
                "capture",
                Toast::warning("Freeze is unavailable because screen capture is not available."),
            );
        }
        FrozenUserToggleAction::RequestUserFreeze => {
            let _ = state.request_screen_acquisition(ScreenAcquisitionOwner::UserFreeze);
        }
    }

    let record = if decision.user_action == FrozenUserToggleAction::RequestUserFreeze {
        state.queued_screen_acquisition()
    } else {
        decision.queued_to_start
    };
    let Some(record) = record else {
        return;
    };
    if !state.enter_overlay_suppression(OverlaySuppression::Frozen) {
        warn!("Frozen mode requested while overlay is suppressed; ignoring toggle");
        state.complete_queued_acquisition(
            record.id,
            record.owner,
            ScreenAcquisitionOutcome::Unavailable,
        );
        return;
    }
    match state.frozen.start_capture_for(record.id, record.owner) {
        Ok(()) => {
            state.mark_screen_acquisition_started(record.id, record.owner);
        }
        Err(err) => {
            warn!("Frozen capture failed to start: {err}");
            state.exit_overlay_suppression(OverlaySuppression::Frozen);
            state.complete_queued_acquisition(
                record.id,
                record.owner,
                ScreenAcquisitionOutcome::Unavailable,
            );
        }
    }
}

fn handle_capture_worker_failure(
    state: &mut WaylandState,
    active_id: Option<CaptureRequestId>,
    operation: Option<ImageOperationKind>,
    error: &str,
) {
    let pending_board = active_id.and_then(|id| state.capture.take_pending_board_paste_for(id));
    if let Some(id) = active_id {
        let _ = state.capture.consume_accepted(id);
    }
    if pending_board.is_some() {
        state.capture.finish_capture_lifecycle();
        let message = operation
            .filter(|operation| *operation == ImageOperationKind::Screenshot)
            .map(|_| friendly_capture_error(error))
            .unwrap_or_else(|| "Capture services stopped unexpectedly.".to_string());
        warn!("Board region capture worker failed: {error}");
        state
            .input_state
            .push_toast(ToastPriority::Critical, "capture", Toast::error(message));
        return;
    }
    handle_capture_manager_failure(state, operation, error);
}

fn resolve_board_capture_outcome(
    state: &mut WaylandState,
    id: CaptureRequestId,
    outcome: CaptureOutcome,
) -> Option<CaptureOutcome> {
    let pending_board = state.capture.take_pending_board_paste_for(id);
    match (outcome, pending_board) {
        (CaptureOutcome::RenderedImageReady(image), Some(pending)) => {
            state.capture.finish_capture_lifecycle();
            let embedded = crate::draw::EmbeddedImage {
                mime_type: image.format.mime_type,
                width: image.width,
                height: image.height,
                bytes: image.bytes.into(),
            };
            state
                .input_state
                .insert_captured_image(embedded, &pending.target);
            None
        }
        (CaptureOutcome::RenderedImageReady(_), None) => {
            state.capture.finish_capture_lifecycle();
            warn!("Rendered region {id} completed without a pending board target");
            state.input_state.push_toast(
                ToastPriority::Critical,
                "capture.region.board",
                Toast::error("Region was not added to the board."),
            );
            None
        }
        (CaptureOutcome::Failed { operation, message }, Some(_)) => {
            state.capture.finish_capture_lifecycle();
            let friendly_error = if matches!(operation, ImageOperationKind::Screenshot) {
                friendly_capture_error(&message)
            } else {
                message.clone()
            };
            warn!("Board region render failed: {message}");
            state.input_state.push_toast(
                ToastPriority::Critical,
                "capture",
                Toast::error(friendly_error),
            );
            None
        }
        (CaptureOutcome::Cancelled { operation, reason }, Some(_)) => {
            state.capture.finish_capture_lifecycle();
            info!("{} cancelled: {}", operation.saved_log_label(), reason);
            None
        }
        (unexpected, Some(_)) => {
            state.capture.finish_capture_lifecycle();
            warn!("Board region {id} completed with unexpected outcome: {unexpected:?}");
            state.input_state.push_toast(
                ToastPriority::Critical,
                "capture.region.board",
                Toast::error("Region was not added to the board."),
            );
            None
        }
        (outcome, None) => Some(outcome),
    }
}

fn handle_capture_results(state: &mut WaylandState) {
    let Some((id, outcome)) = poll_accepted_capture(state) else {
        return;
    };

    info!("Capture completed");

    let Some(outcome) = resolve_board_capture_outcome(state, id, outcome) else {
        return;
    };

    // Restore overlay.
    state.show_overlay();
    let exit_after_capture = state.capture.exit_on_success();
    state.capture.finish_capture_lifecycle();
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
        CaptureOutcome::RenderedImageReady(_) => {
            unreachable!("rendered images return through the board path above")
        }
    }
    if should_exit {
        // Exit-after-capture is intentional teardown. Mark it explicit so XDG
        // stay-mode cannot clear should_exit while the overlay is unfocused
        // (for example after a portal dialog stole focus during capture).
        state.mark_xdg_explicit_close_requested();
        state.input_state.should_exit = true;
    }
}

fn poll_accepted_capture(state: &mut WaylandState) -> Option<(CaptureRequestId, CaptureOutcome)> {
    let (id, operation, outcome) = match state.capture.manager_mut().poll() {
        CapturePoll::Idle | CapturePoll::Pending { .. } => return None,
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
            handle_capture_worker_failure(state, active_id, operation, &error);
            return None;
        }
    };
    if state.capture.consume_accepted(id) {
        return Some((id, outcome));
    }

    let expected = state.capture.accepted_id();
    state.capture.manager_mut().mark_unhealthy();
    handle_capture_manager_failure(
        state,
        Some(operation),
        &format!("capture completion {id} did not match accepted identity {expected:?}"),
    );
    None
}

fn handle_capture_manager_failure(
    state: &mut WaylandState,
    operation: Option<ImageOperationKind>,
    error: &str,
) {
    state.capture.clear_pending_pdf_export();
    state.show_overlay();
    state.capture.finish_capture_lifecycle();

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::acquisition::ScreenAcquisitionRegistry;

    fn record(
        owner: ScreenAcquisitionOwner,
        stage: AcquisitionStage,
    ) -> crate::backend::wayland::acquisition::AcquisitionRecord {
        let mut registry = ScreenAcquisitionRegistry::default();
        let id = registry.request(owner).expect("test acquisition");
        if stage == AcquisitionStage::Started {
            assert!(registry.mark_started(id, owner));
        }
        registry.take().expect("test record")
    }

    #[test]
    fn queued_modal_absorbs_same_batch_user_toggle_then_still_drains_modal() {
        for owner in [
            ScreenAcquisitionOwner::Eyedropper,
            ScreenAcquisitionOwner::Ocr,
            ScreenAcquisitionOwner::RegionCapture,
        ] {
            let modal = record(owner, AcquisitionStage::Queued);

            let decision = frozen_toggle_pass_decision(true, Some(modal), false, true);

            assert_eq!(
                decision.user_action,
                FrozenUserToggleAction::AbsorbQueuedModal
            );
            assert_eq!(decision.queued_to_start, Some(modal));
        }
    }

    #[test]
    fn started_modal_ignores_same_batch_user_toggle_without_starting_another_capture() {
        for owner in [
            ScreenAcquisitionOwner::Ocr,
            ScreenAcquisitionOwner::RegionCapture,
        ] {
            let modal = record(owner, AcquisitionStage::Started);

            let decision = frozen_toggle_pass_decision(true, Some(modal), false, true);

            assert_eq!(
                decision.user_action,
                FrozenUserToggleAction::IgnoreInProgress,
                "owner={owner:?}"
            );
            assert_eq!(decision.queued_to_start, None, "owner={owner:?}");
        }
    }

    #[test]
    fn queued_acquisition_drains_with_or_without_a_user_toggle() {
        let modal = record(ScreenAcquisitionOwner::Ocr, AcquisitionStage::Queued);
        let user = record(ScreenAcquisitionOwner::UserFreeze, AcquisitionStage::Queued);

        assert_eq!(
            frozen_toggle_pass_decision(false, Some(modal), false, true),
            FrozenTogglePassDecision {
                user_action: FrozenUserToggleAction::None,
                queued_to_start: Some(modal),
            }
        );
        assert_eq!(
            frozen_toggle_pass_decision(true, Some(user), false, true),
            FrozenTogglePassDecision {
                user_action: FrozenUserToggleAction::IgnoreInProgress,
                queued_to_start: Some(user),
            }
        );
    }
}
