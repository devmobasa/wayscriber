use log::info;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wayland_client::protocol::wl_output;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::backend::wayland::RuntimeWakeHandle;
use crate::backend::wayland::acquisition::{
    ScreenAcquisitionCompletion, ScreenAcquisitionId, ScreenAcquisitionOutcome,
    ScreenAcquisitionOwner,
};
use crate::backend::wayland::capture::CaptureLayoutContext;
use crate::backend::wayland::frozen::{FrozenImage, ScreenImageProvenance};
use crate::backend::wayland::frozen_geometry::OutputGeometry;
use crate::backend::wayland::portal_capture::{crop_argb, layout_token_matches};
use crate::backend::wayland::portal_task::PortalTask;
use crate::input::InputState;
use crate::input::state::{Toast, ToastPriority};

use super::PortalCaptureResult;
use super::capture::CaptureSession;
use super::ext_image_copy::{ExtImageCopyManagers, ExtImageCopySession};

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
    Desktop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum FrozenCaptureBackend {
    WlrScreencopy,
    ExtImageCopy,
    Portal,
}

pub(super) enum DirectCaptureAttempt {
    WlrScreencopy {
        session: Box<CaptureSession>,
        context: DirectCaptureContext,
    },
    ExtImageCopy {
        session: Box<ExtImageCopySession>,
        context: DirectCaptureContext,
    },
}

impl DirectCaptureAttempt {
    fn backend(&self) -> FrozenCaptureBackend {
        match self {
            Self::WlrScreencopy { .. } => FrozenCaptureBackend::WlrScreencopy,
            Self::ExtImageCopy { .. } => FrozenCaptureBackend::ExtImageCopy,
        }
    }

    fn context(&self) -> &DirectCaptureContext {
        match self {
            Self::WlrScreencopy { context, .. } | Self::ExtImageCopy { context, .. } => context,
        }
    }

    fn destroy(self) {
        match self {
            Self::WlrScreencopy { session, .. } => session.frame.destroy(),
            Self::ExtImageCopy { session, .. } => (*session).destroy(),
        }
    }
}

pub(super) const DIRECT_CAPTURE_TIMEOUT: Duration = Duration::from_secs(3);

pub(super) struct DirectCaptureContext {
    pub(super) layout: CaptureLayoutContext,
    pub(super) source_geometry: OutputGeometry,
    started_at: Instant,
}

impl DirectCaptureContext {
    pub(super) fn new(layout: CaptureLayoutContext, source_geometry: OutputGeometry) -> Self {
        Self::new_at(layout, source_geometry, Instant::now())
    }

    fn new_at(
        layout: CaptureLayoutContext,
        source_geometry: OutputGeometry,
        started_at: Instant,
    ) -> Self {
        Self {
            layout,
            source_geometry,
            started_at,
        }
    }

    fn timeout(&self, now: Instant) -> Duration {
        self.started_at
            .checked_add(DIRECT_CAPTURE_TIMEOUT)
            .map(|deadline| deadline.saturating_duration_since(now))
            .unwrap_or(Duration::ZERO)
    }
}

/// End-to-end controller for frozen mode capture and image storage.
#[allow(clippy::type_complexity)]
pub struct FrozenState {
    pub(super) manager: Option<ZwlrScreencopyManagerV1>,
    pub(super) ext_managers: Option<ExtImageCopyManagers>,
    pub(super) portal_available: bool,
    pub(super) active_output: Option<wl_output::WlOutput>,
    pub(super) active_output_id: Option<u32>,
    pub(super) active_geometry: Option<OutputGeometry>,
    /// Bumped when freeze/zoom crop geometry actually changes so in-flight
    /// portal captures can be rejected after a layout change.
    pub(super) output_layout_generation: u64,
    pub(super) direct_capture: Option<DirectCaptureAttempt>,
    pub(super) image: Option<Arc<FrozenImage>>,
    image_provenance: Option<ScreenImageProvenance>,
    image_target_dimensions: Option<(u32, u32)>,
    image_generation: u64,
    pub(super) portal_task: Option<PortalTask<PortalCaptureResult>>,
    pub(super) portal_in_progress: bool,
    pub(super) portal_target_output_id: Option<u32>,
    pub(super) runtime_wake: Option<RuntimeWakeHandle>,
    pub(super) preflight_pending: bool,
    pub(super) preflight_backend: Option<FrozenCaptureBackend>,
    preflight_output_id: Option<u32>,
    preflight_layout_generation: Option<u64>,
    pub(super) capture_done: bool,
    pending_image: Option<PendingFrozenImage>,
    acquisition_attempt: Option<(ScreenAcquisitionId, ScreenAcquisitionOwner)>,
    acquisition_completion: Option<ScreenAcquisitionCompletion>,
}

impl FrozenState {
    #[cfg(test)]
    pub fn new(manager: Option<ZwlrScreencopyManagerV1>) -> Self {
        Self::new_inner(manager, None, false, None)
    }

    #[cfg(test)]
    pub(in crate::backend::wayland) fn new_with_runtime_wake(
        manager: Option<ZwlrScreencopyManagerV1>,
        runtime_wake: RuntimeWakeHandle,
    ) -> Self {
        Self::new_inner(manager, None, true, Some(runtime_wake))
    }

    pub(in crate::backend::wayland) fn new_with_backends(
        manager: Option<ZwlrScreencopyManagerV1>,
        ext_managers: Option<ExtImageCopyManagers>,
        portal_available: bool,
        runtime_wake: RuntimeWakeHandle,
    ) -> Self {
        Self::new_inner(manager, ext_managers, portal_available, Some(runtime_wake))
    }

    fn new_inner(
        manager: Option<ZwlrScreencopyManagerV1>,
        ext_managers: Option<ExtImageCopyManagers>,
        portal_available: bool,
        runtime_wake: Option<RuntimeWakeHandle>,
    ) -> Self {
        Self {
            manager,
            ext_managers,
            portal_available,
            active_output: None,
            active_output_id: None,
            active_geometry: None,
            output_layout_generation: 0,
            direct_capture: None,
            image: None,
            image_provenance: None,
            image_target_dimensions: None,
            image_generation: 0,
            portal_task: None,
            portal_in_progress: false,
            portal_target_output_id: None,
            runtime_wake,
            preflight_pending: false,
            preflight_backend: None,
            preflight_output_id: None,
            preflight_layout_generation: None,
            capture_done: false,
            pending_image: None,
            acquisition_attempt: None,
            acquisition_completion: None,
        }
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
        if self.active_geometry != geometry {
            self.output_layout_generation = self.output_layout_generation.wrapping_add(1);
        }
        self.active_geometry = geometry;
    }

    pub(in crate::backend::wayland) fn output_layout_generation(&self) -> u64 {
        self.output_layout_generation
    }

    pub(in crate::backend::wayland) fn source_context_matches(
        &self,
        provenance: ScreenImageProvenance,
    ) -> bool {
        self.active_output_id == Some(provenance.output_id)
            && self.output_layout_generation == provenance.output_layout_generation
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

    pub fn set_pending_output_image(
        &mut self,
        image: FrozenImage,
        target_output_id: u32,
        source_geometry: OutputGeometry,
    ) {
        self.pending_image = Some(PendingFrozenImage {
            image,
            target_output_id: Some(target_output_id),
            layout_generation: self.output_layout_generation,
            source_geometry: Some(source_geometry),
            output_transform: None,
            source: FrozenCaptureSource::ActiveOutput,
        });
    }

    pub(super) fn set_pending_output_image_with_transform(
        &mut self,
        image: FrozenImage,
        target_output_id: u32,
        source_geometry: OutputGeometry,
        output_transform: Option<wl_output::Transform>,
    ) {
        self.pending_image = Some(PendingFrozenImage {
            image,
            target_output_id: Some(target_output_id),
            layout_generation: self.output_layout_generation,
            source_geometry: Some(source_geometry),
            output_transform,
            source: FrozenCaptureSource::ActiveOutput,
        });
    }

    pub fn set_pending_desktop_image(
        &mut self,
        image: FrozenImage,
        target_output_id: Option<u32>,
        source_geometry: Option<OutputGeometry>,
    ) {
        self.pending_image = Some(PendingFrozenImage {
            image,
            target_output_id,
            layout_generation: self.output_layout_generation,
            source_geometry,
            output_transform: None,
            source: FrozenCaptureSource::Desktop,
        });
    }

    pub fn has_pending_image(&self) -> bool {
        self.pending_image.is_some()
    }

    pub fn is_in_progress(&self) -> bool {
        self.direct_capture.is_some()
            || self.portal_in_progress
            || self.preflight_pending
            || self.pending_image.is_some()
    }

    pub(in crate::backend::wayland) fn take_preflight_pending(
        &mut self,
    ) -> Option<FrozenCaptureBackend> {
        if !self.preflight_pending {
            return None;
        }
        self.preflight_pending = false;
        self.preflight_backend.take()
    }

    pub(super) fn snapshot_preflight_layout(&mut self) {
        self.preflight_output_id = self.active_output_id;
        self.preflight_layout_generation = Some(self.output_layout_generation);
    }

    pub(super) fn ensure_preflight_layout_current(&self) -> Result<(), String> {
        let Some(generation) = self.preflight_layout_generation else {
            return Ok(());
        };
        if layout_token_matches(
            self.preflight_output_id,
            generation,
            self.active_output_id,
            self.output_layout_generation,
        ) {
            Ok(())
        } else {
            Err("Freeze failed after the display layout changed".to_string())
        }
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
        let stale_message = self.ensure_preflight_layout_current().err();
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
        message: String,
        input_state: &mut InputState,
    ) {
        let outcome = if self.ensure_preflight_layout_current().is_err() {
            ScreenAcquisitionOutcome::StaleLayout
        } else {
            ScreenAcquisitionOutcome::Failed(message)
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
        if let Some(capture) = self.direct_capture.take() {
            capture.destroy();
        }
        self.preflight_pending = false;
        self.preflight_backend = None;
        self.preflight_output_id = None;
        self.preflight_layout_generation = None;
        self.portal_in_progress = false;
        if let Some(mut task) = self.portal_task.take() {
            task.cancel();
        }
        self.portal_target_output_id = None;
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

    #[cfg(test)]
    pub fn activate_pending_image(
        &mut self,
        phys_width: u32,
        phys_height: u32,
        input_state: &mut InputState,
    ) -> Result<bool, String> {
        self.activate_pending_image_with_live_outputs(phys_width, phys_height, input_state, None)
    }

    pub fn activate_pending_image_with_live_outputs(
        &mut self,
        phys_width: u32,
        phys_height: u32,
        input_state: &mut InputState,
        live_output_count: Option<u32>,
    ) -> Result<bool, String> {
        let Some(pending) = self.pending_image.take() else {
            return Ok(false);
        };
        if !layout_token_matches(
            pending.target_output_id,
            pending.layout_generation,
            self.active_output_id,
            self.output_layout_generation,
        ) {
            return self.reject_pending_image(
                input_state,
                "Freeze failed after the display layout changed",
            );
        }
        let mut image = pending.image;
        let mut provenance = None;

        if matches!(pending.source, FrozenCaptureSource::ActiveOutput) {
            let Some(geometry) = pending.source_geometry.as_ref() else {
                return self
                    .reject_pending_image(input_state, "Freeze capture geometry is unavailable");
            };
            let output_transform = pending.output_transform.unwrap_or(geometry.transform);
            provenance = pending.target_output_id.and_then(|output_id| {
                ScreenImageProvenance::new(
                    output_id,
                    pending.layout_generation,
                    geometry.scale,
                    output_transform,
                )
            });
            image = match image.with_output_transform(output_transform) {
                Ok(image) => image,
                Err(error) => {
                    return self.reject_pending_image(
                        input_state,
                        format!("Freeze capture transform failed: {error}"),
                    );
                }
            };
            if !geometry.accepts_transformed_pixel_size(image.width, image.height) {
                return self.reject_pending_image(
                    input_state,
                    "Freeze capture dimensions do not match the active output",
                );
            }
        }

        if matches!(pending.source, FrozenCaptureSource::Desktop) {
            let Some(geometry) = pending
                .source_geometry
                .as_ref()
                .cloned()
                .and_then(|geometry| geometry.with_revalidated_output_count(live_output_count))
            else {
                return self.reject_pending_image(
                    input_state,
                    "Freeze failed after the output layout changed",
                );
            };
            provenance = pending.target_output_id.and_then(|output_id| {
                ScreenImageProvenance::new(
                    output_id,
                    pending.layout_generation,
                    geometry.scale,
                    geometry.transform,
                )
            });
            let Some((capture_width, capture_height)) = geometry.verified_pixel_size() else {
                return self.reject_pending_image(
                    input_state,
                    "Freeze failed after the display changed size",
                );
            };
            let Some(cropped) =
                self.crop_pending_image(image, &geometry, capture_width, capture_height)
            else {
                return self.reject_pending_image(
                    input_state,
                    "Freeze failed after the display changed size",
                );
            };
            image = cropped;
        }

        let Some(provenance) = provenance else {
            return self.reject_pending_image(
                input_state,
                "Freeze capture source identity is unavailable",
            );
        };

        if !OutputGeometry::dimensions_have_compatible_aspect(
            (image.width, image.height),
            (phys_width, phys_height),
        ) {
            return self.reject_pending_image(
                input_state,
                "Freeze capture aspect does not match the overlay surface",
            );
        }

        self.image_target_dimensions = Some((phys_width, phys_height));
        self.image = Some(Arc::new(image));
        self.image_provenance = Some(provenance);
        self.bump_image_generation();
        input_state.set_frozen_active(true);
        input_state.dirty_tracker.mark_full();
        input_state.needs_redraw = true;
        self.finish_ready_acquisition(input_state);
        // Legacy tests and late callbacks without an acquisition still use the
        // capture-done wakeup as their resource-restoration boundary.
        self.capture_done = true;
        Ok(true)
    }

    fn reject_pending_image(
        &mut self,
        input_state: &mut InputState,
        error: impl Into<String>,
    ) -> Result<bool, String> {
        let error = error.into();
        self.finish_acquisition(ScreenAcquisitionOutcome::Failed(error.clone()), input_state);
        self.capture_done = true;
        input_state.set_frozen_active(false);
        input_state.needs_redraw = true;
        Err(error)
    }

    fn crop_pending_image(
        &self,
        image: FrozenImage,
        geometry: &OutputGeometry,
        target_width: u32,
        target_height: u32,
    ) -> Option<FrozenImage> {
        if target_width == 0 || target_height == 0 {
            return None;
        }
        let (origin_x, origin_y) = geometry.portal_crop_origin(image.width, image.height)?;
        let (width, height, data) = crop_argb(
            &image.data,
            image.width,
            image.height,
            origin_x,
            origin_y,
            target_width,
            target_height,
        )?;
        if width != target_width || height != target_height {
            return None;
        }
        let stride = i32::try_from(target_width.checked_mul(4)?).ok()?;
        Some(FrozenImage {
            width: target_width,
            height: target_height,
            stride,
            data,
        })
    }

    /// Drop frozen image if the surface size no longer matches.
    pub fn handle_resize(
        &mut self,
        phys_width: u32,
        phys_height: u32,
        input_state: &mut InputState,
    ) {
        if let Some(target_dimensions) = self.image_target_dimensions
            && target_dimensions != (phys_width, phys_height)
        {
            info!("Surface resized; clearing frozen image");
            self.clear_image();
            input_state.set_frozen_active(false);
        }
    }

    /// Toggle unfreeze: drop the image and mark redraw.
    pub fn unfreeze(&mut self, input_state: &mut InputState) {
        self.clear_image();
        input_state.set_frozen_active(false);
        input_state.dirty_tracker.mark_full();
        input_state.needs_redraw = true;
    }

    pub fn cancel(&mut self, input_state: &mut InputState) {
        self.abandon_acquisition(input_state);
    }

    fn clear_image(&mut self) -> bool {
        let had_image = self.image.take().is_some();
        self.image_provenance = None;
        self.image_target_dimensions = None;
        if had_image {
            self.bump_image_generation();
        }
        had_image
    }

    fn bump_image_generation(&mut self) {
        self.image_generation = self.image_generation.wrapping_add(1).max(1);
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
    fn direct_capture_context_tracks_its_deadline_and_output_identity() {
        let started_at = Instant::now();
        let geometry = OutputGeometry::update_from(
            Some((0, 0)),
            Some((1, 1)),
            (1, 1),
            1,
            wl_output::Transform::Normal,
            Some((1, 1)),
        )
        .expect("geometry");
        let capture =
            DirectCaptureContext::new_at(CaptureLayoutContext::new(7, 3), geometry, started_at);

        assert_eq!(capture.timeout(started_at), DIRECT_CAPTURE_TIMEOUT);
        assert_eq!(
            capture.timeout(started_at + DIRECT_CAPTURE_TIMEOUT),
            Duration::ZERO
        );
        assert!(capture.layout.matches(Some(7), 3));
        assert!(!capture.layout.matches(Some(8), 3));
        assert!(!capture.layout.matches(None, 3));
        assert!(!capture.layout.matches(Some(7), 4));
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

    #[test]
    fn desktop_capture_still_requires_a_crop_covering_the_target() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.set_pending_desktop_image(
            FrozenImage {
                width: 4,
                height: 3,
                stride: 16,
                data: vec![0; 4 * 3 * 4],
            },
            None,
            None,
        );

        assert!(
            state
                .activate_pending_image(6, 4, &mut input_state)
                .is_err()
        );
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
        }
    }

    #[test]
    fn output_layout_generation_bumps_only_when_geometry_changes() {
        let mut state = FrozenState::new(None);
        assert_eq!(state.output_layout_generation, 0);
        let first = desktop_geometry(0, 0, 4, 1, 1, Some((0, 0)));
        state.set_active_geometry(Some(first.clone()));
        assert_eq!(state.output_layout_generation, 1);
        state.set_active_geometry(Some(first));
        assert_eq!(state.output_layout_generation, 1);
        state.set_active_geometry(Some(desktop_geometry(0, 0, 4, 1, 1, Some((6, 0)))));
        assert_eq!(state.output_layout_generation, 2);
    }

    #[test]
    fn desktop_capture_crops_from_screenshot_origin_not_logical_times_scale() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.set_active_output(None, Some(7));
        let mut data = Vec::new();
        for pixel in 0u8..10 {
            data.extend_from_slice(&[pixel, pixel, pixel, 255]);
        }
        let geometry = desktop_geometry(0, 0, 4, 1, 1, Some((6, 0)));
        assert_eq!(geometry.physical_origin(), (0, 0));

        state.set_pending_desktop_image(
            FrozenImage {
                width: 10,
                height: 1,
                stride: 40,
                data,
            },
            Some(7),
            Some(geometry),
        );

        state
            .activate_pending_image(4, 1, &mut input_state)
            .expect("mixed-scale screenshot origin should crop the active output");

        let image = state.image().expect("the frozen image should be active");
        assert_eq!(
            state.image_provenance(),
            ScreenImageProvenance::new(7, 0, 1, wl_output::Transform::Normal)
        );
        assert_eq!(
            image.data,
            vec![6, 6, 6, 255, 7, 7, 7, 255, 8, 8, 8, 255, 9, 9, 9, 255]
        );
    }

    #[test]
    fn desktop_capture_keeps_fractional_output_pixels_for_a_larger_overlay_buffer() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.set_active_output(None, Some(7));
        let mut geometry = desktop_geometry(0, 0, 3, 2, 2, Some((0, 0)));
        geometry.pixel_size = Some((5, 3));
        state.set_pending_desktop_image(
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
    fn desktop_capture_rejects_a_portal_image_from_a_different_layout_size() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        let geometry = desktop_geometry(0, 0, 4, 1, 1, None).with_desktop_backdrop_geometry(Some(
            crate::capture::DesktopBackdropGeometry {
                logical_x: 0,
                logical_y: 0,
                logical_width: 4,
                logical_height: 1,
                physical_width: Some(4),
                physical_height: Some(1),
                crop_x: Some(6),
                crop_y: Some(0),
                screenshot_width: Some(10),
                screenshot_height: Some(1),
            },
        ));
        state.set_pending_desktop_image(
            FrozenImage {
                width: 11,
                height: 1,
                stride: 44,
                data: vec![0; 11 * 4],
            },
            None,
            Some(geometry),
        );

        assert!(
            state
                .activate_pending_image(4, 1, &mut input_state)
                .is_err()
        );
        assert!(state.image().is_none());
        assert!(!input_state.frozen_active());
    }

    #[test]
    fn desktop_capture_fails_closed_when_screenshot_origin_is_unknown() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        let geometry = desktop_geometry(10, 20, 2, 1, 2, None);
        assert_eq!(geometry.physical_origin(), (20, 40));
        assert_eq!(geometry.portal_crop_origin(4, 2), None);
        assert_eq!(geometry.portal_crop_origin(10, 2), None);

        state.set_pending_desktop_image(
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
                .activate_pending_image(4, 2, &mut input_state)
                .is_err()
        );
        assert!(state.image().is_none());
        assert!(!input_state.frozen_active());
    }

    #[test]
    fn desktop_capture_crops_at_buffer_origin_for_a_proven_single_output() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.set_active_output(None, Some(7));
        let geometry = desktop_geometry(10, 20, 2, 1, 2, None).with_known_output_count(Some(1));
        assert_eq!(geometry.portal_crop_origin(4, 2), Some((0, 0)));

        state.set_pending_desktop_image(
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
            .expect(
                "a single-output desktop shot without zxdg layout still crops at the buffer origin",
            );
        assert!(state.image().is_some());
        assert!(input_state.frozen_active());
    }

    #[test]
    fn desktop_capture_refuses_activation_without_capture_time_output_identity() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        let geometry = desktop_geometry(0, 0, 2, 1, 1, Some((0, 0)));
        state.set_pending_desktop_image(
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

        state.set_pending_desktop_image(
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
    fn desktop_capture_fails_when_screenshot_origin_is_unknown_and_image_is_larger() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        let geometry = desktop_geometry(10, 20, 2, 1, 2, None);

        state.set_pending_desktop_image(
            FrozenImage {
                width: 10,
                height: 2,
                stride: 40,
                data: vec![7; 10 * 2 * 4],
            },
            None,
            Some(geometry),
        );

        assert!(
            state
                .activate_pending_image(4, 2, &mut input_state)
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
        state.portal_in_progress = true;

        state.cancel(&mut input_state);

        assert!(!state.is_in_progress());
        assert!(state.take_capture_done());
    }

    #[test]
    fn acquisition_terminal_consumes_attempt_once_and_retains_old_image_on_failure() {
        let mut state = FrozenState::new_inner(None, None, true, None);
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
        DesktopCropFailure,
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
            Self::DesktopCropFailure,
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
                Self::MissingVerifiedPixelSize | Self::DesktopCropFailure => {
                    "Freeze failed after the display changed size"
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
                    state.set_pending_desktop_image(image(2, 1, 8, 8), Some(7), None);
                    ((2, 1), None)
                }
                Self::StaleDesktopGeometry => {
                    state.set_pending_desktop_image(
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
                    state.set_pending_desktop_image(image(2, 1, 8, 8), Some(7), Some(geometry));
                    ((2, 1), None)
                }
                Self::DesktopCropFailure => {
                    state.set_pending_desktop_image(
                        image(2, 1, 8, 8),
                        Some(7),
                        Some(desktop_geometry(0, 0, 2, 1, 1, Some((3, 0)))),
                    );
                    ((2, 1), None)
                }
                Self::MissingOutputIdentity => {
                    state.set_pending_desktop_image(
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
            let mut state = FrozenState::new_inner(None, None, true, None);
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
        let mut state = FrozenState::new_inner(None, None, true, None);
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
        let mut state = FrozenState::new_inner(None, None, true, None);
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
        let mut state = FrozenState::new_inner(None, None, true, None);
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

        state.finish_preflight_failure("backend refused".to_string(), &mut input_state);

        assert_eq!(
            state
                .take_acquisition_completion()
                .map(|completion| completion.outcome),
            Some(ScreenAcquisitionOutcome::StaleLayout)
        );
    }
}
