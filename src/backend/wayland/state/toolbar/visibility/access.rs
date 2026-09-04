use super::*;
use wayland_client::protocol::wl_surface;

impl WaylandState {
    pub(in crate::backend::wayland) fn inline_toolbars_render_active(&self) -> bool {
        self.toolbar_chrome.inline_toolbars()
            || self.toolbar_drag.preview_active()
            || self.toolbar_drag.gtk_preview_kind().is_some()
    }

    pub(in crate::backend::wayland) fn toolbar_surface_screen_coords(
        &self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
    ) -> Option<(f64, f64)> {
        if !self.toolbar.is_focusable_surface(surface) {
            return None;
        }
        Some(self.local_to_screen_coords(MoveDragKind::Top, position))
    }
}
