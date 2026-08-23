use smithay_client_toolkit::shell::{
    WaylandSurface,
    wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
};
use wayland_client::{Connection, Proxy, QueueHandle};

use super::super::PinHost;

impl LayerShellHandler for PinHost {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, layer: &LayerSurface) {
        // SCTK exposes the role's owning wl_surface rather than its private
        // protocol proxy; this route remains unique and O(1).
        let Some(id) = self.by_wl_surface.get(&layer.wl_surface().id()).copied() else {
            log::debug!("Ignoring close for unknown pin layer surface");
            return;
        };
        self.close_pin(id);
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let Some(id) = self.by_wl_surface.get(&layer.wl_surface().id()).copied() else {
            return;
        };
        if let Err(error) = self.accept_configure(id, configure.new_size, _qh) {
            log::error!("Pin {id} configure failed: {error:#}");
            self.close_pin(id);
        }
    }
}
