use super::*;
use wayland_client::protocol::wl_surface;

impl WaylandState {
    pub(in crate::backend::wayland) fn toolbar_dragging(&self) -> bool {
        self.data.toolbar_dragging
    }

    pub(in crate::backend::wayland) fn set_toolbar_dragging(&mut self, value: bool) {
        self.data.toolbar_dragging = value;
    }

    pub(in crate::backend::wayland) fn toolbar_drag_preview_active(&self) -> bool {
        self.data.toolbar_drag_preview
    }

    pub(in crate::backend::wayland) fn set_toolbar_drag_preview_active(&mut self, value: bool) {
        self.data.toolbar_drag_preview = value;
    }

    pub(in crate::backend::wayland) fn request_toolbar_drag_flush(&mut self) {
        self.data.toolbar_drag_flush_requested = true;
    }

    pub(in crate::backend::wayland) fn take_toolbar_drag_flush_requested(&mut self) -> bool {
        let requested = self.data.toolbar_drag_flush_requested;
        self.data.toolbar_drag_flush_requested = false;
        requested
    }

    pub(in crate::backend::wayland) fn inline_toolbars_render_active(&self) -> bool {
        self.toolbar_chrome.inline_toolbars()
            || self.toolbar_drag_preview_active()
            || self.data.gtk_drag_preview.is_some()
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
