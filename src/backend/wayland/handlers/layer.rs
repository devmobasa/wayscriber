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
            let size_changed = self
                .surface
                .update_dimensions(configure.new_size.0, configure.new_size.1);

            if size_changed {
                info!("Surface size changed - recreating SlotPool");
            }

            self.input_state
                .update_screen_dimensions(self.surface.width(), self.surface.height());
            self.frozen
                .handle_resize(self.surface.width(), self.surface.height(), &mut self.input_state);
        }

        self.surface.set_configured(true);
        self.input_state.needs_redraw = true;

        let (phys_w, phys_h) = self.surface.physical_dimensions();
        self.frozen
            .handle_resize(phys_w, phys_h, &mut self.input_state);
    }
}
