use log::{debug, info};
use smithay_client_toolkit::{
    compositor::CompositorState,
    shell::{
        WaylandSurface,
        wlr_layer::{KeyboardInteractivity, Layer, LayerShell, LayerSurfaceConfigure},
    },
};
use wayland_client::{QueueHandle, protocol::wl_output};

use super::structs::ToolbarSurface;
use crate::backend::wayland::state::WaylandState;

impl ToolbarSurface {
    pub fn ensure_created(
        &mut self,
        qh: &QueueHandle<WaylandState>,
        compositor: &CompositorState,
        layer_shell: &LayerShell,
        scale: i32,
        output: Option<&wl_output::WlOutput>,
    ) {
        if self.layer_surface.is_some() {
            return;
        }

        info!(
            "Creating toolbar surface '{}' (anchor {:?}, logical {:?}, margin {:?}, scale {})",
            self.name, self.anchor, self.logical_size, self.margin, scale
        );

        let wl_surface = compositor.create_surface(qh);
        wl_surface.set_buffer_scale(scale);

        let layer_surface = layer_shell.create_layer_surface(
            qh,
            wl_surface.clone(),
            Layer::Overlay, // map in overlay layer so toolbars can stack above main surface
            Some(self.name),
            output,
        );
        layer_surface.set_anchor(self.anchor);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_margin(self.margin.0, self.margin.1, self.margin.2, self.margin.3);

        if self.logical_size != (0, 0) {
            layer_surface.set_size(self.logical_size.0, self.logical_size.1);
        }

        layer_surface.commit();

        self.wl_surface = Some(wl_surface);
        self.layer_surface = Some(layer_surface);
        self.scale = scale.max(1);
        self.dirty = true;
        self.configured = false;
    }

    pub fn destroy(&mut self) {
        self.layer_surface = None;
        self.wl_surface = None;
        self.pool = None;
        self.width = 0;
        self.height = 0;
        self.configured = false;
        self.dirty = false;
        self.suppressed = false;
        self.hit_regions.clear();
        self.hover = None;
        self.focus_index = None;
    }

    pub fn handle_configure(&mut self, configure: &LayerSurfaceConfigure) -> bool {
        if self.layer_surface.is_none() {
            return false;
        }

        if configure.new_size.0 > 0 && configure.new_size.1 > 0 {
            let changed = self.width != configure.new_size.0 || self.height != configure.new_size.1;
            self.width = configure.new_size.0;
            self.height = configure.new_size.1;
            debug!(
                "Toolbar surface '{}' configured to {}x{} (scale {})",
                self.name, self.width, self.height, self.scale
            );
            if changed {
                self.pool = None;
            }
        }

        self.configured = true;
        self.dirty = true;
        true
    }
}
