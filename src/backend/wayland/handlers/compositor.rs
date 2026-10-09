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
        self.refresh_freeze_zoom_geometry();
        self.buffer_damage
            .mark_all_full(FullDamageReason::ScaleChanged);
        let (phys_w, phys_h) = self.surface.physical_dimensions();
        self.frozen
            .handle_resize(phys_w, phys_h, &mut self.input_state);
        self.zoom
            .handle_resize(phys_w, phys_h, &mut self.input_state);
        self.cancel_screen_modals_if_source_changed();
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
        self.refresh_freeze_zoom_geometry();
        self.cancel_screen_modals_if_source_changed();
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

        let previous_output = self.surface.current_output();
        self.surface.enter_output(output.clone());
        self.refresh_surface_output(previous_output.as_ref(), None);
        self.log_capture_output_event(
            super::output::CaptureOutputEvent::SurfaceEnter,
            output,
            previous_output.as_ref(),
        );

        // If freeze-on-start was requested, trigger it once the surface is configured and active.
        if self.frozen.take_pending_on_start() {
            info!("Applying freeze-on-start after initial configure");
            self.input_state.request_frozen_toggle();
        }
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

        let previous_output = self.surface.current_output();
        self.surface.clear_output(output);
        self.refresh_surface_output(previous_output.as_ref(), None);
        self.log_capture_output_event(
            super::output::CaptureOutputEvent::SurfaceLeave,
            output,
            previous_output.as_ref(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::frozen::FrozenImage;
    use crate::backend::wayland::handlers::test_support::HandlerFixture;

    #[test]
    fn reenter_and_neighbour_leave_preserve_installed_freeze_and_zoom() {
        let mut fixture = HandlerFixture::with_outputs(crate::config::Config::default(), 2);
        let outputs: Vec<_> = fixture.state.protocol.output().outputs().collect();
        let qh = fixture.queue.handle();
        let surface = fixture.state.surface.wl_surface().unwrap().clone();
        let state = &mut fixture.state;
        state.surface_enter(&fixture.conn, &qh, &surface, &outputs[0]);
        state.frozen.set_image(FrozenImage {
            width: 1,
            height: 1,
            stride: 4,
            data: vec![7; 4],
        });
        state.input_state.set_frozen_active(true);
        state.zoom.activate_without_capture();
        state.toolbar_chrome.set_needs_recreate(false);

        state.surface_enter(&fixture.conn, &qh, &surface, &outputs[0]);
        state.surface_enter(&fixture.conn, &qh, &surface, &outputs[1]);
        state.surface_leave(&fixture.conn, &qh, &surface, &outputs[1]);

        assert_eq!(state.surface.current_output(), Some(outputs[0].clone()));
        assert_eq!(state.frozen.image().unwrap().data, vec![7; 4]);
        assert!(state.input_state.frozen_active());
        assert!(state.zoom.active);
        assert!(!state.toolbar_chrome.needs_recreate());

        state.surface_leave(&fixture.conn, &qh, &surface, &outputs[0]);

        assert!(state.surface.current_output().is_none());
        assert!(state.frozen.image().is_none());
        assert!(!state.input_state.frozen_active());
        assert!(!state.zoom.active);
    }
}
