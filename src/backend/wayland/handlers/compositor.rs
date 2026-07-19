// Handles compositor callbacks (frame pacing, surface enter/leave) so the backend
// can throttle rendering; invoked by smithay through the delegate in `mod.rs`.
use log::{debug, info};
use smithay_client_toolkit::compositor::CompositorHandler;
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    protocol::{wl_callback, wl_output, wl_surface},
};

use super::super::state::{FullDamageReason, WaylandState};
use super::super::surface::MainSurfaceFrameCallback;

impl Dispatch<wl_callback::WlCallback, MainSurfaceFrameCallback> for WaylandState {
    fn event(
        state: &mut Self,
        _callback: &wl_callback::WlCallback,
        event: wl_callback::Event,
        data: &MainSurfaceFrameCallback,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_callback::Event::Done { callback_data } = event else {
            unreachable!("wl_callback has no event other than done");
        };
        if !state.surface.is_surface(&data.surface) {
            return;
        }

        let cleared_throttle = state.surface.complete_frame_callback(data.token);
        debug!(
            "Frame callback received (time: {callback_data}ms, token: {}, current: {cleared_throttle})",
            data.token
        );
        if let Some(generation) = data.capture_generation {
            state.mark_overlay_capture_frame_ready(generation, qh);
        }
    }
}

impl CompositorHandler for WaylandState {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        if !self.surface.is_surface(surface) {
            return;
        }

        let scale = new_factor.max(1);
        debug!("Scale factor changed to {}", scale);
        self.surface.set_scale(scale);
        self.buffer_damage
            .mark_all_full(FullDamageReason::ScaleChanged);
        let (phys_w, phys_h) = self.surface.physical_dimensions();
        self.frozen
            .handle_resize(phys_w, phys_h, &mut self.input_state);
        self.zoom
            .handle_resize(phys_w, phys_h, &mut self.input_state);
        self.cancel_eyedropper_if_source_missing();
        self.toolbar
            .maybe_update_scale(self.surface.current_output().as_ref(), scale);
        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
        if !self.surface.is_surface(surface) {
            return;
        }

        debug!("Transform changed");
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        time: u32,
    ) {
        if !self.surface.is_surface(surface) {
            return;
        }

        debug!("Legacy untagged frame callback received (time: {time}ms)");

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
        surface: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        if !self.surface.is_surface(surface) {
            return;
        }

        debug!("Surface entered output");

        let previous_output = self.surface.current_output();
        let output_changed = previous_output.as_ref() != Some(output);
        self.surface.set_current_output(output.clone());
        self.set_has_seen_surface_enter(true);
        if output_changed {
            // Keep layer-shell toolbars pinned to the monitor that owns the drawing surface.
            self.set_toolbar_needs_recreate(true);
        }
        self.refresh_active_output_label();

        if let Some(info) = self.output_state.info(output) {
            let scale = info.scale_factor.max(1);
            self.surface.set_scale(scale);
            // Mark full damage when entering output - scale may have changed, pool may be new
            self.buffer_damage
                .mark_all_full(FullDamageReason::OutputChanged);
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
                    transform: info.transform,
                },
            ));
            self.frozen
                .set_active_output(Some(output.clone()), Some(info.id));
            self.zoom.set_active_geometry(Some(
                crate::backend::wayland::frozen_geometry::OutputGeometry {
                    logical_x,
                    logical_y,
                    logical_width: logical_w.max(0) as u32,
                    logical_height: logical_h.max(0) as u32,
                    scale,
                    transform: info.transform,
                },
            ));
            self.zoom
                .set_active_output(Some(output.clone()), Some(info.id));
        }
        self.frozen.unfreeze(&mut self.input_state);
        self.zoom.deactivate(&mut self.input_state);

        // Update frozen buffer dimensions in case this output's scale differs
        let (phys_w, phys_h) = self.surface.physical_dimensions();
        self.frozen
            .handle_resize(phys_w, phys_h, &mut self.input_state);
        self.zoom
            .handle_resize(phys_w, phys_h, &mut self.input_state);
        self.cancel_eyedropper_if_source_missing();

        // If freeze-on-start was requested, trigger it once the surface is configured and active.
        if self.pending_freeze_on_start() {
            info!("Applying freeze-on-start after initial configure");
            self.set_pending_freeze_on_start(false);
            self.input_state.request_frozen_toggle();
        }

        let identity = self.output_identity_for(output);
        self.begin_session_output_transition(identity, "surface output change");
        self.input_state.needs_redraw = true;
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        if !self.surface.is_surface(surface) {
            return;
        }

        debug!("Surface left output");
        self.surface.clear_output(output);
        if self.surface.current_output().is_none() {
            self.set_has_seen_surface_enter(false);
        }
        self.refresh_active_output_label();
        self.frozen.set_active_output(None, None);
        self.frozen.set_active_geometry(None);
        self.frozen.unfreeze(&mut self.input_state);
        self.zoom.set_active_output(None, None);
        self.zoom.set_active_geometry(None);
        self.zoom.deactivate(&mut self.input_state);
    }
}
