// Handles compositor callbacks (frame pacing, surface enter/leave) so the backend
// can throttle rendering; invoked by smithay through the delegate in `mod.rs`.
use log::{debug, info, warn};
use smithay_client_toolkit::compositor::CompositorHandler;
use wayland_client::{
    Connection, QueueHandle,
    protocol::{wl_output, wl_surface},
};

use super::super::state::WaylandState;
use crate::session;

impl CompositorHandler for WaylandState {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        debug!("Scale factor changed");
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
        debug!("Transform changed");
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        time: u32,
    ) {
        debug!(
            "Frame callback received (time: {}ms), clearing frame_callback_pending",
            time
        );
        self.frame_callback_pending = false;

        if self.input_state.needs_redraw {
            debug!(
                "Frame callback: needs_redraw is still true, will render on next loop iteration"
            );
        }
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        debug!("Surface entered output");

        let identity = self.output_identity_for(output);

        let mut load_result = None;
        let already_loaded = self.session_loaded;
        let mut load_requested = false;
        if let Some(options) = self.session_options_mut() {
            let changed = options.set_output_identity(identity.as_deref());

            if changed {
                if let Some(id) = options.output_identity() {
                    info!("Persisting session using monitor identity '{}'.", id);
                }
            }

            if changed || !already_loaded {
                load_result = Some(session::load_snapshot(options));
                load_requested = true;
            }
        }

        if let Some(result) = load_result {
            let current_options = self.session_options().cloned();
            match result {
                Ok(Some(snapshot)) => {
                    if let Some(ref options) = current_options {
                        debug!(
                            "Restoring session from {}",
                            options.session_file_path().display()
                        );
                        session::apply_snapshot(&mut self.input_state, snapshot, options);
                    }
                }
                Ok(None) => {
                    if let Some(ref options) = current_options {
                        debug!(
                            "No session data found for {}",
                            options.session_file_path().display()
                        );
                    }
                }
                Err(err) => {
                    warn!("Failed to load session state: {}", err);
                }
            }

            if load_requested {
                self.last_loaded_identity = current_options
                    .as_ref()
                    .and_then(|opts| opts.output_identity().map(|s| s.to_string()));
                self.session_loaded = true;
                self.input_state.needs_redraw = true;
            }
        }
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        debug!("Surface left output");
    }
}
