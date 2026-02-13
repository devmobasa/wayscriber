use std::time::{Duration, Instant};

use log::{debug, warn};

use super::super::super::state::WaylandState;

const MAX_RENDER_FAILURES: u32 = 10;

/// Calculates minimum frame time from FPS cap. Returns None if unlimited (0 FPS).
fn min_frame_time_from_fps(max_fps: u32) -> Option<Duration> {
    if max_fps == 0 {
        None
    } else {
        Some(Duration::from_micros(1_000_000 / max_fps as u64))
    }
}

/// Returns the remaining time until the next frame is allowed when VSync is disabled.
/// Returns None if the cap is disabled, there was no previous render, or the frame is ready.
pub(super) fn frame_rate_cap_timeout(
    max_fps: u32,
    last_render_time: Option<Instant>,
) -> Option<Duration> {
    let min_frame_time = min_frame_time_from_fps(max_fps)?;
    let last = last_render_time?;
    let elapsed = last.elapsed();
    if elapsed >= min_frame_time {
        None // Ready to render now
    } else {
        Some(min_frame_time - elapsed)
    }
}

fn handle_render_failure(
    consecutive_render_failures: &mut u32,
    needs_redraw: &mut bool,
    err: &anyhow::Error,
) -> Option<anyhow::Error> {
    *consecutive_render_failures += 1;
    warn!(
        "Rendering error (attempt {}/{}): {}",
        *consecutive_render_failures, MAX_RENDER_FAILURES, err
    );

    if *consecutive_render_failures >= MAX_RENDER_FAILURES {
        return Some(anyhow::anyhow!(
            "Too many consecutive render failures ({}), exiting: {}",
            *consecutive_render_failures,
            err
        ));
    }

    // Clear redraw flag to avoid infinite error loop.
    *needs_redraw = false;
    None
}

pub(super) fn maybe_render(
    state: &mut WaylandState,
    qh: &wayland_client::QueueHandle<WaylandState>,
    consecutive_render_failures: &mut u32,
    last_render_time: &mut Option<Instant>,
) -> Option<anyhow::Error> {
    // Render if configured and needs redraw, but only if no frame callback pending.
    // This throttles rendering to display refresh rate (when vsync is enabled).
    // When VSync is disabled, enforce a minimum frame time to prevent CPU spinning.
    let vsync_enabled = state.config.performance.enable_vsync;
    let frame_time_ok = if vsync_enabled {
        // VSync uses frame callbacks for throttling
        !state.surface.frame_callback_pending()
    } else {
        // Without VSync, enforce configurable frame rate cap (0 = unlimited)
        let min_frame_time = min_frame_time_from_fps(state.config.performance.max_fps_no_vsync);
        match (*last_render_time, min_frame_time) {
            (Some(t), Some(min)) => t.elapsed() >= min,
            _ => true, // No previous render or unlimited FPS
        }
    };
    let can_render =
        state.surface.is_configured() && state.input_state.needs_redraw && frame_time_ok;

    if can_render {
        debug!(
            "Main loop: needs_redraw=true, frame_callback_pending={}, triggering render",
            state.surface.frame_callback_pending()
        );
        let render_start = Instant::now();
        match state.render(qh) {
            Ok(keep_rendering) => {
                let render_duration = render_start.elapsed();
                if render_duration > Duration::from_millis(5) {
                    debug!("Render took {:?}", render_duration);
                }

                // Reset failure counter and record render time.
                *consecutive_render_failures = 0;
                *last_render_time = Some(Instant::now());
                state.input_state.needs_redraw =
                    keep_rendering || state.input_state.has_pending_history();
                // Only set frame_callback_pending if vsync is enabled.
                if vsync_enabled {
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
                if let Some(err) = handle_render_failure(
                    consecutive_render_failures,
                    &mut state.input_state.needs_redraw,
                    &e,
                ) {
                    return Some(err);
                }
            }
        }
    } else {
        state.render_layer_toolbars_if_needed();
        if state.input_state.needs_redraw {
            if vsync_enabled && state.surface.frame_callback_pending() {
                debug!("Main loop: Skipping render - frame callback already pending");
            } else if !vsync_enabled && !frame_time_ok {
                debug!(
                    "Main loop: Skipping render - frame rate cap ({} FPS)",
                    state.config.performance.max_fps_no_vsync
                );
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_rate_cap_timeout_returns_none_when_unlimited_or_missing_last_frame() {
        assert_eq!(frame_rate_cap_timeout(0, None), None);
        assert_eq!(frame_rate_cap_timeout(60, None), None);
    }

    #[test]
    fn frame_rate_cap_timeout_returns_remaining_budget_when_called_too_soon() {
        let timeout = frame_rate_cap_timeout(60, Some(Instant::now())).expect("timeout");
        assert!(timeout > Duration::ZERO);
        assert!(timeout <= Duration::from_millis(17));
    }

    #[test]
    fn frame_rate_cap_timeout_returns_none_when_budget_elapsed() {
        let last = Instant::now() - Duration::from_millis(20);
        assert_eq!(frame_rate_cap_timeout(60, Some(last)), None);
    }

    #[test]
    fn handle_render_failure_increments_counter_and_clears_redraw() {
        let mut failures = 0;
        let mut needs_redraw = true;
        let err = anyhow::anyhow!("render failed");

        let fatal = handle_render_failure(&mut failures, &mut needs_redraw, &err);

        assert!(fatal.is_none());
        assert_eq!(failures, 1);
        assert!(!needs_redraw);
    }

    #[test]
    fn handle_render_failure_returns_fatal_error_at_limit() {
        let mut failures = MAX_RENDER_FAILURES - 1;
        let mut needs_redraw = true;
        let err = anyhow::anyhow!("render failed");

        let fatal = handle_render_failure(&mut failures, &mut needs_redraw, &err)
            .expect("should fail at limit");

        assert_eq!(failures, MAX_RENDER_FAILURES);
        assert!(
            fatal
                .to_string()
                .contains("Too many consecutive render failures")
        );
        assert!(needs_redraw);
    }
}
