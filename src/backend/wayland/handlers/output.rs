// Tracks monitor hotplug/updates so `WaylandState` can respond to geometry changes.
use log::debug;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use wayland_client::{Connection, QueueHandle, protocol::wl_output};

use super::super::state::WaylandState;

impl OutputHandler for WaylandState {
    fn output_state(&mut self) -> &mut OutputState {
        self.protocol.output_mut()
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        debug!("New output detected");
        self.refresh_active_output_label();
        self.refresh_freeze_zoom_geometry();
        self.cancel_screen_modals_if_source_changed();
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
        // Screenshot origin walks every output, so a non-active monitor that
        // is added, moved, scaled, or given logical geometry still has to
        // refresh the active crop.
        self.refresh_freeze_zoom_geometry();
        self.cancel_screen_modals_if_source_changed();
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        debug!("Output destroyed");
        self.surface.clear_output(&output);
        self.refresh_active_output_label();
        // SCTK 0.20 calls this before removing the output from OutputState, so
        // a walk of current outputs would still include it. Exclude it here;
        // there is no later callback after the removal.
        self.refresh_freeze_zoom_geometry_excluding(Some(&output));
        self.cancel_screen_modals_if_source_changed();
    }
}
