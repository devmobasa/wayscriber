use super::*;
use crate::backend::wayland::toolbar::ToolbarFocusTarget;
use wayland_client::protocol::wl_surface;

impl WaylandState {
    pub(in crate::backend::wayland) fn pointer_over_toolbar(&self) -> bool {
        self.data.pointer_over_toolbar
    }

    pub(in crate::backend::wayland) fn set_pointer_over_toolbar(&mut self, value: bool) {
        self.data.pointer_over_toolbar = value;
    }

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

    pub(in crate::backend::wayland) fn toolbar_needs_recreate(&self) -> bool {
        self.data.toolbar_needs_recreate
    }

    pub(in crate::backend::wayland) fn set_toolbar_needs_recreate(&mut self, value: bool) {
        self.data.toolbar_needs_recreate = value;
    }

    pub(in crate::backend::wayland) fn toolbar_top_offset(&self) -> f64 {
        self.data.toolbar_top_offset
    }

    pub(in crate::backend::wayland) fn toolbar_top_offset_y(&self) -> f64 {
        self.data.toolbar_top_offset_y
    }

    pub(in crate::backend::wayland) fn toolbar_side_offset(&self) -> f64 {
        self.data.toolbar_side_offset
    }

    pub(in crate::backend::wayland) fn toolbar_side_offset_x(&self) -> f64 {
        self.data.toolbar_side_offset_x
    }

    pub(in crate::backend::wayland) fn inline_toolbars_active(&self) -> bool {
        self.data.inline_toolbars
    }

    pub(in crate::backend::wayland) fn inline_toolbars_render_active(&self) -> bool {
        self.inline_toolbars_active() || self.toolbar_drag_preview_active()
    }

    pub(in crate::backend::wayland) fn toolbar_surface_screen_coords(
        &self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
    ) -> Option<(f64, f64)> {
        let target = self.toolbar.focus_target_for_surface(surface)?;
        let kind = match target {
            ToolbarFocusTarget::Top => MoveDragKind::Top,
            ToolbarFocusTarget::Side => MoveDragKind::Side,
        };
        Some(self.local_to_screen_coords(kind, position))
    }
}
