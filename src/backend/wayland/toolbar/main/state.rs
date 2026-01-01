use smithay_client_toolkit::{compositor::CompositorState, shell::wlr_layer::LayerSurface};
use wayland_client::protocol::wl_surface;

use super::structs::ToolbarSurfaceManager;

impl ToolbarSurfaceManager {
    pub fn top_created(&self) -> bool {
        self.top.layer_surface.is_some()
    }

    pub fn side_created(&self) -> bool {
        self.side.layer_surface.is_some()
    }

    /// Returns true if any toolbar is visible
    pub fn is_visible(&self) -> bool {
        self.visible || self.top_visible || self.side_visible
    }

    /// Returns true if the top toolbar is visible
    pub fn is_top_visible(&self) -> bool {
        self.top_visible
    }

    /// Returns true if the side toolbar is visible
    pub fn is_side_visible(&self) -> bool {
        self.side_visible
    }

    /// Set combined visibility (shows/hides both toolbars)
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        if visible {
            self.top_visible = true;
            self.side_visible = true;
        } else {
            self.top_visible = false;
            self.side_visible = false;
            self.top.destroy();
            self.side.destroy();
            self.top_hover = None;
            self.side_hover = None;
        }
    }

    /// Set visibility of the top toolbar only
    pub fn set_top_visible(&mut self, visible: bool) {
        self.top_visible = visible;
        if !visible {
            self.top.destroy();
            self.top_hover = None;
        }
        // Update combined flag: any toolbar visible keeps overlays alive.
        self.visible = self.top_visible || self.side_visible;
    }

    /// Set visibility of the side toolbar only
    pub fn set_side_visible(&mut self, visible: bool) {
        self.side_visible = visible;
        if !visible {
            self.side.destroy();
            self.side_hover = None;
        }
        // Update combined flag: any toolbar visible keeps overlays alive.
        self.visible = self.top_visible || self.side_visible;
    }

    pub fn is_toolbar_surface(&self, surface: &wl_surface::WlSurface) -> bool {
        self.top.is_surface(surface) || self.side.is_surface(surface)
    }

    pub fn set_suppressed(&mut self, compositor: &CompositorState, suppressed: bool) {
        if self.suppressed == suppressed {
            return;
        }
        self.suppressed = suppressed;
        self.top.set_suppressed(compositor, suppressed);
        self.side.set_suppressed(compositor, suppressed);
    }

    pub fn set_top_margins(&mut self, top: i32, left: i32) {
        let (_, right, bottom, _) = self.top.margin;
        self.top.set_margins(top, right, bottom, left);
    }

    pub fn set_side_margins(&mut self, top: i32, left: i32) {
        let (_, right, bottom, _) = self.side.margin;
        self.side.set_margins(top, right, bottom, left);
    }

    pub fn top_layer_surface(&self) -> Option<&LayerSurface> {
        self.top.layer_surface.as_ref()
    }

    pub fn side_layer_surface(&self) -> Option<&LayerSurface> {
        self.side.layer_surface.as_ref()
    }

    pub fn destroy_all(&mut self) {
        self.top.destroy();
        self.side.destroy();
        self.top_hover = None;
        self.side_hover = None;
    }

    pub fn is_toolbar_layer(&self, layer: &LayerSurface) -> bool {
        self.top.is_layer(layer) || self.side.is_layer(layer)
    }

    pub fn configured_states(&self) -> (bool, bool) {
        (self.top.configured, self.side.configured)
    }
}
