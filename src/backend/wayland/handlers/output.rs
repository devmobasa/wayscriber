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
        self.refresh_active_output_label();
        self.refresh_freeze_zoom_screenshot_origin();
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        debug!("Output updated");
        if self.surface.current_output().as_ref() == Some(&output) {
            self.refresh_active_output_label();
        }
        if let Some(info) = self.output_state.info(&output)
            && (self.frozen.active_output_matches(info.id)
                || self.zoom.active_output_matches(info.id))
            && let Some(geo) = crate::backend::wayland::frozen_geometry::OutputGeometry::update_from(
                info.logical_position,
                info.logical_size,
                (self.surface.width(), self.surface.height()),
                info.scale_factor.max(1),
                info.transform,
            )
        {
            self.set_freeze_zoom_geometry(Some(geo));
            self.frozen
                .set_active_output(Some(output.clone()), Some(info.id));
            self.zoom
                .set_active_output(Some(output.clone()), Some(info.id));
            return;
        }
        // Screenshot origin walks every output, so a non-active monitor that
        // is added, moved, scaled, or given logical geometry still has to
        // refresh the active crop.
        self.refresh_freeze_zoom_screenshot_origin();
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        debug!("Output destroyed");
        self.surface.clear_output(&output);
        if self.surface.current_output().is_none() {
            self.set_has_seen_surface_enter(false);
        }
        self.refresh_active_output_label();
        // SCTK 0.20 calls this before removing the output from OutputState, so
        // a walk of current outputs would still include it. Exclude it here;
        // there is no later callback after the removal.
        self.refresh_freeze_zoom_screenshot_origin_excluding(Some(&output));
    }
}
