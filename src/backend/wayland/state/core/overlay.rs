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

    pub(in crate::backend::wayland) fn overlay_keyboard_passthrough_requested(&self) -> bool {
        overlay_keyboard_passthrough_requested_for(
            self.data.overlay_suppression,
            self.data.overlay_suppression_keyboard_policy,
            self.input_state.light_mode_passthrough(),
        )
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
        self.enter_overlay_suppression_with_keyboard_policy(
            reason,
            OverlaySuppressionKeyboardPolicy::Release,
        )
    }

    pub(in crate::backend::wayland) fn enter_overlay_suppression_with_keyboard_policy(
        &mut self,
        reason: OverlaySuppression,
        keyboard_policy: OverlaySuppressionKeyboardPolicy,
    ) -> bool {
        if self.data.overlay_suppression != OverlaySuppression::None {
            log::warn!(
                "capture.preflight component=overlay reason={reason:?} phase=enter-rejected active={:?}",
                self.data.overlay_suppression
            );
            return false;
        }
        self.data.overlay_suppression = reason;
        self.data.overlay_suppression_keyboard_policy = keyboard_policy;
        if reason.requires_capture_barrier() {
            self.data
                .overlay_capture_barrier
                .begin(reason, self.gtk_toolbar.is_some());
        }
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
            log::info!(
                "capture.preflight component=overlay reason={reason:?} phase=exit-ignored active={:?}",
                self.data.overlay_suppression
            );
            return;
        }
        self.data.overlay_capture_barrier.cancel(reason);
        self.data.overlay_suppression = OverlaySuppression::None;
        self.data.overlay_suppression_keyboard_policy = OverlaySuppressionKeyboardPolicy::Release;
        self.sync_overlay_interactivity();
        self.buffer_damage
            .mark_all_full(FullDamageReason::OverlayRestored);
        self.input_state.needs_redraw = true;
        self.toolbar.mark_dirty();
        log::info!("capture.preflight component=overlay reason={reason:?} phase=restored");
    }
}

fn overlay_keyboard_passthrough_requested_for(
    suppression: OverlaySuppression,
    keyboard_policy: OverlaySuppressionKeyboardPolicy,
    light_mode_passthrough: bool,
) -> bool {
    light_mode_passthrough
        || (suppression != OverlaySuppression::None
            && keyboard_policy == OverlaySuppressionKeyboardPolicy::Release)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_zoom_suppression_can_retain_keyboard_focus() {
        assert!(!overlay_keyboard_passthrough_requested_for(
            OverlaySuppression::Zoom,
            OverlaySuppressionKeyboardPolicy::Retain,
            false,
        ));
        assert!(overlay_keyboard_passthrough_requested_for(
            OverlaySuppression::Zoom,
            OverlaySuppressionKeyboardPolicy::Release,
            false,
        ));
        assert!(overlay_keyboard_passthrough_requested_for(
            OverlaySuppression::None,
            OverlaySuppressionKeyboardPolicy::Retain,
            true,
        ));
    }
}
