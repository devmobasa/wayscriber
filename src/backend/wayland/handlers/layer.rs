// Responds to layer-shell configure/close events, keeping dimensions in sync with the compositor.
use log::info;
use smithay_client_toolkit::shell::wlr_layer::{
    LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
};
use wayland_client::{Connection, QueueHandle};

use super::super::state::WaylandState;

impl LayerShellHandler for WaylandState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        info!("Layer surface closed by compositor");
        self.input_state.should_exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        info!(
            "Layer surface configured: {}x{}",
            configure.new_size.0, configure.new_size.1
        );

        if configure.new_size.0 > 0 && configure.new_size.1 > 0 {
            let size_changed =
                self.width != configure.new_size.0 || self.height != configure.new_size.1;

            self.width = configure.new_size.0;
            self.height = configure.new_size.1;

            if size_changed && self.pool.is_some() {
                info!("Surface size changed - recreating SlotPool");
                self.pool = None;
            }

            self.input_state
                .update_screen_dimensions(self.width, self.height);
        }

        self.configured = true;
        self.input_state.needs_redraw = true;
    }
}
