use log::{debug, info, warn};
use std::sync::atomic::Ordering;
use wayland_client::{Connection, EventQueue, backend::WaylandError};

use super::super::state::{OverlaySuppression, WaylandState};
use super::helpers::{dispatch_with_timeout, friendly_capture_error};
use super::signals::setup_signal_handlers;
use super::tray::process_tray_action;
use crate::capture::CaptureOutcome;
use crate::{notification, session};

pub(super) struct EventLoopOutcome {
    pub(super) loop_error: Option<anyhow::Error>,
}

pub(super) fn run_event_loop(
    conn: &Connection,
    event_queue: &mut EventQueue<WaylandState>,
    qh: &wayland_client::QueueHandle<WaylandState>,
    state: &mut WaylandState,
) -> EventLoopOutcome {
    // Gracefully exit the overlay when external signals request termination
    let (exit_flag, tray_action_flag) = setup_signal_handlers();

    // Track consecutive render failures for error recovery
    let mut consecutive_render_failures = 0u32;
    const MAX_RENDER_FAILURES: u32 = 10;

    // Main event loop
    let mut loop_error: Option<anyhow::Error> = None;
    loop {
        if exit_flag
            .as_ref()
            .map(|flag| flag.load(Ordering::Acquire))
            .unwrap_or(false)
        {
            state.input_state.should_exit = true;
        }

        // Check if we should exit before blocking
        if state.input_state.should_exit {
            info!("Exit requested, breaking event loop");
            break;
        }

        // Apply any completed portal fallback captures without blocking.
        state.frozen.poll_portal_capture(&mut state.input_state);
        state.zoom.poll_portal_capture(&mut state.input_state);

        if tray_action_flag
            .as_ref()
            .map(|flag| flag.swap(false, Ordering::AcqRel))
            .unwrap_or(false)
        {
            process_tray_action(state);
        }

        let capture_active = state.capture.is_in_progress()
            || state.frozen.is_in_progress()
            || state.zoom.is_in_progress()
            || state.overlay_suppressed();
        let frame_callback_pending = state.surface.frame_callback_pending();
        let vsync_enabled = state.config.performance.enable_vsync;
        let animation_timeout = if capture_active
            || !state.surface.is_configured()
            || (vsync_enabled && frame_callback_pending)
        {
            None
        } else {
            state.ui_animation_timeout(std::time::Instant::now())
        };

        let mut dispatch_error: Option<anyhow::Error> = None;
        if capture_active {
            if let Err(e) = event_queue.dispatch_pending(state) {
                dispatch_error = Some(anyhow::anyhow!("Wayland event queue error: {}", e));
            }

            if dispatch_error.is_none()
                && let Err(e) = event_queue.flush()
            {
                dispatch_error = Some(anyhow::anyhow!("Wayland flush error: {}", e));
            }

            if dispatch_error.is_none()
                && let Some(guard) = event_queue.prepare_read()
            {
                match guard.read() {
                    Ok(_) => {
                        if let Err(e) = event_queue.dispatch_pending(state) {
                            dispatch_error =
                                Some(anyhow::anyhow!("Wayland event queue error: {}", e));
                        }
                    }
                    Err(WaylandError::Io(err)) if err.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(err) => {
                        dispatch_error = Some(anyhow::anyhow!("Wayland read error: {}", err));
                    }
                }
            }
        } else if let Err(e) = dispatch_with_timeout(event_queue, state, animation_timeout) {
            dispatch_error = Some(anyhow::anyhow!("Wayland event queue error: {}", e));
        }

        match dispatch_error {
            None => {
                // Check immediately after dispatch returns
                if state.input_state.should_exit {
                    info!("Exit requested after dispatch, breaking event loop");
                    break;
                }
                // Adjust keyboard interactivity if toolbar visibility changed.
                state.sync_toolbar_visibility(qh);

                // Advance any delayed history playback (undo/redo with delay).
                if state
                    .input_state
                    .tick_delayed_history(std::time::Instant::now())
                {
                    state.toolbar.mark_dirty();
                    state.input_state.needs_redraw = true;
                }
                if state.input_state.has_pending_history() {
                    state.input_state.needs_redraw = true;
                }
            }
            Some(e) => {
                warn!("Event queue error: {}", e);
                loop_error = Some(e);
                break;
            }
        }

        if !capture_active && state.ui_animation_due(std::time::Instant::now()) {
            state.input_state.needs_redraw = true;
        }

        if capture_active {
            let _ = conn.flush();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        state.apply_capture_completion();

        if state.input_state.take_pending_frozen_toggle() {
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
                    log::info!("Frozen mode: using screencopy fast path");
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

        if let Some(action) = state.input_state.take_pending_zoom_action() {
            state.handle_zoom_action(action);
        }
        state.sync_zoom_board_mode();

        // Check for completed capture operations
        if state.capture.is_in_progress()
            && let Some(outcome) = state.capture.manager_mut().try_take_result()
        {
            log::info!("Capture completed");

            // Restore overlay
            state.show_overlay();
            state.capture.clear_in_progress();

            let exit_on_success = state.capture.take_exit_on_success()
                && matches!(&outcome, CaptureOutcome::Success(_));
            match outcome {
                CaptureOutcome::Success(result) => {
                    // Build notification message
                    let mut message_parts = Vec::new();

                    if let Some(ref path) = result.saved_path {
                        log::info!("Screenshot saved to: {}", path.display());
                        if let Some(filename) = path.file_name() {
                            message_parts.push(format!("Saved as {}", filename.to_string_lossy()));
                        }
                    }

                    if result.copied_to_clipboard {
                        log::info!("Screenshot copied to clipboard");
                        message_parts.push("Copied to clipboard".to_string());
                    }

                    // Send notification
                    let notification_body = if message_parts.is_empty() {
                        "Screenshot captured".to_string()
                    } else {
                        message_parts.join(" - ")
                    };

                    let open_folder_binding = state
                        .config
                        .keybindings
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
                }
                CaptureOutcome::Failed(error) => {
                    let friendly_error = friendly_capture_error(&error);

                    log::warn!("Screenshot capture failed: {}", error);

                    notification::send_notification_async(
                        &state.tokio_handle,
                        "Screenshot Failed".to_string(),
                        friendly_error,
                        Some("dialog-error".to_string()),
                    );
                }
                CaptureOutcome::Cancelled(reason) => {
                    log::info!("Capture cancelled: {}", reason);
                }
            }
            if exit_on_success {
                state.input_state.should_exit = true;
            }
        }

        // Render if configured and needs redraw, but only if no frame callback pending
        // This throttles rendering to display refresh rate (when vsync is enabled)
        let can_render = state.surface.is_configured()
            && state.input_state.needs_redraw
            && (!state.surface.frame_callback_pending() || !state.config.performance.enable_vsync);

        if can_render {
            debug!(
                "Main loop: needs_redraw=true, frame_callback_pending={}, triggering render",
                state.surface.frame_callback_pending()
            );
            match state.render(qh) {
                Ok(keep_rendering) => {
                    // Reset failure counter on successful render
                    consecutive_render_failures = 0;
                    state.input_state.needs_redraw =
                        keep_rendering || state.input_state.has_pending_history();
                    // Only set frame_callback_pending if vsync is enabled
                    if state.config.performance.enable_vsync {
                        state.surface.set_frame_callback_pending(true);
                        debug!(
                            "Main loop: render complete, frame_callback_pending set to true (vsync enabled)"
                        );
                    } else {
                        debug!(
                            "Main loop: render complete, frame_callback_pending unchanged (vsync disabled)"
                        );
                    }
                }
                Err(e) => {
                    consecutive_render_failures += 1;
                    warn!(
                        "Rendering error (attempt {}/{}): {}",
                        consecutive_render_failures, MAX_RENDER_FAILURES, e
                    );

                    if consecutive_render_failures >= MAX_RENDER_FAILURES {
                        loop_error = Some(anyhow::anyhow!(
                            "Too many consecutive render failures ({}), exiting: {}",
                            consecutive_render_failures,
                            e
                        ));
                        break;
                    }

                    // Clear redraw flag to avoid infinite error loop
                    state.input_state.needs_redraw = false;
                }
            }
        } else {
            state.render_layer_toolbars_if_needed();
            if state.input_state.needs_redraw && state.surface.frame_callback_pending() {
                debug!("Main loop: Skipping render - frame callback already pending");
            }
        }
    }

    info!("Wayland backend exiting");

    if let Some(options) = state.session_options()
        && let Some(snapshot) = session::snapshot_from_input(&state.input_state, options)
        && let Err(err) = session::save_snapshot(&snapshot, options)
    {
        warn!("Failed to save session state: {}", err);
        notification::send_notification_async(
            &state.tokio_handle,
            "Failed to Save Session".to_string(),
            format!("Your drawings may not persist: {}", err),
            Some("dialog-error".to_string()),
        );
    }

    EventLoopOutcome { loop_error }
}
