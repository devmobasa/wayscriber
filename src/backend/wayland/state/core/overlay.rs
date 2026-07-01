use super::super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn overlay_suppressed(&self) -> bool {
        self.data.overlay_suppression != OverlaySuppression::None
    }

    pub(in crate::backend::wayland) fn overlay_blocks_event_loop(&self) -> bool {
        matches!(
            self.data.overlay_suppression,
            OverlaySuppression::Capture
                | OverlaySuppression::DesktopBackdrop
                | OverlaySuppression::ExternalDialog
                | OverlaySuppression::Frozen
                | OverlaySuppression::Zoom
        )
    }

    pub(in crate::backend::wayland) fn capture_suppressed(&self) -> bool {
        matches!(
            self.data.overlay_suppression,
            OverlaySuppression::Capture | OverlaySuppression::DesktopBackdrop
        )
    }

    pub(in crate::backend::wayland) fn overlay_passthrough_requested(&self) -> bool {
        self.overlay_suppressed() || self.input_state.light_mode_passthrough()
    }

    fn set_overlay_clickthrough(&mut self, clickthrough: bool) {
        if self.data.overlay_clickthrough == clickthrough {
            return;
        }
        self.data.overlay_clickthrough = clickthrough;
        if let Some(wl_surface) = self.surface.wl_surface().cloned() {
            set_surface_clickthrough(&self.compositor_state, &wl_surface, clickthrough);
        }
        self.toolbar
            .set_suppressed(&self.compositor_state, clickthrough);
    }

    pub(in crate::backend::wayland) fn sync_overlay_interactivity(&mut self) {
        self.set_overlay_clickthrough(self.overlay_passthrough_requested());
        self.refresh_keyboard_interactivity();
    }

    pub(in crate::backend::wayland) fn force_sync_overlay_interactivity(&mut self) {
        self.data.overlay_clickthrough = !self.overlay_passthrough_requested();
        self.sync_overlay_interactivity();
    }

    pub(in crate::backend::wayland) fn enter_overlay_suppression(
        &mut self,
        reason: OverlaySuppression,
    ) -> bool {
        if self.data.overlay_suppression != OverlaySuppression::None {
            return false;
        }
        self.data.overlay_suppression = reason;
        self.sync_overlay_interactivity();
        self.buffer_damage
            .mark_all_full(FullDamageReason::OverlaySuppression);
        self.input_state.needs_redraw = true;
        self.toolbar.mark_dirty();
        true
    }

    pub(in crate::backend::wayland) fn exit_overlay_suppression(
        &mut self,
        reason: OverlaySuppression,
    ) {
        if self.data.overlay_suppression != reason {
            return;
        }
        self.data.overlay_suppression = OverlaySuppression::None;
        self.sync_overlay_interactivity();
        self.buffer_damage
            .mark_all_full(FullDamageReason::OverlayRestored);
        self.input_state.needs_redraw = true;
        self.toolbar.mark_dirty();
    }
}
