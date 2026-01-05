use log::{info, warn};
use std::sync::atomic::Ordering;
use wayland_client::{Connection, EventQueue};

use super::super::state::WaylandState;
use super::signals::setup_signal_handlers;
use super::tray::process_tray_action;

mod capture;
mod dispatch;
mod render;
mod session_save;

pub(super) struct EventLoopOutcome {
    pub(super) loop_error: Option<anyhow::Error>,
}

pub(super) fn run_event_loop(
    conn: &Connection,
    event_queue: &mut EventQueue<WaylandState>,
    qh: &wayland_client::QueueHandle<WaylandState>,
    state: &mut WaylandState,
) -> EventLoopOutcome {
    // Gracefully exit the overlay when external signals request termination.
    let (exit_flag, tray_action_flag) = setup_signal_handlers();

    // Track consecutive render failures for error recovery.
    let mut consecutive_render_failures = 0u32;

    // Main event loop.
    let mut loop_error: Option<anyhow::Error> = None;
    loop {
        if exit_flag
            .as_ref()
            .map(|flag| flag.load(Ordering::Acquire))
            .unwrap_or(false)
        {
            state.input_state.should_exit = true;
        }

        // Check if we should exit before blocking.
        if state.input_state.should_exit {
            info!("Exit requested, breaking event loop");
            break;
        }

        capture::poll_portal_captures(state);

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

        if let Err(e) =
            dispatch::dispatch_events(event_queue, state, capture_active, animation_timeout)
        {
            warn!("Event queue error: {}", e);
            loop_error = Some(e);
            break;
        }

        // Check immediately after dispatch returns.
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

        if !capture_active && state.ui_animation_due(std::time::Instant::now()) {
            state.input_state.needs_redraw = true;
        }

        capture::flush_if_capture_active(conn, capture_active);
        capture::handle_pending_actions(state);
        state.apply_onboarding_hints();

        if let Some(err) = render::maybe_render(state, qh, &mut consecutive_render_failures) {
            loop_error = Some(err);
            break;
        }
    }

    info!("Wayland backend exiting");

    if let Err(err) = session_save::persist_session(state) {
        warn!("Failed to save session state: {}", err);
        session_save::notify_session_failure(state, &err);
    }

    EventLoopOutcome { loop_error }
}
