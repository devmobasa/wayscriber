// Tracks monitor hotplug/updates so `WaylandState` can respond to geometry changes.
use log::debug;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use wayland_client::{Connection, QueueHandle, protocol::wl_output};

use super::super::state::WaylandState;

impl OutputHandler for WaylandState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        debug!("New output detected");
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        debug!("Output updated");
        // Refresh geometry only for the active output so fallback cropping stays correct.
        if let Some(info) = self.output_state.info(&_output) {
            if !self.frozen.active_output_matches(info.id)
                && !self.zoom.active_output_matches(info.id)
            {
                return;
            }

            if let Some(geo) = crate::backend::wayland::frozen_geometry::OutputGeometry::update_from(
                info.logical_position,
                info.logical_size,
                (self.surface.width(), self.surface.height()),
                info.scale_factor.max(1),
            ) {
                self.frozen.set_active_geometry(Some(geo.clone()));
                self.frozen
                    .set_active_output(Some(_output.clone()), Some(info.id));
                self.zoom.set_active_geometry(Some(geo));
                self.zoom
                    .set_active_output(Some(_output.clone()), Some(info.id));
            }
        }
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        debug!("Output destroyed");
    }
}
