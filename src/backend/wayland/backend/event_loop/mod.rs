use log::{info, warn};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
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
) -> EventLoopOutcome {
    // Gracefully exit the overlay when external signals request termination.
    let (exit_flag, tray_action_flag) = setup_signal_handlers();

    // Track consecutive render failures for error recovery.
    let mut consecutive_render_failures = 0u32;

    // Track last render time for frame rate capping when VSync is disabled.
    let mut last_render_time: Option<Instant> = None;

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

        // Calculate timeout for dispatch:
        // - If capture active, not configured, or waiting for VSync: block indefinitely
        // - If VSync disabled and needs_redraw: use frame rate cap timeout
        // - Otherwise: use animation timeout
        let should_block = capture_active
            || !state.surface.is_configured()
            || (vsync_enabled && frame_callback_pending);
        let now = Instant::now();
        let animation_timeout = state.ui_animation_timeout(now);
        let autosave_timeout = session_save::autosave_timeout(state, now);
        let timeout = if should_block {
            autosave_timeout
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
            min_timeout(merged, autosave_timeout)
        } else {
            min_timeout(animation_timeout, autosave_timeout)
        };

        if let Err(e) = dispatch::dispatch_events(event_queue, state, capture_active, timeout) {
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

        if let Err(err) = session_save::autosave_if_due(state, Instant::now()) {
            warn!("Failed to autosave session state: {}", err);
        }

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

    info!("Wayland backend exiting");

    if let Err(err) = session_save::persist_session(state) {
        warn!("Failed to save session state: {}", err);
        session_save::notify_session_failure(state, &err);
    }

    EventLoopOutcome { loop_error }
}
