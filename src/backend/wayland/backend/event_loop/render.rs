use log::{debug, warn};

use super::super::super::state::WaylandState;

const MAX_RENDER_FAILURES: u32 = 10;

pub(super) fn maybe_render(
    state: &mut WaylandState,
    qh: &wayland_client::QueueHandle<WaylandState>,
    consecutive_render_failures: &mut u32,
) -> Option<anyhow::Error> {
    // Render if configured and needs redraw, but only if no frame callback pending.
    // This throttles rendering to display refresh rate (when vsync is enabled).
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
                // Reset failure counter on successful render.
                *consecutive_render_failures = 0;
                state.input_state.needs_redraw =
                    keep_rendering || state.input_state.has_pending_history();
                // Only set frame_callback_pending if vsync is enabled.
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
                *consecutive_render_failures += 1;
                warn!(
                    "Rendering error (attempt {}/{}): {}",
                    *consecutive_render_failures, MAX_RENDER_FAILURES, e
                );

                if *consecutive_render_failures >= MAX_RENDER_FAILURES {
                    return Some(anyhow::anyhow!(
                        "Too many consecutive render failures ({}), exiting: {}",
                        *consecutive_render_failures,
                        e
                    ));
                }

                // Clear redraw flag to avoid infinite error loop.
                state.input_state.needs_redraw = false;
            }
        }
    } else {
        state.render_layer_toolbars_if_needed();
        if state.input_state.needs_redraw && state.surface.frame_callback_pending() {
            debug!("Main loop: Skipping render - frame callback already pending");
        }
    }

    None
}
