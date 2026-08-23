use super::*;
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

    pub(in crate::backend::wayland) fn request_toolbar_drag_flush(&mut self) {
        self.data.toolbar_drag_flush_requested = true;
    }

    pub(in crate::backend::wayland) fn take_toolbar_drag_flush_requested(&mut self) -> bool {
        let requested = self.data.toolbar_drag_flush_requested;
        self.data.toolbar_drag_flush_requested = false;
        requested
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

    pub(in crate::backend::wayland) fn restore_toolbar_offsets(&mut self, top: (f64, f64)) {
        self.data.toolbar_top_offset = top.0;
        self.data.toolbar_top_offset_y = top.1;
    }

    pub(in crate::backend::wayland) fn inline_toolbars_active(&self) -> bool {
        self.data.inline_toolbars
    }

    pub(in crate::backend::wayland) fn inline_toolbars_render_active(&self) -> bool {
        self.inline_toolbars_active()
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

    pub(in crate::backend::wayland) fn suppress_next_release_from(
        &mut self,
        source: crate::input::state::RegionInputSource,
    ) {
        self.data.release_suppression.arm(source);
    }

    pub(in crate::backend::wayland) fn clear_suppressed_release_from(
        &mut self,
        source: crate::input::state::RegionInputSource,
    ) {
        self.data.release_suppression.clear(source);
    }

    pub(in crate::backend::wayland) fn take_suppressed_release_from(
        &mut self,
        source: crate::input::state::RegionInputSource,
    ) -> bool {
        self.data.release_suppression.take(source)
    }

    pub(in crate::backend::wayland) fn set_pending_toast_press(
        &mut self,
        value: Option<crate::input::state::ToastPress>,
    ) {
        self.data.pending_toast_press = value;
    }

    pub(in crate::backend::wayland) fn take_pending_toast_press(
        &mut self,
    ) -> Option<crate::input::state::ToastPress> {
        self.data.pending_toast_press.take()
    }

    pub(in crate::backend::wayland) fn set_pending_status_hud_press(&mut self, value: bool) {
        self.data.pending_status_hud_press = value;
    }

    pub(in crate::backend::wayland) fn take_pending_status_hud_press(&mut self) -> bool {
        let value = self.data.pending_status_hud_press;
        self.data.pending_status_hud_press = false;
        value
    }

    pub(in crate::backend::wayland) fn set_pending_zoom_chip_press(
        &mut self,
        value: crate::ui::ZoomChipPress,
    ) {
        self.data.pending_zoom_chip_press = value;
    }

    pub(in crate::backend::wayland) fn take_pending_zoom_chip_press(
        &mut self,
    ) -> crate::ui::ZoomChipPress {
        std::mem::replace(
            &mut self.data.pending_zoom_chip_press,
            crate::ui::ZoomChipPress::None,
        )
    }
}
