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
        let scale = self.surface.scale();
        self.toolbar.maybe_update_scale(self.surface.current_output().as_ref(), scale);
        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;
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
        self.surface.set_frame_callback_pending(false);

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

        self.surface.set_current_output(output.clone());

        if let Some(info) = self.output_state.info(output) {
            let scale = info.scale_factor.max(1);
            self.surface.set_scale(scale);
            self.toolbar.maybe_update_scale(Some(output), scale);
            self.toolbar.mark_dirty();
            let (logical_w, logical_h) = info
                .logical_size
                .unwrap_or((self.surface.width() as i32, self.surface.height() as i32));
            let (logical_x, logical_y) = info.logical_position.unwrap_or((0, 0));
            self.frozen.set_active_geometry(Some(
                crate::backend::wayland::frozen_geometry::OutputGeometry {
                    logical_x,
                    logical_y,
                    logical_width: logical_w.max(0) as u32,
                    logical_height: logical_h.max(0) as u32,
                    scale,
                },
            ));
            self.frozen
                .set_active_output(Some(output.clone()), Some(info.id));
        }
        self.frozen.unfreeze(&mut self.input_state);

        // Update frozen buffer dimensions in case this output's scale differs
        let (phys_w, phys_h) = self.surface.physical_dimensions();
        self.frozen
            .handle_resize(phys_w, phys_h, &mut self.input_state);

        // If freeze-on-start was requested, trigger it once the surface is configured and active.
        if self.pending_freeze_on_start {
            info!("Applying freeze-on-start after initial configure");
            self.pending_freeze_on_start = false;
            self.input_state.request_frozen_toggle();
        }

        let identity = self.output_identity_for(output);

        let mut load_result = None;
        let already_loaded = self.session.is_loaded();
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
                self.session.mark_loaded();
                self.input_state.needs_redraw = true;
            }
        }
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        debug!("Surface left output");
        self.surface.clear_output(output);
        self.frozen.set_active_output(None, None);
        self.frozen.set_active_geometry(None);
        self.frozen.unfreeze(&mut self.input_state);
    }
}
