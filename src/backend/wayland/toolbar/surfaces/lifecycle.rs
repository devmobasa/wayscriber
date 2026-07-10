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

    /// Resize an already-mapped surface in place via layer_surface.set_size
    /// instead of destroy/recreate, so size changes (shape picker, drawer,
    /// contextual sections) don't flicker. Rendering pauses until the
    /// compositor acks with a new configure.
    pub fn resize(&mut self, logical: (u32, u32)) {
        if self.logical_size == logical {
            return;
        }
        self.logical_size = logical;
        let Some(layer) = self.layer_surface.as_ref() else {
            return;
        };
        info!(
            "Resizing toolbar surface '{}' in place to logical {:?}",
            self.name, logical
        );
        layer.set_size(logical.0, logical.1);
        layer.wl_surface().commit();
        // Wait for the configure that carries the new size before drawing;
        // needs_render() stays false until handle_configure runs.
        self.configured = false;
        self.dirty = true;
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
        // Compositor-owned regions do not survive the wl_surface. Keep the
        // logical rect cache, but force it onto the next surface even when
        // the next render computes the same rectangles.
        self.input_region_dirty = true;
        self.hit_regions.clear();
        self.hover = None;
        self.focus_index = None;
        self.focus_id = None;
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

#[cfg(test)]
mod tests {
    use super::*;
    use smithay_client_toolkit::shell::wlr_layer::Anchor;

    #[test]
    fn destroy_invalidates_cached_input_region() {
        let mut surface = ToolbarSurface::new("test", Anchor::TOP, (0, 0, 0, 0));
        surface.input_rects = Some(vec![(0.0, 0.0, 100.0, 40.0), (20.0, 48.0, 60.0, 30.0)]);
        surface.input_region_dirty = false;

        surface.destroy();

        assert!(surface.input_region_dirty);
    }
}
