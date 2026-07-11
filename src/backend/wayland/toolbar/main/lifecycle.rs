use log::info;
use smithay_client_toolkit::{
    compositor::CompositorState,
    shell::wlr_layer::{LayerShell, LayerSurfaceConfigure},
};
use wayland_client::{QueueHandle, protocol::wl_output};

use super::structs::ToolbarSurfaceManager;
use crate::backend::wayland::state::WaylandState;
use crate::ui::toolbar::ToolbarSnapshot;

impl ToolbarSurfaceManager {
    pub fn ensure_created(
        &mut self,
        qh: &QueueHandle<WaylandState>,
        compositor: &CompositorState,
        layer_shell: &LayerShell,
        scale: i32,
        output: Option<&wl_output::WlOutput>,
        snapshot: &ToolbarSnapshot,
    ) {
        let top_size = crate::backend::wayland::toolbar::top_size(snapshot);
        let side_size = crate::backend::wayland::toolbar::side_size(snapshot);

        if self.is_top_visible() {
            if self.top.layer_surface.is_none() {
                info!(
                    "Ensuring top toolbar surface exists at logical size {:?}, scale {}",
                    top_size, scale
                );
                self.top.set_logical_size(top_size);
            } else if self.top.logical_size != top_size {
                // Resize the mapped surface in place — destroying and
                // recreating it made every size change flicker.
                self.top.resize(top_size);
            }
            self.top
                .ensure_created(qh, compositor, layer_shell, scale, output);
        }

        if self.is_side_visible() {
            if self.side.layer_surface.is_none() {
                info!(
                    "Ensuring side toolbar surface exists at logical size {:?}, scale {}",
                    side_size, scale
                );
                self.side.set_logical_size(side_size);
            } else if self.side.logical_size != side_size {
                self.side.resize(side_size);
            }
            self.side
                .ensure_created(qh, compositor, layer_shell, scale, output);
        }

        if self.suppressed {
            self.top.set_suppressed(compositor, true);
            self.side.set_suppressed(compositor, true);
        }
    }

    pub fn handle_configure(
        &mut self,
        configure: &LayerSurfaceConfigure,
        layer: &smithay_client_toolkit::shell::wlr_layer::LayerSurface,
    ) -> bool {
        if self.top.is_layer(layer) {
            return self.top.handle_configure(configure);
        }
        if self.side.is_layer(layer) {
            return self.side.handle_configure(configure);
        }
        false
    }

    pub fn maybe_update_scale(&mut self, output: Option<&wl_output::WlOutput>, scale: i32) {
        self.top.maybe_update_scale(output, scale);
        self.side.maybe_update_scale(output, scale);
    }
}
