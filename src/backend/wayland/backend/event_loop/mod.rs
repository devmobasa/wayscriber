use crate::notification;
use log::{info, warn};
use std::time::{Duration, Instant};
use wayland_client::{Connection, EventQueue};

use super::super::state::WaylandState;
use super::runtime_wake::RuntimeWakeSource;
use super::signals::{OverlaySignalState, setup_signal_handlers};
use super::tray::process_tray_action;

mod capture;
mod dispatch;
mod render;
pub(in crate::backend::wayland) mod session_save;

pub(super) struct EventLoopOutcome {
    pub(super) loop_error: Option<anyhow::Error>,
}

fn min_timeout(a: Option<Duration>, b: Option<Duration>) -> Option<Duration> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

pub(super) fn run_event_loop(
    conn: &Connection,
    event_queue: &mut EventQueue<WaylandState>,
    qh: &wayland_client::QueueHandle<WaylandState>,
    state: &mut WaylandState,
    runtime_wake: &RuntimeWakeSource,
) -> EventLoopOutcome {
    let mut loop_error: Option<anyhow::Error> = None;
    // Install signal authority before the first durable tray-action scan. An
    // action published before installation is found here; later publications
    // signal the installed listener and wake the shared runtime descriptor.
    let mut signals = match install_then_scan(
        || setup_signal_handlers(runtime_wake.handle()),
        || {
            if process_tray_action(state) {
                state.sync_overlay_interactivity();
            }
        },
    ) {
        Ok(signals) => Some(signals),
        Err(err) => {
            warn!("Failed to register overlay signal handlers: {err}");
            loop_error = Some(anyhow::anyhow!(
                "failed to register overlay signal handlers: {err}"
            ));
            None
        }
    };

    // Track consecutive render failures for error recovery.
    let mut consecutive_render_failures = 0u32;

    // Track last render time for frame rate capping when VSync is disabled.
    let mut last_render_time: Option<Instant> = None;

    // Main event loop.
    while let Some(signal_state) = signals.as_mut() {
        if let Some(failure) = terminal_signal_failure(signal_state, || {
            if process_tray_action(state) {
                state.sync_overlay_interactivity();
            }
        }) {
            loop_error = Some(failure);
            break;
        }
        if signal_state.exit_requested() {
            state.input_state.should_exit = true;
        }

        // Check if we should exit before blocking.
        if state.input_state.should_exit {
            info!("Exit requested, breaking event loop");
            break;
        }

        capture::poll_portal_captures(state);

        let capture_active = state.capture.is_in_progress()
            || state.frozen.is_in_progress()
            || state.zoom.is_in_progress()
            || state.overlay_blocks_event_loop();
        let frame_callback_pending = state.surface.frame_callback_pending();
        let vsync_enabled = state.config.performance.enable_vsync;

        // Calculate timeout for dispatch:
        // - If capture active, not configured, or waiting for VSync: block indefinitely
        // - If VSync disabled and needs_redraw: use frame rate cap timeout
        // - Otherwise: use animation timeout
        let should_block = capture_active
            || !state.surface.is_configured()
            || (vsync_enabled && frame_callback_pending);
        let now = Instant::now();
        let animation_timeout = state.ui_animation_timeout(now);
        let toolbar_handoff_timeout = state.toolbar_drag_handoff_timeout(now);
        let autosave_timeout = session_save::autosave_timeout(state, now);
        let focus_exit_timeout = state.focus_exit_timeout(now);
        let command_palette_repeat_timeout = state.input_state.command_palette_repeat_timeout(now);
        let clipboard_timeout = (state.clipboard_paste_rx.is_some()
            || state.clipboard_publish_rx.is_some())
        .then_some(Duration::from_millis(25));
        let timeout = if should_block {
            min_timeout(
                clipboard_timeout,
                min_timeout(autosave_timeout, focus_exit_timeout),
            )
        } else if !vsync_enabled && state.input_state.needs_redraw {
            // When VSync is off and we need to redraw, wake up when frame budget allows
            let frame_cap_timeout = render::frame_rate_cap_timeout(
                state.config.performance.max_fps_no_vsync,
                last_render_time,
            );
            // Use the shorter of frame cap timeout and animation timeout.
            // If unlimited FPS (None) and no animation, use zero to avoid blocking.
            let merged = match (frame_cap_timeout, animation_timeout) {
                (Some(fc), Some(anim)) => Some(fc.min(anim)),
                (Some(fc), None) => Some(fc),
                (None, _) => Some(Duration::ZERO),
            };
            min_timeout(
                clipboard_timeout,
                min_timeout(merged, min_timeout(autosave_timeout, focus_exit_timeout)),
            )
        } else {
            min_timeout(
                clipboard_timeout,
                min_timeout(
                    animation_timeout,
                    min_timeout(autosave_timeout, focus_exit_timeout),
                ),
            )
        };
        let timeout = min_timeout(timeout, toolbar_handoff_timeout);
        let timeout = min_timeout(timeout, command_palette_repeat_timeout);
        if let Err(e) = dispatch::dispatch_events(
            event_queue,
            state,
            runtime_wake,
            signal_state,
            capture_active,
            timeout,
        ) {
            warn!("Event queue error: {}", e);
            loop_error = Some(e);
            break;
        }

        if !state.input_state.should_exit {
            state.reconcile_live_source_interaction_if_idle(
                "post-dispatch interaction reconciliation",
            );
        }

        state.process_gtk_toolbar(conn, qh);

        // Check immediately after dispatch returns.
        if state.input_state.should_exit {
            let explicit_xdg_close_requested = state.take_xdg_explicit_close_requested();
            if should_defer_xdg_unfocused_exit(
                state.surface.is_xdg_window(),
                !state.xdg_focus_loss_exits_overlay(),
                state.has_keyboard_focus(),
                explicit_xdg_close_requested,
            ) {
                warn!("Exit requested while unfocused in xdg stay mode; keeping overlay open");
                state.input_state.should_exit = false;
            } else {
                info!("Exit requested after dispatch, breaking event loop");
                break;
            }
        }
        if state.surface.is_xdg_window()
            && !state.has_keyboard_focus()
            && state.focus_exit_suppression_expired(Instant::now())
        {
            if state.xdg_focus_loss_exits_overlay() {
                warn!("Keyboard focus not restored after clipboard action; exiting overlay");
                state.clear_focus_exit_suppression();
                notification::send_notification_async(
                    &state.tokio_handle,
                    "Wayscriber lost focus".to_string(),
                    "The desktop could not keep the overlay focused, so Wayscriber closed it."
                        .to_string(),
                    Some("dialog-warning".to_string()),
                );
                state.input_state.should_exit = true;
            } else {
                warn!(
                    "Keyboard focus not restored after clipboard action; keeping overlay open (ui.xdg_focus_loss_behavior=stay)"
                );
                state.clear_focus_exit_suppression();
                state.set_xdg_close_guard_for(Duration::from_millis(2500));
                state.request_xdg_activation(qh);
            }
        }
        // Adjust keyboard interactivity if toolbar visibility changed.
        state.sync_toolbar_visibility(qh);

        if state.finish_toolbar_drag_handoff_if_due(Instant::now()) {
            let _ = conn.flush();
        }

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

        if state
            .input_state
            .tick_command_palette_repeat(Instant::now())
        {
            state.input_state.needs_redraw = true;
        }

        if !capture_active && state.ui_animation_due(std::time::Instant::now()) {
            state.input_state.needs_redraw = true;
        }

        capture::flush_if_capture_active(conn, capture_active);
        capture::handle_pending_actions(state, qh);
        state.sync_overlay_interactivity();
        state.apply_onboarding_hints();

        if let Err(err) = session_save::autosave_if_due(state, Instant::now()) {
            warn!("Failed to autosave session state: {}", err);
        }

        state.push_gtk_toolbar_update();

        if let Some(err) = render::maybe_render(
            state,
            qh,
            &mut consecutive_render_failures,
            &mut last_render_time,
        ) {
            loop_error = Some(err);
            break;
        }
    }

    state.flush_perf_summaries(Instant::now());
    info!("Wayland backend exiting");

    finalize_event_loop(
        &mut loop_error,
        || {
            if let Err(err) = session_save::persist_session(state) {
                warn!("Failed to save session state: {}", err);
                session_save::notify_session_failure(state, &err);
            }
        },
        || match signals.as_mut() {
            Some(signal_state) => signal_state.stop_and_join(),
            None => Ok(()),
        },
    );

    EventLoopOutcome { loop_error }
}

fn install_then_scan<T>(
    install: impl FnOnce() -> std::io::Result<T>,
    scan: impl FnOnce(),
) -> std::io::Result<T> {
    let installed = install()?;
    scan();
    Ok(installed)
}

fn terminal_signal_failure(
    signals: &OverlaySignalState,
    preserve_pending_actions: impl FnOnce(),
) -> Option<anyhow::Error> {
    signals.failure().map(|failure| {
        // Durable actions remain authoritative even if their notification
        // listener has become terminal.
        preserve_pending_actions();
        anyhow::anyhow!("overlay signal listener failed: {failure}")
    })
}

fn finalize_event_loop(
    loop_error: &mut Option<anyhow::Error>,
    persist_final_session: impl FnOnce(),
    stop_signal_listener: impl FnOnce() -> std::io::Result<()>,
) {
    persist_final_session();
    if let Err(err) = stop_signal_listener() {
        if loop_error.is_none() {
            *loop_error = Some(anyhow::anyhow!(
                "overlay signal listener teardown failed: {err}"
            ));
        } else {
            warn!("Overlay signal listener teardown failed: {err}");
        }
    }
}

fn should_defer_xdg_unfocused_exit(
    is_xdg_window: bool,
    stay_mode: bool,
    has_keyboard_focus: bool,
    explicit_xdg_close_requested: bool,
) -> bool {
    is_xdg_window && stay_mode && !has_keyboard_focus && !explicit_xdg_close_requested
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::{finalize_event_loop, install_then_scan, should_defer_xdg_unfocused_exit};

    #[test]
    fn defers_exit_only_for_unfocused_xdg_stay_without_explicit_close() {
        assert!(should_defer_xdg_unfocused_exit(true, true, false, false));
        assert!(!should_defer_xdg_unfocused_exit(true, true, true, false));
        assert!(!should_defer_xdg_unfocused_exit(true, false, false, false));
        assert!(!should_defer_xdg_unfocused_exit(false, true, false, false));
        assert!(!should_defer_xdg_unfocused_exit(true, true, false, true));
    }

    #[test]
    fn listener_is_installed_before_the_startup_action_scan() {
        let calls = RefCell::new(Vec::new());
        let installed = install_then_scan(
            || {
                calls.borrow_mut().push("install");
                Ok(7)
            },
            || calls.borrow_mut().push("scan"),
        )
        .unwrap();

        assert_eq!(installed, 7);
        assert_eq!(calls.into_inner(), ["install", "scan"]);
    }

    #[test]
    fn final_save_precedes_owned_listener_teardown_after_failure() {
        let calls = RefCell::new(Vec::new());
        let mut loop_error = Some(anyhow::anyhow!("listener failed"));

        finalize_event_loop(
            &mut loop_error,
            || calls.borrow_mut().push("final-save"),
            || {
                calls.borrow_mut().push("listener-stop");
                Ok(())
            },
        );

        assert_eq!(calls.into_inner(), ["final-save", "listener-stop"]);
        assert_eq!(loop_error.unwrap().to_string(), "listener failed");
    }
}
