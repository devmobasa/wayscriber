use super::super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn overlay_suppressed(&self) -> bool {
        self.data.overlay_suppression != OverlaySuppression::None
    }

    pub(in crate::backend::wayland) fn capture_suppressed(&self) -> bool {
        self.data.overlay_suppression == OverlaySuppression::Capture
    }

    fn apply_overlay_clickthrough(&mut self, clickthrough: bool) {
        if let Some(wl_surface) = self.surface.wl_surface().cloned() {
            set_surface_clickthrough(&self.compositor_state, &wl_surface, clickthrough);
        }
        self.toolbar
            .set_suppressed(&self.compositor_state, clickthrough);
    }

    pub(in crate::backend::wayland) fn enter_overlay_suppression(
        &mut self,
        reason: OverlaySuppression,
    ) {
        if self.data.overlay_suppression != OverlaySuppression::None {
            return;
        }
        self.data.overlay_suppression = reason;
        self.apply_overlay_clickthrough(true);
        if let Some(layer) = self.surface.layer_surface_mut() {
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            self.set_current_keyboard_interactivity(Some(KeyboardInteractivity::None));
        }
        self.input_state.needs_redraw = true;
        self.toolbar.mark_dirty();
    }

    pub(in crate::backend::wayland) fn exit_overlay_suppression(
        &mut self,
        reason: OverlaySuppression,
    ) {
        if self.data.overlay_suppression != reason {
            return;
        }
        self.data.overlay_suppression = OverlaySuppression::None;
        self.apply_overlay_clickthrough(false);
        self.refresh_keyboard_interactivity();
        self.input_state.needs_redraw = true;
        self.toolbar.mark_dirty();
    }
}
