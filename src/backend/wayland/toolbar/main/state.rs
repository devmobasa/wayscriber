use smithay_client_toolkit::{compositor::CompositorState, shell::wlr_layer::LayerSurface};
use wayland_client::protocol::wl_surface;

use super::structs::ToolbarSurfaceManager;

impl ToolbarSurfaceManager {
    pub fn top_created(&self) -> bool {
        self.top.layer_surface.is_some()
    }

    /// Returns true if the toolbar is visible
    pub fn is_visible(&self) -> bool {
        self.top_visible
    }

    /// Returns true if the top toolbar is visible
    pub fn is_top_visible(&self) -> bool {
        self.top_visible
    }

    /// Set visibility of the top toolbar
    pub fn set_visible(&mut self, visible: bool) {
        self.top_visible = visible;
        if !visible {
            self.top.destroy();
            self.top_hover = None;
        }
    }

    /// Set visibility of the top toolbar only
    pub fn set_top_visible(&mut self, visible: bool) {
        self.set_visible(visible);
    }

    pub(in crate::backend::wayland) fn wl_surface(&self) -> Option<&wl_surface::WlSurface> {
        self.top.wl_surface()
    }

    /// Whether the pointer (or keyboard focus hover) is currently on the
    /// top strip. Drives the idle-fade restore.
    pub fn top_pointer_present(&self) -> bool {
        self.top_hover.is_some() || self.top.focused_hover().is_some()
    }

    pub fn set_suppressed(&mut self, compositor: &CompositorState, suppressed: bool) {
        if self.suppressed == suppressed {
            return;
        }
        self.suppressed = suppressed;
        self.top.set_suppressed(compositor, suppressed);
    }

    pub fn is_suppressed(&self) -> bool {
        self.suppressed
    }

    /// Apply pending input-region changes (no-op unless a render declared
    /// partial input rects or suppression state changed).
    pub fn apply_input_regions(&mut self, compositor: &CompositorState) {
        self.top.apply_input_region(compositor);
    }

    pub fn set_top_margins(&mut self, top: i32, left: i32) {
        let (_, right, bottom, _) = self.top.margin;
        self.top.set_margins(top, right, bottom, left);
    }

    pub fn destroy_all(&mut self) {
        self.top.destroy();
        self.top_hover = None;
    }

    pub fn is_toolbar_layer(&self, layer: &LayerSurface) -> bool {
        self.top.is_layer(layer)
    }

    pub fn top_configured(&self) -> bool {
        self.top.configured
    }
}
