mod activation;
mod direct;

pub(super) use direct::{DirectCaptureAttempt, DirectCaptureContext};

use crate::backend::wayland::capture_preflight::{
    CaptureLayoutGenerations, CapturePreflight, CapturePreflightError, PortalLayoutRetry,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wayland_client::protocol::wl_output;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::backend::wayland::RuntimeWakeHandle;
use crate::backend::wayland::acquisition::{
    ScreenAcquisitionCompletion, ScreenAcquisitionId, ScreenAcquisitionOutcome,
    ScreenAcquisitionOwner,
};
use crate::backend::wayland::frozen::{FrozenImage, ScreenImageProvenance};
use crate::backend::wayland::frozen_geometry::OutputGeometry;
use crate::backend::wayland::portal_task::PortalOperation;
use crate::input::InputState;
use crate::input::state::{Toast, ToastPriority};

use super::PortalCaptureResult;
use super::ext_image_copy::ExtImageCopyManagers;

struct PendingFrozenImage {
    image: FrozenImage,
    target_output_id: Option<u32>,
    layout_generation: u64,
    source_geometry: Option<OutputGeometry>,
    output_transform: Option<wl_output::Transform>,
    source: FrozenCaptureSource,
}

#[derive(Clone, Copy)]
enum FrozenCaptureSource {
    ActiveOutput,
    Portal { desktop_generation: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum FrozenCaptureBackend {
    WlrScreencopy,
    ExtImageCopy,
    Portal,
}

/// End-to-end controller for frozen mode capture and image storage.
#[allow(clippy::type_complexity)]
pub struct FrozenState {
    enabled: bool,
    pending_on_start: bool,
    pub(super) manager: Option<ZwlrScreencopyManagerV1>,
    pub(super) ext_managers: Option<ExtImageCopyManagers>,
    pub(super) portal_available: bool,
    pub(super) active_output: Option<wl_output::WlOutput>,
    pub(super) active_output_id: Option<u32>,
    pub(super) active_geometry: Option<OutputGeometry>,
    /// Shared active-output and full-desktop validity generations.
    pub(super) layout_generations: CaptureLayoutGenerations,
    pub(super) direct_capture: Option<DirectCaptureAttempt>,
    pub(super) image: Option<Arc<FrozenImage>>,
    image_provenance: Option<ScreenImageProvenance>,
    image_target_dimensions: Option<(u32, u32)>,
    image_generation: u64,
    pub(super) portal: PortalOperation<PortalCaptureResult>,
    pub(super) runtime_wake: Option<RuntimeWakeHandle>,
    pub(super) preflight: CapturePreflight<FrozenCaptureBackend>,
    pub(super) capture_done: bool,
    pub(super) layout_retry: PortalLayoutRetry,
    pending_image: Option<PendingFrozenImage>,
    acquisition_attempt: Option<(ScreenAcquisitionId, ScreenAcquisitionOwner)>,
    acquisition_completion: Option<ScreenAcquisitionCompletion>,
}

impl FrozenState {
    #[cfg(test)]
    pub fn new(manager: Option<ZwlrScreencopyManagerV1>) -> Self {
        Self::new_inner(manager, None, false, None, true, false)
    }

    #[cfg(test)]
    pub(in crate::backend::wayland) fn new_with_runtime_wake(
        manager: Option<ZwlrScreencopyManagerV1>,
        runtime_wake: RuntimeWakeHandle,
    ) -> Self {
        Self::new_inner(manager, None, true, Some(runtime_wake), true, false)
    }

    pub(in crate::backend::wayland) fn new_with_backends(
        manager: Option<ZwlrScreencopyManagerV1>,
        ext_managers: Option<ExtImageCopyManagers>,
        portal_available: bool,
        runtime_wake: RuntimeWakeHandle,
        enabled: bool,
        pending_on_start: bool,
    ) -> Self {
        Self::new_inner(
            manager,
            ext_managers,
            portal_available,
            Some(runtime_wake),
            enabled,
            pending_on_start,
        )
    }

    fn new_inner(
        manager: Option<ZwlrScreencopyManagerV1>,
        ext_managers: Option<ExtImageCopyManagers>,
        portal_available: bool,
        runtime_wake: Option<RuntimeWakeHandle>,
        enabled: bool,
        pending_on_start: bool,
    ) -> Self {
        Self {
            enabled,
            pending_on_start,
            manager,
            ext_managers,
            portal_available,
            active_output: None,
            active_output_id: None,
            active_geometry: None,
            layout_generations: CaptureLayoutGenerations::default(),
            direct_capture: None,
            image: None,
            image_provenance: None,
            image_target_dimensions: None,
            image_generation: 0,
            portal: PortalOperation::default(),
            runtime_wake,
            preflight: CapturePreflight::Idle,
            capture_done: false,
            layout_retry: PortalLayoutRetry::default(),
            pending_image: None,
            acquisition_attempt: None,
            acquisition_completion: None,
        }
    }

    pub(in crate::backend::wayland) const fn enabled(&self) -> bool {
        self.enabled
    }

    pub(in crate::backend::wayland) fn take_pending_on_start(&mut self) -> bool {
        std::mem::take(&mut self.pending_on_start)
    }

    pub(in crate::backend::wayland) fn preferred_backend(&self) -> Option<FrozenCaptureBackend> {
        select_capture_backend(
            self.manager.is_some(),
            self.ext_managers.is_some(),
            self.portal_available,
        )
    }

    pub(super) fn next_backend_after(
        &self,
        failed: FrozenCaptureBackend,
    ) -> Option<FrozenCaptureBackend> {
        next_capture_backend_after(failed, self.ext_managers.is_some(), self.portal_available)
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

    pub(in crate::backend::wayland) fn desktop_layout_generation(&self) -> u64 {
        self.layout_generations.desktop
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

    #[cfg(test)]
    pub fn set_image(&mut self, image: FrozenImage) {
        self.image_target_dimensions = Some((image.width, image.height));
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
        self.image_target_dimensions = Some((image.width, image.height));
        self.image = Some(Arc::new(image));
        self.image_provenance = Some(provenance);
        self.bump_image_generation();
    }

    pub fn is_in_progress(&self) -> bool {
        self.direct_capture.is_some()
            || self.portal.is_running()
            || self.preflight.is_pending()
            || self.pending_image.is_some()
            || self.layout_retry.is_pending()
    }

    pub(in crate::backend::wayland) fn take_preflight_pending(
        &mut self,
    ) -> Option<FrozenCaptureBackend> {
        self.preflight.take_pending()
    }

    #[cfg(test)]
    pub(super) fn snapshot_preflight_layout(&mut self) {
        self.preflight.begin(
            FrozenCaptureBackend::Portal,
            self.active_output_id,
            self.layout_generations.desktop,
        );
    }

    pub(super) fn capture_layout_generation(&self, backend: FrozenCaptureBackend) -> u64 {
        match backend {
            FrozenCaptureBackend::Portal => self.layout_generations.desktop,
            FrozenCaptureBackend::WlrScreencopy | FrozenCaptureBackend::ExtImageCopy => {
                self.layout_generations.active_output
            }
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

    pub(super) fn push_stale_layout_toast(input_state: &mut InputState) {
        input_state.push_toast(
            ToastPriority::Critical,
            "freeze",
            Toast::error("Freeze failed after the display layout changed"),
        );
    }

    pub(in crate::backend::wayland) fn finish_failed_fallback_capture(
        &mut self,
        input_state: &mut InputState,
    ) {
        let stale_message = self
            .ensure_preflight_layout_current()
            .err()
            .map(|error| error.message("Freeze"));
        let message = stale_message
            .clone()
            .unwrap_or_else(|| "Freeze could not capture the screen.".to_string());
        if self.has_acquisition_attempt() {
            let outcome = if stale_message.is_some() {
                ScreenAcquisitionOutcome::StaleLayout
            } else {
                ScreenAcquisitionOutcome::Failed(message)
            };
            self.finish_acquisition(outcome, input_state);
            return;
        }
        input_state.push_toast(ToastPriority::Critical, "freeze", Toast::error(message));
        self.cancel(input_state);
    }

    pub(in crate::backend::wayland) fn finish_preflight_failure(
        &mut self,
        error: CapturePreflightError,
        input_state: &mut InputState,
    ) {
        if !self.has_acquisition_attempt() {
            self.abandon_acquisition(input_state);
            return;
        }

        let outcome = if error.is_stale_layout() {
            ScreenAcquisitionOutcome::StaleLayout
        } else {
            ScreenAcquisitionOutcome::Failed(error.message("Freeze"))
        };

        self.finish_acquisition(outcome, input_state);
    }

    pub fn take_capture_done(&mut self) -> bool {
        let done = self.capture_done;
        self.capture_done = false;
        done
    }

    pub(in crate::backend::wayland) fn start_capture_for(
        &mut self,
        id: ScreenAcquisitionId,
        owner: ScreenAcquisitionOwner,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.acquisition_attempt.is_none() && self.acquisition_completion.is_none(),
            "another screen acquisition is still pending"
        );
        anyhow::ensure!(
            !self.is_in_progress(),
            "another frozen capture is already in progress"
        );
        self.start_capture()?;
        self.acquisition_attempt = Some((id, owner));
        Ok(())
    }

    fn finish_attempt_resources(&mut self) {
        self.layout_retry = PortalLayoutRetry::default();
        if let Some(capture) = self.direct_capture.take() {
            capture.destroy();
        }
        self.preflight = CapturePreflight::Idle;
        self.portal.finish();
        self.pending_image = None;
        self.capture_done = true;
    }

    pub(in crate::backend::wayland) fn finish_ready_acquisition(
        &mut self,
        input_state: &mut InputState,
    ) {
        debug_assert!(self.image().is_some());
        debug_assert!(input_state.frozen_active());
        self.finish_acquisition(
            ScreenAcquisitionOutcome::Ready {
                installed_generation: self.image_generation(),
            },
            input_state,
        );
    }

    pub(in crate::backend::wayland) fn finish_acquisition(
        &mut self,
        outcome: ScreenAcquisitionOutcome,
        input_state: &mut InputState,
    ) {
        let Some((id, owner)) = self.acquisition_attempt.take() else {
            return;
        };
        debug_assert!(self.acquisition_completion.is_none());
        self.finish_attempt_resources();
        if !matches!(outcome, ScreenAcquisitionOutcome::Ready { .. }) {
            input_state.set_frozen_active(false);
            input_state.needs_redraw = true;
        }
        self.acquisition_completion = Some(ScreenAcquisitionCompletion { id, owner, outcome });
    }

    pub(in crate::backend::wayland) fn abandon_acquisition(
        &mut self,
        input_state: &mut InputState,
    ) {
        self.acquisition_attempt = None;
        self.finish_attempt_resources();
        input_state.set_frozen_active(false);
        input_state.needs_redraw = true;
    }

    pub(in crate::backend::wayland) fn acquisition_completion(
        &self,
    ) -> Option<&ScreenAcquisitionCompletion> {
        self.acquisition_completion.as_ref()
    }

    pub(in crate::backend::wayland) fn has_acquisition_attempt(&self) -> bool {
        self.acquisition_attempt.is_some()
    }

    pub(in crate::backend::wayland) fn take_acquisition_completion(
        &mut self,
    ) -> Option<ScreenAcquisitionCompletion> {
        self.acquisition_completion.take()
    }

    pub(in crate::backend::wayland) fn take_matching_acquisition_completion(
        &mut self,
        id: ScreenAcquisitionId,
        owner: ScreenAcquisitionOwner,
    ) -> Option<ScreenAcquisitionCompletion> {
        if !self
            .acquisition_completion
            .as_ref()
            .is_some_and(|completion| completion.id == id && completion.owner == owner)
        {
            return None;
        }
        self.acquisition_completion.take()
    }

    #[cfg(test)]
    fn attempt(&self) -> Option<(ScreenAcquisitionId, ScreenAcquisitionOwner)> {
        self.acquisition_attempt
    }

    pub(in crate::backend::wayland) fn direct_capture_timeout(
        &self,
        now: Instant,
    ) -> Option<Duration> {
        self.direct_capture
            .as_ref()
            .map(|capture| capture.context().timeout(now))
    }

    pub(in crate::backend::wayland) fn take_timed_out_direct_capture(
        &mut self,
        now: Instant,
    ) -> Option<FrozenCaptureBackend> {
        let capture = self.direct_capture.as_ref()?;
        if !capture.context().timeout(now).is_zero() {
            return None;
        }
        let backend = capture.backend();
        let capture = self.direct_capture.take()?;
        capture.destroy();
        Some(backend)
    }
}

fn select_capture_backend(
    wlr_screencopy: bool,
    ext_image_copy: bool,
    portal: bool,
) -> Option<FrozenCaptureBackend> {
    if wlr_screencopy {
        Some(FrozenCaptureBackend::WlrScreencopy)
    } else if ext_image_copy {
        Some(FrozenCaptureBackend::ExtImageCopy)
    } else if portal {
        Some(FrozenCaptureBackend::Portal)
    } else {
        None
    }
}

fn next_capture_backend_after(
    failed: FrozenCaptureBackend,
    ext_image_copy: bool,
    portal: bool,
) -> Option<FrozenCaptureBackend> {
    match failed {
        FrozenCaptureBackend::WlrScreencopy if ext_image_copy => {
            Some(FrozenCaptureBackend::ExtImageCopy)
        }
        FrozenCaptureBackend::WlrScreencopy | FrozenCaptureBackend::ExtImageCopy if portal => {
            Some(FrozenCaptureBackend::Portal)
        }
        FrozenCaptureBackend::WlrScreencopy
        | FrozenCaptureBackend::ExtImageCopy
        | FrozenCaptureBackend::Portal => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::acquisition::{
        ScreenAcquisitionCompletion, ScreenAcquisitionOutcome, ScreenAcquisitionOwner,
        ScreenAcquisitionRegistry,
    };
    use crate::input::state::test_support::make_test_input_state;

    fn verified_output_geometry(
        overlay_logical: (u32, u32),
        scale: i32,
        transform: wl_output::Transform,
        pixel_size: (u32, u32),
    ) -> OutputGeometry {
        OutputGeometry::update_from(
            Some((0, 0)),
            Some((
                i32::try_from(overlay_logical.0).expect("test width"),
                i32::try_from(overlay_logical.1).expect("test height"),
            )),
            overlay_logical,
            scale,
            transform,
            Some(pixel_size),
        )
        .expect("verified test output geometry")
    }

    #[test]
    fn capability_gate_matches_construction() {
        let enabled = FrozenState::new_inner(None, None, false, None, true, false);
        let disabled = FrozenState::new_inner(None, None, false, None, false, false);

        assert!(enabled.enabled());
        assert!(!disabled.enabled());
    }

    #[test]
    fn pending_on_start_is_consumed_once() {
        let mut state = FrozenState::new_inner(None, None, false, None, true, true);

        assert!(state.take_pending_on_start());
        assert!(!state.take_pending_on_start());
    }

    #[test]
    fn capture_backend_priority_is_wlr_then_ext_then_portal() {
        assert_eq!(
            select_capture_backend(true, true, true),
            Some(FrozenCaptureBackend::WlrScreencopy)
        );
        assert_eq!(
            select_capture_backend(false, true, true),
            Some(FrozenCaptureBackend::ExtImageCopy)
        );
        assert_eq!(
            select_capture_backend(false, false, true),
            Some(FrozenCaptureBackend::Portal)
        );
        assert_eq!(select_capture_backend(false, false, false), None);
        assert_eq!(
            next_capture_backend_after(FrozenCaptureBackend::WlrScreencopy, true, true),
            Some(FrozenCaptureBackend::ExtImageCopy)
        );
        assert_eq!(
            next_capture_backend_after(FrozenCaptureBackend::WlrScreencopy, false, true),
            Some(FrozenCaptureBackend::Portal)
        );
        assert_eq!(
            next_capture_backend_after(FrozenCaptureBackend::ExtImageCopy, true, true),
            Some(FrozenCaptureBackend::Portal)
        );
        assert_eq!(
            next_capture_backend_after(FrozenCaptureBackend::Portal, true, true),
            None
        );
    }

    #[test]
    fn active_output_capture_accepts_native_fractional_scale_dimensions() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.set_active_output(None, Some(7));
        state.set_pending_output_image(
            FrozenImage {
                width: 10,
                height: 10,
                stride: 40,
                data: vec![0; 10 * 10 * 4],
            },
            7,
            verified_output_geometry((6, 6), 2, wl_output::Transform::Normal, (10, 10)),
        );

        state
            .activate_pending_image(12, 12, &mut input_state)
            .expect("native output pixels should render into the fractional-scale buffer");

        let image = state.image().expect("the frozen image should be active");
        assert_eq!((image.width, image.height), (10, 10));
        assert!(input_state.frozen_active());

        state.handle_resize(12, 12, &mut input_state);
        assert!(state.image().is_some());

        state.handle_resize(13, 12, &mut input_state);
        assert!(state.image().is_none());
        assert!(!input_state.frozen_active());
    }

    #[test]
    fn active_output_capture_rejects_known_pixel_size_mismatch() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.set_active_output(None, Some(7));
        let geometry = OutputGeometry::update_from(
            Some((0, 0)),
            Some((3, 2)),
            (3, 2),
            2,
            wl_output::Transform::Normal,
            Some((5, 3)),
        )
        .expect("known output geometry");
        state.set_pending_output_image(
            FrozenImage {
                width: 4,
                height: 3,
                stride: 16,
                data: vec![0; 4 * 3 * 4],
            },
            7,
            geometry,
        );

        assert!(
            state
                .activate_pending_image(6, 4, &mut input_state)
                .is_err()
        );
        assert!(state.image().is_none());
        assert!(!input_state.frozen_active());
    }

    #[test]
    fn active_output_capture_rejects_unknown_pixel_size() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.set_active_output(None, Some(7));
        let geometry = OutputGeometry::update_from(
            Some((0, 0)),
            Some((3, 2)),
            (3, 2),
            2,
            wl_output::Transform::Normal,
            None,
        )
        .expect("geometry without mode pixels");
        state.set_pending_output_image(
            FrozenImage {
                width: 6,
                height: 4,
                stride: 24,
                data: vec![0; 6 * 4 * 4],
            },
            7,
            geometry,
        );

        assert!(
            state
                .activate_pending_image(6, 4, &mut input_state)
                .is_err()
        );
        assert!(state.image().is_none());
        assert!(!input_state.frozen_active());
    }

    #[test]
    fn active_output_capture_prefers_protocol_transform_over_geometry() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.set_active_output(None, Some(7));
        state.set_pending_output_image_with_transform(
            FrozenImage {
                width: 2,
                height: 1,
                stride: 8,
                data: vec![1, 0, 0, 255, 2, 0, 0, 255],
            },
            7,
            verified_output_geometry((1, 2), 1, wl_output::Transform::Normal, (1, 2)),
            Some(wl_output::Transform::_90),
        );

        state
            .activate_pending_image(1, 2, &mut input_state)
            .expect("capture transform should orient the frozen image");

        let image = state.image().expect("the frozen image should be active");
        assert_eq!((image.width, image.height), (1, 2));
    }

    #[test]
    fn active_output_capture_fails_closed_when_transform_input_is_malformed() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.set_active_output(None, Some(7));
        state.set_pending_output_image_with_transform(
            FrozenImage {
                width: 2,
                height: 2,
                stride: 8,
                data: vec![0; 12],
            },
            7,
            verified_output_geometry((2, 2), 1, wl_output::Transform::Normal, (2, 2)),
            Some(wl_output::Transform::_90),
        );

        assert!(
            state
                .activate_pending_image(2, 2, &mut input_state)
                .is_err()
        );
        assert!(state.image().is_none());
        assert!(!input_state.frozen_active());
        assert!(state.take_capture_done());
    }

    #[test]
    fn active_output_capture_rejects_stretching_into_a_different_viewport() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.set_active_output(None, Some(7));
        state.set_pending_output_image(
            FrozenImage {
                width: 320,
                height: 180,
                stride: 1280,
                data: vec![0; 320 * 180 * 4],
            },
            7,
            verified_output_geometry((320, 176), 1, wl_output::Transform::Normal, (320, 180)),
        );

        let error = state
            .activate_pending_image(320, 176, &mut input_state)
            .expect_err("a full output cannot be stretched into a shorter viewport");
        assert!(error.contains("aspect does not match"));
        assert!(state.image().is_none());
        assert!(!input_state.frozen_active());
    }

    fn desktop_geometry(
        logical_x: i32,
        logical_y: i32,
        logical_width: u32,
        logical_height: u32,
        scale: i32,
        screenshot_origin: Option<(u32, u32)>,
    ) -> OutputGeometry {
        let pixel_scale = u32::try_from(scale).expect("test scale must be positive");
        OutputGeometry {
            logical_x,
            logical_y,
            logical_width,
            logical_height,
            scale,
            transform: wl_output::Transform::Normal,
            overlay_buffer_size: (logical_width * pixel_scale, logical_height * pixel_scale),
            pixel_size: Some((logical_width * pixel_scale, logical_height * pixel_scale)),
            screenshot_origin,
            screenshot_size: None,
            known_output_count: None,
            portal_outputs: None,
        }
    }

    #[test]
    fn output_layout_generation_bumps_only_when_geometry_changes() {
        let mut state = FrozenState::new(None);
        assert_eq!(state.layout_generations.active_output, 0);
        let first = desktop_geometry(0, 0, 4, 1, 1, Some((0, 0)));
        state.set_active_geometry(Some(first.clone()));
        assert_eq!(state.layout_generations.active_output, 1);
        state.set_active_geometry(Some(first));
        assert_eq!(state.layout_generations.active_output, 1);
        state.set_active_geometry(Some(desktop_geometry(0, 0, 4, 1, 1, Some((6, 0)))));
        assert_eq!(state.layout_generations.active_output, 1);
        assert_eq!(state.layout_generations.desktop, 2);
    }

    #[test]
    fn other_output_metadata_does_not_invalidate_a_direct_source() {
        let mut state = FrozenState::new(None);
        let mut input = make_test_input_state();
        state.set_active_output(None, Some(7));
        let first = desktop_geometry(0, 0, 2, 1, 1, Some((0, 0)));
        state.set_active_geometry(Some(first.clone()));
        let generation = state.layout_generations.active_output;
        let provenance =
            ScreenImageProvenance::new(7, generation, 1, wl_output::Transform::Normal).unwrap();
        state.set_pending_output_image(
            FrozenImage {
                width: 2,
                height: 1,
                stride: 8,
                data: vec![7; 8],
            },
            7,
            first.clone(),
        );
        let mut changed = first.with_known_output_count(Some(2));
        changed.screenshot_origin = Some((8, 0));

        state.set_active_geometry(Some(changed));

        assert!(state.source_context_matches(provenance));
        assert!(state.activate_pending_image(2, 1, &mut input).unwrap());
        assert_eq!(state.image_provenance(), Some(provenance));
        assert!(state.layout_generations.desktop > generation);
    }

    #[test]
    fn desktop_capture_keeps_fractional_output_pixels_for_a_larger_overlay_buffer() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.set_active_output(None, Some(7));
        let mut geometry = desktop_geometry(0, 0, 3, 2, 2, Some((0, 0)));
        geometry.pixel_size = Some((5, 3));
        state.set_pending_portal_image(
            FrozenImage {
                width: 5,
                height: 3,
                stride: 20,
                data: vec![7; 5 * 3 * 4],
            },
            Some(7),
            Some(geometry),
        );

        state
            .activate_pending_image(6, 4, &mut input_state)
            .expect("native output pixels should render into the integer-scale overlay buffer");

        let image = state.image().expect("the frozen image should be active");
        assert_eq!((image.width, image.height), (5, 3));
        state.handle_resize(6, 4, &mut input_state);
        assert!(state.image().is_some());
        assert!(input_state.frozen_active());
    }

    #[test]
    fn portal_activation_installs_a_completed_single_output_crop() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.set_active_output(None, Some(7));
        let geometry = desktop_geometry(10, 20, 2, 1, 2, None).with_known_output_count(Some(1));
        assert_eq!(geometry.portal_crop_origin(4, 2), Some((0, 0)));

        state.set_pending_portal_image(
            FrozenImage {
                width: 4,
                height: 2,
                stride: 16,
                data: vec![7; 4 * 2 * 4],
            },
            Some(7),
            Some(geometry),
        );

        state
            .activate_pending_image_with_live_outputs(4, 2, &mut input_state, Some(1))
            .expect("a completed portal crop retains its native pixel dimensions");
        assert!(state.image().is_some());
        assert!(input_state.frozen_active());
    }

    #[test]
    fn desktop_capture_refuses_activation_without_capture_time_output_identity() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        let geometry = desktop_geometry(0, 0, 2, 1, 1, Some((0, 0)));
        state.set_pending_portal_image(
            FrozenImage {
                width: 2,
                height: 1,
                stride: 8,
                data: vec![7; 8],
            },
            None,
            Some(geometry),
        );

        let error = state
            .activate_pending_image(2, 1, &mut input_state)
            .expect_err("missing capture-time output identity must fail closed");

        assert_eq!(error, "Freeze capture source identity is unavailable");
        assert!(state.image().is_none());
        assert!(state.image_provenance().is_none());
        assert!(!input_state.frozen_active());
    }

    #[test]
    fn desktop_capture_rejects_a_stale_single_output_snapshot_when_live_count_grows() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        let geometry = desktop_geometry(10, 20, 2, 1, 2, None).with_known_output_count(Some(1));

        state.set_pending_portal_image(
            FrozenImage {
                width: 4,
                height: 2,
                stride: 16,
                data: vec![7; 4 * 2 * 4],
            },
            None,
            Some(geometry),
        );

        assert!(
            state
                .activate_pending_image_with_live_outputs(4, 2, &mut input_state, Some(2))
                .is_err()
        );
        assert!(state.image().is_none());
        assert!(!input_state.frozen_active());
    }

    #[test]
    fn pending_capture_is_rejected_if_output_changes_before_activation() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.set_active_output(None, Some(7));
        state.set_pending_output_image(
            FrozenImage {
                width: 1,
                height: 1,
                stride: 4,
                data: vec![0; 4],
            },
            7,
            verified_output_geometry((1, 1), 1, wl_output::Transform::Normal, (1, 1)),
        );

        state.set_active_output(None, Some(8));
        let error = state
            .activate_pending_image(1, 1, &mut input_state)
            .expect_err("stale output identity must fail closed");

        assert!(error.contains("display layout changed"));
        assert!(state.image().is_none());
        assert!(!state.has_pending_image());
        assert!(state.take_capture_done());
        assert!(!input_state.frozen_active());
    }

    #[test]
    fn pending_capture_is_rejected_if_layout_changes_before_activation() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        let first = verified_output_geometry((1, 1), 1, wl_output::Transform::Normal, (1, 1));
        let second = verified_output_geometry((2, 2), 1, wl_output::Transform::Normal, (2, 2));
        state.set_active_output(None, Some(7));
        state.set_active_geometry(Some(first.clone()));
        state.set_pending_output_image(
            FrozenImage {
                width: 1,
                height: 1,
                stride: 4,
                data: vec![0; 4],
            },
            7,
            first,
        );

        state.set_active_geometry(Some(second));
        let error = state
            .activate_pending_image(1, 1, &mut input_state)
            .expect_err("stale layout token must fail closed after delayed activation");

        assert!(error.contains("display layout changed"));
        assert!(state.image().is_none());
        assert!(!state.has_pending_image());
        assert!(!input_state.frozen_active());
    }

    #[test]
    fn preflight_layout_snapshot_goes_stale_when_geometry_changes() {
        let mut state = FrozenState::new(None);
        state.snapshot_preflight_layout();
        assert!(state.preflight_layout_is_current());
        state.set_active_geometry(Some(verified_output_geometry(
            (1, 1),
            1,
            wl_output::Transform::Normal,
            (1, 1),
        )));
        assert!(!state.preflight_layout_is_current());
    }

    #[test]
    fn exhausted_fallback_toasts_layout_change_when_preflight_is_stale() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.snapshot_preflight_layout();
        state.set_active_geometry(Some(verified_output_geometry(
            (1, 1),
            1,
            wl_output::Transform::Normal,
            (1, 1),
        )));

        state.finish_failed_fallback_capture(&mut input_state);

        let toast = input_state.active_toast().expect("visible stale rejection");
        assert!(toast.message.contains("display layout changed"));
        assert!(state.take_capture_done());
    }

    #[test]
    fn exhausted_fallback_toasts_a_generic_failure_when_layout_is_current() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.snapshot_preflight_layout();

        state.finish_failed_fallback_capture(&mut input_state);

        let toast = input_state.active_toast().expect("visible capture failure");
        assert_eq!(toast.message, "Freeze could not capture the screen.");
        assert!(state.take_capture_done());
    }

    #[test]
    fn cancel_clears_an_in_flight_portal_capture() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.portal.start(
            crate::backend::wayland::portal_task::PortalTask::disconnected_for_test(Instant::now()),
        );

        state.cancel(&mut input_state);

        assert!(!state.is_in_progress());
        assert!(state.take_capture_done());
    }

    #[test]
    fn acquisition_terminal_consumes_attempt_once_and_retains_old_image_on_failure() {
        let mut state = FrozenState::new_inner(None, None, true, None, true, false);
        let mut input_state = make_test_input_state();
        let mut registry = ScreenAcquisitionRegistry::default();
        let id = registry.request(ScreenAcquisitionOwner::Ocr).expect("id");
        state.set_image(FrozenImage {
            width: 1,
            height: 1,
            stride: 4,
            data: vec![0; 4],
        });
        input_state.set_frozen_active(true);

        state
            .start_capture_for(id, ScreenAcquisitionOwner::Ocr)
            .expect("capture starts");
        state.finish_acquisition(
            ScreenAcquisitionOutcome::Failed("capture failed".to_string()),
            &mut input_state,
        );
        state.finish_acquisition(ScreenAcquisitionOutcome::Cancelled, &mut input_state);

        assert_eq!(state.attempt(), None);
        assert!(state.image().is_some());
        assert!(!input_state.frozen_active());
        assert!(state.take_capture_done());
        let completion = state.take_acquisition_completion().expect("one completion");
        assert_eq!(completion.id, id);
        assert_eq!(completion.owner, ScreenAcquisitionOwner::Ocr);
        assert_eq!(
            completion.outcome,
            ScreenAcquisitionOutcome::Failed("capture failed".to_string())
        );
        assert_eq!(state.take_acquisition_completion(), None);
    }

    #[derive(Clone, Copy, Debug)]
    enum PendingImageRejectionFixture {
        LayoutMismatch,
        MissingActiveOutputGeometry,
        MalformedTransformBuffer,
        ActiveOutputSizeMismatch,
        MissingDesktopGeometry,
        StaleDesktopGeometry,
        MissingVerifiedPixelSize,
        PortalSizeMismatch,
        MissingOutputIdentity,
        AspectMismatch,
    }

    impl PendingImageRejectionFixture {
        const ALL: [Self; 10] = [
            Self::LayoutMismatch,
            Self::MissingActiveOutputGeometry,
            Self::MalformedTransformBuffer,
            Self::ActiveOutputSizeMismatch,
            Self::MissingDesktopGeometry,
            Self::StaleDesktopGeometry,
            Self::MissingVerifiedPixelSize,
            Self::PortalSizeMismatch,
            Self::MissingOutputIdentity,
            Self::AspectMismatch,
        ];

        fn expected_error(self) -> &'static str {
            match self {
                Self::LayoutMismatch => "Freeze failed after the display layout changed",
                Self::MissingActiveOutputGeometry => "Freeze capture geometry is unavailable",
                Self::MalformedTransformBuffer => "Freeze capture transform failed:",
                Self::ActiveOutputSizeMismatch => {
                    "Freeze capture dimensions do not match the active output"
                }
                Self::MissingDesktopGeometry | Self::StaleDesktopGeometry => {
                    "Freeze failed after the output layout changed"
                }
                Self::MissingVerifiedPixelSize => "Freeze failed after the display changed size",
                Self::PortalSizeMismatch => {
                    "Freeze portal crop dimensions do not match the active output"
                }
                Self::MissingOutputIdentity => "Freeze capture source identity is unavailable",
                Self::AspectMismatch => "Freeze capture aspect does not match the overlay surface",
            }
        }

        fn active_output_id(self) -> Option<u32> {
            match self {
                Self::StaleDesktopGeometry | Self::MissingOutputIdentity => None,
                _ => Some(7),
            }
        }

        fn install_pending(self, state: &mut FrozenState) -> ((u32, u32), Option<u32>) {
            let image = |width, height, stride, data_len| FrozenImage {
                width,
                height,
                stride,
                data: vec![2; data_len],
            };
            let output_geometry =
                || verified_output_geometry((2, 1), 1, wl_output::Transform::Normal, (2, 1));

            match self {
                Self::LayoutMismatch => {
                    state.set_pending_output_image(image(2, 1, 8, 8), 7, output_geometry());
                    state.set_active_output(None, Some(8));
                    ((2, 1), None)
                }
                Self::MissingActiveOutputGeometry => {
                    state.set_pending_output_image(image(2, 1, 8, 8), 7, output_geometry());
                    state
                        .pending_image
                        .as_mut()
                        .expect("pending image installed through the production setter")
                        .source_geometry = None;
                    ((2, 1), None)
                }
                Self::MalformedTransformBuffer => {
                    state.set_pending_output_image_with_transform(
                        image(2, 2, 8, 12),
                        7,
                        verified_output_geometry((2, 2), 1, wl_output::Transform::Normal, (2, 2)),
                        Some(wl_output::Transform::_90),
                    );
                    ((2, 2), None)
                }
                Self::ActiveOutputSizeMismatch => {
                    state.set_pending_output_image(image(3, 1, 12, 12), 7, output_geometry());
                    ((2, 1), None)
                }
                Self::MissingDesktopGeometry => {
                    state.set_pending_portal_image(image(2, 1, 8, 8), Some(7), None);
                    ((2, 1), None)
                }
                Self::StaleDesktopGeometry => {
                    state.set_pending_portal_image(
                        image(2, 1, 8, 8),
                        None,
                        Some(
                            desktop_geometry(0, 0, 2, 1, 1, None).with_known_output_count(Some(1)),
                        ),
                    );
                    ((2, 1), Some(2))
                }
                Self::MissingVerifiedPixelSize => {
                    let mut geometry = desktop_geometry(0, 0, 2, 1, 1, Some((0, 0)));
                    geometry.pixel_size = None;
                    state.set_pending_portal_image(image(2, 1, 8, 8), Some(7), Some(geometry));
                    ((2, 1), None)
                }
                Self::PortalSizeMismatch => {
                    state.set_pending_portal_image(
                        image(3, 1, 12, 12),
                        Some(7),
                        Some(desktop_geometry(0, 0, 2, 1, 1, Some((0, 0)))),
                    );
                    ((2, 1), None)
                }
                Self::MissingOutputIdentity => {
                    state.set_pending_portal_image(
                        image(2, 1, 8, 8),
                        None,
                        Some(desktop_geometry(0, 0, 2, 1, 1, Some((0, 0)))),
                    );
                    ((2, 1), None)
                }
                Self::AspectMismatch => {
                    state.set_pending_output_image(
                        image(10, 1, 40, 40),
                        7,
                        verified_output_geometry((10, 1), 1, wl_output::Transform::Normal, (10, 1)),
                    );
                    ((1, 10), None)
                }
            }
        }
    }

    #[test]
    fn every_pending_image_rejection_finishes_its_acquisition_exactly_once() {
        for fixture in PendingImageRejectionFixture::ALL {
            let mut state = FrozenState::new_inner(None, None, true, None, true, false);
            let mut input_state = make_test_input_state();
            let mut registry = ScreenAcquisitionRegistry::default();
            let owner = ScreenAcquisitionOwner::UserFreeze;
            let id = registry.request(owner).expect("id");
            let retained_provenance =
                ScreenImageProvenance::new(42, 9, 1, wl_output::Transform::Normal)
                    .expect("valid retained image provenance");
            state.set_image_with_provenance_for_test(
                FrozenImage {
                    width: 2,
                    height: 1,
                    stride: 8,
                    data: vec![1; 8],
                },
                retained_provenance,
            );
            let retained_generation = state.image_generation();
            input_state.set_frozen_active(true);
            state.set_active_output(None, fixture.active_output_id());
            state
                .start_capture_for(id, owner)
                .expect("capture attempt starts");
            let ((phys_width, phys_height), live_output_count) =
                fixture.install_pending(&mut state);

            let message = state
                .activate_pending_image_with_live_outputs(
                    phys_width,
                    phys_height,
                    &mut input_state,
                    live_output_count,
                )
                .expect_err("pending image fixture must be rejected");

            if matches!(
                fixture,
                PendingImageRejectionFixture::MalformedTransformBuffer
            ) {
                assert!(
                    message.starts_with(fixture.expected_error()),
                    "{fixture:?} returned {message:?}"
                );
            } else {
                assert_eq!(message, fixture.expected_error(), "{fixture:?}");
            }
            assert_eq!(state.attempt(), None, "{fixture:?}");
            assert!(!state.has_pending_image(), "{fixture:?}");
            assert!(!state.is_in_progress(), "{fixture:?}");
            assert_eq!(state.image_generation(), retained_generation, "{fixture:?}");
            assert_eq!(
                state.image_provenance(),
                Some(retained_provenance),
                "{fixture:?}"
            );
            let retained = state.image().expect("the old image remains installed");
            assert_eq!(
                (retained.width, retained.height, retained.stride),
                (2, 1, 8)
            );
            assert_eq!(retained.data, vec![1; 8], "{fixture:?}");
            assert!(!input_state.frozen_active(), "{fixture:?}");
            assert!(state.take_capture_done(), "{fixture:?}");
            assert!(!state.take_capture_done(), "{fixture:?}");

            assert_eq!(
                state.take_matching_acquisition_completion(id, owner),
                Some(ScreenAcquisitionCompletion {
                    id,
                    owner,
                    outcome: ScreenAcquisitionOutcome::Failed(message),
                }),
                "{fixture:?}"
            );
            assert_eq!(
                state.take_matching_acquisition_completion(id, owner),
                None,
                "{fixture:?} published more than one terminal"
            );
            assert_eq!(state.take_acquisition_completion(), None, "{fixture:?}");
        }
    }

    #[test]
    fn undrained_terminal_is_taken_only_by_its_correlated_owner() {
        let mut state = FrozenState::new_inner(None, None, true, None, true, false);
        let mut input_state = make_test_input_state();
        let mut registry = ScreenAcquisitionRegistry::default();
        let id = registry.request(ScreenAcquisitionOwner::Ocr).expect("id");
        state
            .start_capture_for(id, ScreenAcquisitionOwner::Ocr)
            .expect("capture starts");
        state.finish_acquisition(ScreenAcquisitionOutcome::Cancelled, &mut input_state);

        assert_eq!(
            state.take_matching_acquisition_completion(id, ScreenAcquisitionOwner::Eyedropper),
            None
        );
        assert!(state.acquisition_completion().is_some());
        assert_eq!(
            state
                .take_matching_acquisition_completion(id, ScreenAcquisitionOwner::Ocr)
                .map(|completion| completion.outcome),
            Some(ScreenAcquisitionOutcome::Cancelled)
        );
        assert!(state.acquisition_completion().is_none());
    }

    #[test]
    fn undrained_ready_terminal_transfers_its_exact_generation_once() {
        let mut state = FrozenState::new_inner(None, None, true, None, true, false);
        let mut input_state = make_test_input_state();
        let mut registry = ScreenAcquisitionRegistry::default();
        let id = registry
            .request(ScreenAcquisitionOwner::Eyedropper)
            .expect("id");
        state.set_image(FrozenImage {
            width: 1,
            height: 1,
            stride: 4,
            data: vec![7; 4],
        });
        let generation = state.image_generation();
        input_state.set_frozen_active(true);
        state
            .start_capture_for(id, ScreenAcquisitionOwner::Eyedropper)
            .expect("capture starts");

        state.finish_ready_acquisition(&mut input_state);

        assert_eq!(
            state.take_matching_acquisition_completion(id, ScreenAcquisitionOwner::Eyedropper,),
            Some(ScreenAcquisitionCompletion {
                id,
                owner: ScreenAcquisitionOwner::Eyedropper,
                outcome: ScreenAcquisitionOutcome::Ready {
                    installed_generation: generation,
                },
            })
        );
        assert_eq!(
            state.take_matching_acquisition_completion(id, ScreenAcquisitionOwner::Eyedropper,),
            None,
            "a second cancellation cannot consume or release the generation again"
        );
        assert!(state.take_capture_done());
        assert!(!state.take_capture_done());
        assert!(input_state.frozen_active());
    }

    #[test]
    fn preflight_layout_failure_is_classified_as_stale_layout() {
        let mut state = FrozenState::new_inner(None, None, true, None, true, false);
        let mut input_state = make_test_input_state();
        let mut registry = ScreenAcquisitionRegistry::default();
        let id = registry
            .request(ScreenAcquisitionOwner::UserFreeze)
            .expect("id");
        state
            .start_capture_for(id, ScreenAcquisitionOwner::UserFreeze)
            .expect("capture starts");
        state.set_active_geometry(Some(verified_output_geometry(
            (1, 1),
            1,
            wl_output::Transform::Normal,
            (1, 1),
        )));

        let error = state
            .ensure_preflight_layout_current()
            .map_err(anyhow::Error::msg)
            .unwrap_err();

        state
            .finish_preflight_failure(CapturePreflightError::from_backend(error), &mut input_state);

        assert_eq!(
            state
                .take_acquisition_completion()
                .map(|completion| completion.outcome),
            Some(ScreenAcquisitionOutcome::StaleLayout)
        );
    }

    #[test]
    fn unowned_preflight_failure_clears_retry_and_publishes_capture_done() {
        let mut state = FrozenState::new_inner(None, None, true, None, true, false);
        let mut input_state = make_test_input_state();
        state.set_active_output(None, Some(1));
        state.start_capture().unwrap();
        state.take_preflight_pending();
        assert!(state.queue_portal_layout_retry(Some(1), true));

        state.finish_preflight_failure(CapturePreflightError::LostSuppression, &mut input_state);

        assert!(!state.has_portal_layout_retry());
        assert!(!state.is_in_progress());
        assert!(state.take_capture_done());
        assert!(state.take_acquisition_completion().is_none());
        assert!(input_state.needs_redraw);
    }
}
