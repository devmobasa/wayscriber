use super::super::capture::OverlayCaptureBarrier;
use super::super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::backend::wayland) enum OverlaySuppression {
    #[default]
    None,
    Capture,
    DesktopBackdrop,
    ExternalDialog,
    Frozen,
    Zoom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::backend::wayland) enum OverlaySuppressionKeyboardPolicy {
    #[default]
    Release,
    Retain,
}

impl OverlaySuppression {
    pub(in crate::backend::wayland) fn requires_capture_barrier(self) -> bool {
        matches!(
            self,
            Self::Capture | Self::DesktopBackdrop | Self::Frozen | Self::Zoom
        )
    }

    pub(in crate::backend::wayland) fn effective_for_board(
        self,
        board_is_transparent: bool,
    ) -> Self {
        if self == Self::Zoom && !board_is_transparent {
            Self::None
        } else {
            self
        }
    }

    pub(in crate::backend::wayland) fn renders_canvas(self) -> bool {
        !matches!(
            self,
            Self::DesktopBackdrop | Self::ExternalDialog | Self::Frozen | Self::Zoom
        )
    }

    pub(in crate::backend::wayland) fn renders_canvas_transients(self) -> bool {
        self == Self::None
    }

    pub(in crate::backend::wayland) fn renders_ui(self) -> bool {
        self == Self::None
    }
}

#[derive(Debug, Default)]
pub(in crate::backend::wayland) struct OverlaySuppressionState {
    reason: OverlaySuppression,
    keyboard_policy: OverlaySuppressionKeyboardPolicy,
    pub(in crate::backend::wayland::state) barrier: OverlayCaptureBarrier,
    clickthrough: bool,
}

impl OverlaySuppressionState {
    pub(in crate::backend::wayland) fn reason(&self) -> OverlaySuppression {
        self.reason
    }

    pub(in crate::backend::wayland) fn suppressed(&self) -> bool {
        self.reason != OverlaySuppression::None
    }

    pub(in crate::backend::wayland) fn blocks_event_loop(&self) -> bool {
        self.suppressed()
    }

    pub(in crate::backend::wayland) fn capture_suppressed(&self) -> bool {
        matches!(
            self.reason,
            OverlaySuppression::Capture | OverlaySuppression::DesktopBackdrop
        )
    }

    pub(in crate::backend::wayland) fn requires_capture_barrier(&self) -> bool {
        self.reason.requires_capture_barrier()
    }

    pub(in crate::backend::wayland) fn enter(
        &mut self,
        reason: OverlaySuppression,
        keyboard_policy: OverlaySuppressionKeyboardPolicy,
        wait_for_gtk: bool,
    ) -> Result<(), OverlaySuppression> {
        if self.reason != OverlaySuppression::None {
            return Err(self.reason);
        }
        self.reason = reason;
        self.keyboard_policy = keyboard_policy;
        if reason.requires_capture_barrier() {
            self.barrier.begin(reason, wait_for_gtk);
        }
        Ok(())
    }

    pub(in crate::backend::wayland) fn exit(&mut self, reason: OverlaySuppression) -> bool {
        if self.reason != reason {
            return false;
        }
        self.barrier.cancel(reason);
        self.reason = OverlaySuppression::None;
        self.keyboard_policy = OverlaySuppressionKeyboardPolicy::Release;
        true
    }

    pub(in crate::backend::wayland) fn passthrough_requested(
        &self,
        light_mode_passthrough: bool,
    ) -> bool {
        self.reason != OverlaySuppression::None || light_mode_passthrough
    }

    pub(in crate::backend::wayland) fn keyboard_passthrough_requested(
        &self,
        light_mode_passthrough: bool,
    ) -> bool {
        light_mode_passthrough
            || (self.reason != OverlaySuppression::None
                && self.keyboard_policy == OverlaySuppressionKeyboardPolicy::Release)
    }

    pub(in crate::backend::wayland) fn set_clickthrough(&mut self, value: bool) -> bool {
        if self.clickthrough == value {
            return false;
        }
        self.clickthrough = value;
        true
    }
}

impl WaylandState {
    /// Derived chrome suppression for the native capture picker.
    ///
    /// This never changes the user's toolbar/chrome preferences and never
    /// enters the capture-preflight suppression state. Ending the picker
    /// therefore reveals whatever Focus/Light/overlay suppression still
    /// permits instead of restoring a stale snapshot over it.
    pub(in crate::backend::wayland) fn capture_picker_chrome_suppressed(&self) -> bool {
        capture_picker_chrome_suppressed_for(self.input_state.region_state())
    }

    pub(in crate::backend::wayland) fn overlay_passthrough_requested(&self) -> bool {
        self.suppression
            .passthrough_requested(self.input_state.light_mode_passthrough())
    }

    pub(in crate::backend::wayland) fn overlay_keyboard_passthrough_requested(&self) -> bool {
        self.suppression
            .keyboard_passthrough_requested(self.input_state.light_mode_passthrough())
    }

    fn set_overlay_clickthrough(&mut self, clickthrough: bool) {
        if !self.suppression.set_clickthrough(clickthrough) {
            return;
        }
        if let Some(wl_surface) = self.surface.wl_surface().cloned() {
            set_surface_clickthrough(self.protocol.compositor(), &wl_surface, clickthrough);
        }
        self.toolbar
            .set_suppressed(self.protocol.compositor(), clickthrough);
    }

    pub(in crate::backend::wayland) fn sync_overlay_interactivity(&mut self) {
        self.set_overlay_clickthrough(self.overlay_passthrough_requested());
        self.refresh_keyboard_interactivity();
    }

    pub(in crate::backend::wayland) fn force_sync_overlay_interactivity(&mut self) {
        let desired = self.overlay_passthrough_requested();
        self.suppression.set_clickthrough(!desired);
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
        if let Err(active) =
            self.suppression
                .enter(reason, keyboard_policy, self.gtk_toolbar.is_some())
        {
            log::warn!(
                "capture.preflight component=overlay reason={reason:?} phase=enter-rejected active={active:?}"
            );
            return false;
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
        if !self.suppression.exit(reason) {
            log::info!(
                "capture.preflight component=overlay reason={reason:?} phase=exit-ignored active={:?}",
                self.suppression.reason()
            );
            return;
        }
        self.sync_overlay_interactivity();
        self.buffer_damage
            .mark_all_full(FullDamageReason::OverlayRestored);
        self.input_state.needs_redraw = true;
        self.toolbar.mark_dirty();
        log::info!("capture.preflight component=overlay reason={reason:?} phase=restored");
    }
}

fn capture_picker_chrome_suppressed_for(region: crate::input::state::RegionSelectUiState) -> bool {
    region.is_engaged() && region.purpose().is_some_and(|purpose| purpose.is_capture())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_zoom_suppression_can_retain_keyboard_focus() {
        let mut state = OverlaySuppressionState::default();
        state
            .enter(
                OverlaySuppression::Zoom,
                OverlaySuppressionKeyboardPolicy::Retain,
                false,
            )
            .expect("idle state accepts suppression");
        assert!(!state.keyboard_passthrough_requested(false));

        assert!(state.exit(OverlaySuppression::Zoom));
        state
            .enter(
                OverlaySuppression::Zoom,
                OverlaySuppressionKeyboardPolicy::Release,
                false,
            )
            .expect("finished state accepts suppression");
        assert!(state.keyboard_passthrough_requested(false));
        assert!(OverlaySuppressionState::default().keyboard_passthrough_requested(true));
    }

    #[test]
    fn suppression_rejects_overlap_and_only_matching_exit_clears_it() {
        let mut state = OverlaySuppressionState::default();
        state
            .enter(
                OverlaySuppression::Capture,
                OverlaySuppressionKeyboardPolicy::Release,
                false,
            )
            .expect("first suppression");

        assert_eq!(
            state.enter(
                OverlaySuppression::Frozen,
                OverlaySuppressionKeyboardPolicy::Release,
                false,
            ),
            Err(OverlaySuppression::Capture)
        );
        assert!(!state.exit(OverlaySuppression::Frozen));
        assert_eq!(state.reason(), OverlaySuppression::Capture);
        assert!(state.exit(OverlaySuppression::Capture));
        assert_eq!(state.reason(), OverlaySuppression::None);
    }

    #[test]
    fn clickthrough_reports_only_real_changes() {
        let mut state = OverlaySuppressionState::default();

        assert!(!state.set_clickthrough(false));
        assert!(state.set_clickthrough(true));
        assert!(!state.set_clickthrough(true));
    }

    #[test]
    fn suppression_render_policy_keeps_capture_canvas_only() {
        let capture = OverlaySuppression::Capture.effective_for_board(true);
        assert!(capture.renders_canvas());
        assert!(!capture.renders_ui());
        assert!(!capture.renders_canvas_transients());

        assert!(!OverlaySuppression::DesktopBackdrop.renders_canvas());
        assert!(!OverlaySuppression::ExternalDialog.renders_ui());
        assert_eq!(
            OverlaySuppression::Zoom.effective_for_board(false),
            OverlaySuppression::None
        );
        assert_eq!(
            OverlaySuppression::Zoom.effective_for_board(true),
            OverlaySuppression::Zoom
        );
    }

    #[test]
    fn capture_picker_chrome_suppression_is_derived_without_affecting_ocr() {
        use crate::input::state::{
            RegionInputSource, RegionPurposeTag, RegionSelectUiState, ScreenCaptureSource,
        };

        assert!(!capture_picker_chrome_suppressed_for(
            RegionSelectUiState::Inactive
        ));
        assert!(!capture_picker_chrome_suppressed_for(
            RegionSelectUiState::Armed {
                purpose: RegionPurposeTag::Ocr,
                generation: 1,
            }
        ));
        assert!(!capture_picker_chrome_suppressed_for(
            RegionSelectUiState::Armed {
                purpose: RegionPurposeTag::Measure,
                generation: 1,
            }
        ));
        for purpose in [
            RegionPurposeTag::CaptureDeliver,
            RegionPurposeTag::CaptureInteractive,
        ] {
            assert!(capture_picker_chrome_suppressed_for(
                RegionSelectUiState::PendingCapture {
                    purpose,
                    generation: 2,
                    source: ScreenCaptureSource::Frozen,
                }
            ));
            assert!(capture_picker_chrome_suppressed_for(
                RegionSelectUiState::Armed {
                    purpose,
                    generation: 2,
                }
            ));
            assert!(capture_picker_chrome_suppressed_for(
                RegionSelectUiState::Selecting {
                    purpose,
                    generation: 2,
                    owner: RegionInputSource::Pointer,
                    start: (10.0, 20.0),
                    current: (30.0, 40.0),
                }
            ));
        }
    }
}
