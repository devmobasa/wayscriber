use crate::backend::wayland::capture_preflight::{
    CaptureLayoutGenerations, CapturePreflight, CapturePreflightError, PortalLayoutRetry,
};
use std::sync::Arc;
use wayland_client::protocol::wl_output;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::backend::wayland::RuntimeWakeHandle;
use crate::backend::wayland::frozen::{FrozenImage, ScreenImageProvenance};
use crate::backend::wayland::frozen_geometry::OutputGeometry;
use crate::backend::wayland::portal_task::PortalOperation;
use crate::input::InputState;

use super::capture::CaptureSession;
use super::{MIN_ZOOM_SCALE, PortalCaptureResult};

mod source;

pub(in crate::backend::wayland) use source::{
    ZoomCaptureId, ZoomSourceOutcome, ZoomSourceTerminal, ZoomTerminalReport, ZoomWaiter,
    ZoomWaiterOwner, ZoomWaiterRegistry,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum ZoomCaptureBackend {
    WlrScreencopy,
    Portal,
}

/// Zoom state, capture logic, and pan/lock bookkeeping.
pub struct ZoomState {
    pub(super) manager: Option<ZwlrScreencopyManagerV1>,
    pub(super) active_output: Option<wl_output::WlOutput>,
    pub(super) active_output_id: Option<u32>,
    pub(super) active_geometry: Option<OutputGeometry>,
    /// Shared active-output and full-desktop validity generations.
    pub(super) layout_generations: CaptureLayoutGenerations,
    pub(super) capture: Option<CaptureSession>,
    pub(super) image: Option<Arc<FrozenImage>>,
    image_provenance: Option<ScreenImageProvenance>,
    pub(super) image_target_dimensions: Option<(u32, u32)>,
    image_generation: u64,
    pub(super) portal: PortalOperation<PortalCaptureResult>,
    pub(super) runtime_wake: Option<RuntimeWakeHandle>,
    pub(super) preflight: CapturePreflight<ZoomCaptureBackend>,
    pub(super) capture_done: bool,
    pub(super) layout_retry: PortalLayoutRetry,
    next_capture_id: u64,
    current_capture_id: Option<ZoomCaptureId>,
    pub(super) source_terminal: Option<ZoomSourceTerminal>,
    pub(super) pending_activation: bool,
    pub active: bool,
    pub locked: bool,
    pub scale: f64,
    pub view_offset: (f64, f64),
    pub panning: bool,
    pub(super) last_pan_pos: (f64, f64),
}

impl ZoomState {
    #[cfg(test)]
    pub fn new(manager: Option<ZwlrScreencopyManagerV1>) -> Self {
        Self::new_inner(manager, None)
    }

    pub(in crate::backend::wayland) fn new_with_runtime_wake(
        manager: Option<ZwlrScreencopyManagerV1>,
        runtime_wake: RuntimeWakeHandle,
    ) -> Self {
        Self::new_inner(manager, Some(runtime_wake))
    }

    fn new_inner(
        manager: Option<ZwlrScreencopyManagerV1>,
        runtime_wake: Option<RuntimeWakeHandle>,
    ) -> Self {
        Self {
            manager,
            active_output: None,
            active_output_id: None,
            active_geometry: None,
            layout_generations: CaptureLayoutGenerations::default(),
            capture: None,
            image: None,
            image_provenance: None,
            image_target_dimensions: None,
            image_generation: 0,
            portal: PortalOperation::default(),
            runtime_wake,
            preflight: CapturePreflight::Idle,
            capture_done: false,
            layout_retry: PortalLayoutRetry::default(),
            next_capture_id: 1,
            current_capture_id: None,
            source_terminal: None,
            pending_activation: false,
            active: false,
            locked: false,
            scale: MIN_ZOOM_SCALE,
            view_offset: (0.0, 0.0),
            panning: false,
            last_pan_pos: (0.0, 0.0),
        }
    }

    pub fn manager_available(&self) -> bool {
        self.manager.is_some()
    }

    pub fn set_active_output(&mut self, output: Option<wl_output::WlOutput>, id: Option<u32>) {
        self.active_output = output;
        self.active_output_id = id;
    }

    pub fn set_active_geometry(&mut self, geometry: Option<OutputGeometry>) {
        self.layout_generations
            .update(self.active_geometry.as_ref(), geometry.as_ref());
        self.active_geometry = geometry;
    }

    pub(in crate::backend::wayland) fn source_context_matches(
        &self,
        provenance: ScreenImageProvenance,
    ) -> bool {
        self.active_output_id == Some(provenance.output_id)
            && self.layout_generations.active_output == provenance.output_layout_generation
    }

    pub(in crate::backend::wayland) fn image_provenance(&self) -> Option<ScreenImageProvenance> {
        self.image.as_ref()?;
        self.image_provenance
    }

    pub fn image(&self) -> Option<&FrozenImage> {
        self.image.as_deref()
    }

    pub(in crate::backend::wayland) fn shared_image(&self) -> Option<Arc<FrozenImage>> {
        self.image.clone()
    }

    pub fn image_generation(&self) -> u64 {
        self.image_generation
    }

    pub(in crate::backend::wayland) fn install_image(
        &mut self,
        image: FrozenImage,
        provenance: ScreenImageProvenance,
    ) {
        self.image_target_dimensions = self
            .active_geometry
            .as_ref()
            .map(OutputGeometry::buffer_size)
            .or(Some((image.width, image.height)));
        self.image = Some(Arc::new(image));
        self.image_provenance = Some(provenance);
        self.bump_image_generation();
    }

    #[cfg(test)]
    pub fn set_image(&mut self, image: FrozenImage) {
        self.image_target_dimensions = self
            .active_geometry
            .as_ref()
            .map(OutputGeometry::buffer_size)
            .or(Some((image.width, image.height)));
        self.image = Some(Arc::new(image));
        self.image_provenance = None;
        self.bump_image_generation();
    }

    #[cfg(test)]
    pub(in crate::backend::wayland) fn set_image_with_provenance_for_test(
        &mut self,
        image: FrozenImage,
        provenance: ScreenImageProvenance,
    ) {
        self.image_target_dimensions = self
            .active_geometry
            .as_ref()
            .map(OutputGeometry::buffer_size)
            .or(Some((image.width, image.height)));
        self.image = Some(Arc::new(image));
        self.image_provenance = Some(provenance);
        self.bump_image_generation();
    }

    pub fn clear_image(&mut self) -> bool {
        let had_image = self.image.take().is_some();
        self.image_provenance = None;
        self.image_target_dimensions = None;
        if had_image {
            self.bump_image_generation();
        }
        had_image
    }

    pub fn is_in_progress(&self) -> bool {
        self.capture.is_some()
            || self.portal.is_running()
            || self.preflight.is_pending()
            || self.layout_retry.is_pending()
    }

    #[cfg(test)]
    pub fn preflight_pending(&self) -> bool {
        self.preflight.is_pending()
    }

    pub(in crate::backend::wayland) fn take_preflight_pending(
        &mut self,
    ) -> Option<ZoomCaptureBackend> {
        self.preflight.take_pending()
    }

    #[cfg(test)]
    pub(super) fn snapshot_preflight_layout(&mut self) {
        self.preflight.begin(
            ZoomCaptureBackend::Portal,
            self.active_output_id,
            self.layout_generations.desktop,
        );
    }

    pub(super) fn capture_layout_generation(&self, backend: ZoomCaptureBackend) -> u64 {
        match backend {
            ZoomCaptureBackend::Portal => self.layout_generations.desktop,
            ZoomCaptureBackend::WlrScreencopy => self.layout_generations.active_output,
        }
    }

    pub(super) fn ensure_preflight_layout_current(&self) -> Result<(), CapturePreflightError> {
        self.preflight.ensure_layout_current(
            self.active_output_id,
            self.preflight
                .backend()
                .map(|backend| self.capture_layout_generation(backend))
                .unwrap_or(self.layout_generations.active_output),
        )
    }

    #[cfg(test)]
    pub fn preflight_layout_is_current(&self) -> bool {
        self.ensure_preflight_layout_current().is_ok()
    }

    pub(super) fn finish_stale_direct_capture(&mut self, input_state: &mut InputState) {
        self.cancel_with_outcome(input_state, false, ZoomSourceOutcome::StaleLayout);
    }

    pub fn take_capture_done(&mut self) -> bool {
        let done = self.capture_done;
        self.capture_done = false;
        done
    }

    pub fn is_engaged(&self) -> bool {
        self.active || self.pending_activation
    }

    pub fn request_activation(&mut self) {
        if !self.active {
            self.pending_activation = true;
        }
    }

    pub fn activate_without_capture(&mut self) {
        self.active = true;
        self.pending_activation = false;
    }

    pub fn abort_capture(&mut self) -> bool {
        let mut changed = self.pending_activation || self.layout_retry.is_pending();
        if let Some(capture) = self.capture.take() {
            capture.frame.destroy();
            changed = true;
        }
        if self.preflight.is_pending() || self.portal.is_running() {
            changed = true;
        }
        self.preflight = CapturePreflight::Idle;
        self.layout_retry = PortalLayoutRetry::default();
        self.portal.finish();
        self.pending_activation = false;
        if changed {
            self.finish_source_capture(ZoomSourceOutcome::Aborted);
            self.capture_done = true;
        }
        changed
    }

    pub fn deactivate(&mut self, input_state: &mut InputState) {
        self.cancel_with_outcome(input_state, true, ZoomSourceOutcome::Deactivated);
    }

    pub fn reset_view(&mut self) {
        self.scale = MIN_ZOOM_SCALE;
        self.view_offset = (0.0, 0.0);
        self.panning = false;
        self.last_pan_pos = (0.0, 0.0);
    }

    #[allow(dead_code)] // Kept as the explicit non-deactivation capture terminal.
    pub fn cancel(&mut self, input_state: &mut InputState, force_reset: bool) {
        self.cancel_with_outcome(input_state, force_reset, ZoomSourceOutcome::Cancelled);
    }

    pub(in crate::backend::wayland) fn fail_capture(
        &mut self,
        input_state: &mut InputState,
        force_reset: bool,
        message: impl Into<String>,
    ) {
        self.cancel_with_outcome(
            input_state,
            force_reset,
            ZoomSourceOutcome::Failed(message.into()),
        );
    }

    fn bump_image_generation(&mut self) {
        self.image_generation = self.image_generation.wrapping_add(1).max(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;

    #[tokio::test]
    async fn capture_ids_are_monotonic_and_terminals_require_a_current_capture() {
        let mut state = ZoomState::new(None);

        assert_eq!(state.current_capture_id(), None);
        state.abort_capture();
        assert_eq!(state.take_source_terminal(), None);

        state
            .start_capture(crate::backend::wayland::zoom::ZoomCaptureBackend::WlrScreencopy)
            .expect("first capture starts");
        let first = state.current_capture_id().expect("first capture id");
        assert!(state.abort_capture());
        assert_eq!(
            state.take_source_terminal(),
            Some(ZoomSourceTerminal {
                id: first,
                outcome: ZoomSourceOutcome::Aborted,
                report: None,
            })
        );

        state
            .start_capture(crate::backend::wayland::zoom::ZoomCaptureBackend::WlrScreencopy)
            .expect("second capture starts");
        let second = state.current_capture_id().expect("second capture id");
        assert!(second > first);
    }

    #[tokio::test]
    async fn capture_refuses_to_start_until_the_previous_terminal_is_drained() {
        let mut state = ZoomState::new(None);

        state
            .start_capture(crate::backend::wayland::zoom::ZoomCaptureBackend::WlrScreencopy)
            .expect("first capture starts");
        let first = state.current_capture_id().expect("first capture id");
        assert!(state.abort_capture());

        let error = state
            .start_capture(crate::backend::wayland::zoom::ZoomCaptureBackend::WlrScreencopy)
            .expect_err("an undrained terminal blocks the next capture");

        assert_eq!(
            error.to_string(),
            "a zoom capture terminal is still pending"
        );
        assert_eq!(state.current_capture_id(), None);
        assert!(!state.preflight_pending());
        assert_eq!(
            state.take_source_terminal(),
            Some(ZoomSourceTerminal {
                id: first,
                outcome: ZoomSourceOutcome::Aborted,
                report: None,
            })
        );
    }

    #[tokio::test]
    async fn portal_desktop_change_preserves_installed_source_but_invalidates_preflight() {
        let mut state = ZoomState::new(None);
        state.set_active_output(None, Some(7));
        let geometry = OutputGeometry::update_from(
            Some((0, 0)),
            Some((2, 1)),
            (2, 1),
            1,
            wl_output::Transform::Normal,
            Some((2, 1)),
        )
        .unwrap();
        state.set_active_geometry(Some(geometry.clone()));
        let provenance = ScreenImageProvenance::new(
            7,
            state.layout_generations.active_output,
            1,
            wl_output::Transform::Normal,
        )
        .unwrap();
        state.install_image(
            FrozenImage {
                width: 2,
                height: 1,
                stride: 8,
                data: vec![7; 8],
            },
            provenance,
        );
        state
            .start_capture(crate::backend::wayland::zoom::ZoomCaptureBackend::Portal)
            .unwrap();
        let generation = state.image_generation();
        let mut changed = geometry.with_known_output_count(Some(2));
        changed.screenshot_size = Some((4, 1));

        state.set_active_geometry(Some(changed));

        assert!(state.source_context_matches(provenance));
        assert_eq!(state.image_generation(), generation);
        assert!(state.ensure_preflight_layout_current().is_err());
    }

    #[test]
    fn typed_terminal_is_emitted_once_for_each_current_capture() {
        let outcomes = [
            ZoomSourceOutcome::Ready {
                installed_generation: 9,
            },
            ZoomSourceOutcome::Aborted,
            ZoomSourceOutcome::Cancelled,
            ZoomSourceOutcome::Deactivated,
            ZoomSourceOutcome::StaleLayout,
            ZoomSourceOutcome::Failed("failed".to_string()),
        ];

        for outcome in outcomes {
            let mut state = ZoomState::new(None);
            let id = state.begin_identified_capture();
            state.finish_source_capture(outcome.clone());
            state.finish_source_capture(ZoomSourceOutcome::Failed("duplicate".to_string()));
            let report =
                matches!(&outcome, ZoomSourceOutcome::StaleLayout).then(|| ZoomTerminalReport {
                    source: "zoom",
                    message: "Zoom failed after the display layout changed".to_string(),
                });

            assert_eq!(
                state.take_source_terminal(),
                Some(ZoomSourceTerminal {
                    id,
                    outcome,
                    report,
                })
            );
            assert_eq!(state.current_capture_id(), None);
            assert_eq!(state.take_source_terminal(), None);
        }
    }

    #[test]
    fn cancel_and_deactivate_publish_distinct_terminals() {
        let mut state = ZoomState::new(None);
        let mut input_state = make_test_input_state();
        let cancelled = state.begin_identified_capture();
        state.cancel(&mut input_state, false);
        assert_eq!(
            state.take_source_terminal(),
            Some(ZoomSourceTerminal {
                id: cancelled,
                outcome: ZoomSourceOutcome::Cancelled,
                report: None,
            })
        );

        let deactivated = state.begin_identified_capture();
        state.deactivate(&mut input_state);
        assert_eq!(
            state.take_source_terminal(),
            Some(ZoomSourceTerminal {
                id: deactivated,
                outcome: ZoomSourceOutcome::Deactivated,
                report: None,
            })
        );
    }

    #[test]
    fn preflight_failure_terminal_carries_the_specific_error_report() {
        for message in [
            "specific backend failure",
            "Zoom failed after the display layout changed",
        ] {
            let mut state = ZoomState::new(None);
            let mut input_state = make_test_input_state();
            let id = state.begin_identified_capture();

            state.finish_preflight_failure(
                &mut input_state,
                CapturePreflightError::Backend(message.to_string()),
            );

            assert_eq!(
                state.take_source_terminal(),
                Some(ZoomSourceTerminal {
                    id,
                    outcome: ZoomSourceOutcome::Failed(message.to_string()),
                    report: Some(ZoomTerminalReport {
                        source: "zoom",
                        message: message.to_string(),
                    }),
                })
            );
        }
    }

    #[test]
    fn aborting_pending_activation_completes_the_capture_lifecycle() {
        let mut state = ZoomState::new(None);
        state.request_activation();
        assert!(state.is_engaged());
        assert!(!state.is_in_progress());

        assert!(state.abort_capture());

        assert!(!state.is_engaged());
        assert!(state.take_capture_done());
    }

    #[test]
    fn stale_direct_capture_publishes_one_report_and_preserves_the_current_image() {
        let mut state = ZoomState::new(None);
        let mut input_state = make_test_input_state();
        let id = state.begin_identified_capture();
        state.set_image(FrozenImage {
            width: 1,
            height: 1,
            stride: 4,
            data: vec![4; 4],
        });
        let generation = state.image_generation();

        state.finish_stale_direct_capture(&mut input_state);

        assert_eq!(input_state.test_toast_count(), 0);
        assert_eq!(
            state.take_source_terminal(),
            Some(ZoomSourceTerminal {
                id,
                outcome: ZoomSourceOutcome::StaleLayout,
                report: Some(ZoomTerminalReport {
                    source: "zoom",
                    message: "Zoom failed after the display layout changed".to_string(),
                }),
            })
        );
        assert_eq!(state.image_generation(), generation);
        assert_eq!(state.image().unwrap().data, vec![4; 4]);
        assert!(state.take_capture_done());
    }
}
