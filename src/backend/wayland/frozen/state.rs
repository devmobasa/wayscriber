use log::info;
use std::time::{Duration, Instant};
use wayland_client::protocol::wl_output;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::backend::wayland::RuntimeWakeHandle;
use crate::backend::wayland::frozen::FrozenImage;
use crate::backend::wayland::frozen_geometry::OutputGeometry;
use crate::backend::wayland::portal_capture::crop_argb;
use crate::backend::wayland::portal_task::PortalTask;
use crate::input::InputState;

use super::PortalCaptureResult;
use super::capture::CaptureSession;
use super::ext_image_copy::{ExtImageCopyManagers, ExtImageCopySession};

struct PendingFrozenImage {
    image: FrozenImage,
    source_geometry: Option<OutputGeometry>,
    output_transform: Option<wl_output::Transform>,
    needs_output_transform: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DirectCaptureBackend {
    WlrScreencopy,
    ExtImageCopy,
}

impl DirectCaptureBackend {
    fn capture_backend(self) -> FrozenCaptureBackend {
        match self {
            Self::WlrScreencopy => FrozenCaptureBackend::WlrScreencopy,
            Self::ExtImageCopy => FrozenCaptureBackend::ExtImageCopy,
        }
    }
}

pub(super) const DIRECT_CAPTURE_TIMEOUT: Duration = Duration::from_secs(3);

pub(super) struct DirectCaptureContext {
    pub(super) backend: DirectCaptureBackend,
    pub(super) target_output_id: u32,
    pub(super) source_geometry: Option<OutputGeometry>,
    started_at: Instant,
}

impl DirectCaptureContext {
    pub(super) fn new(
        backend: DirectCaptureBackend,
        target_output_id: u32,
        source_geometry: Option<OutputGeometry>,
    ) -> Self {
        Self::new_at(backend, target_output_id, source_geometry, Instant::now())
    }

    fn new_at(
        backend: DirectCaptureBackend,
        target_output_id: u32,
        source_geometry: Option<OutputGeometry>,
        started_at: Instant,
    ) -> Self {
        Self {
            backend,
            target_output_id,
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

    pub(super) fn output_matches(&self, current_output_id: Option<u32>) -> bool {
        current_output_id == Some(self.target_output_id)
    }
}

/// End-to-end controller for frozen mode capture and image storage.
#[allow(clippy::type_complexity)]
pub struct FrozenState {
    pub(super) manager: Option<ZwlrScreencopyManagerV1>,
    pub(super) ext_managers: Option<ExtImageCopyManagers>,
    pub(super) ext_capture: Option<ExtImageCopySession>,
    pub(super) portal_available: bool,
    pub(super) active_output: Option<wl_output::WlOutput>,
    pub(super) active_output_id: Option<u32>,
    pub(super) active_geometry: Option<OutputGeometry>,
    pub(super) capture: Option<CaptureSession>,
    pub(super) direct_capture: Option<DirectCaptureContext>,
    pub(super) image: Option<FrozenImage>,
    image_target_dimensions: Option<(u32, u32)>,
    image_generation: u64,
    pub(super) portal_task: Option<PortalTask<PortalCaptureResult>>,
    pub(super) portal_in_progress: bool,
    pub(super) portal_target_output_id: Option<u32>,
    pub(super) runtime_wake: Option<RuntimeWakeHandle>,
    pub(super) preflight_pending: bool,
    pub(super) preflight_backend: Option<FrozenCaptureBackend>,
    pub(super) capture_done: bool,
    pending_image: Option<PendingFrozenImage>,
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
            ext_capture: None,
            portal_available,
            active_output: None,
            active_output_id: None,
            active_geometry: None,
            capture: None,
            direct_capture: None,
            image: None,
            image_target_dimensions: None,
            image_generation: 0,
            portal_task: None,
            portal_in_progress: false,
            portal_target_output_id: None,
            runtime_wake,
            preflight_pending: false,
            preflight_backend: None,
            capture_done: false,
            pending_image: None,
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
        self.active_geometry = geometry;
    }

    pub fn active_output_matches(&self, info_id: u32) -> bool {
        self.active_output_id == Some(info_id)
    }

    pub fn image(&self) -> Option<&FrozenImage> {
        self.image.as_ref()
    }

    pub fn image_generation(&self) -> u64 {
        self.image_generation
    }

    #[cfg(test)]
    pub fn set_image(&mut self, image: FrozenImage) {
        self.image_target_dimensions = Some((image.width, image.height));
        self.image = Some(image);
        self.bump_image_generation();
    }

    pub fn set_pending_output_image(
        &mut self,
        image: FrozenImage,
        source_geometry: Option<OutputGeometry>,
    ) {
        self.pending_image = Some(PendingFrozenImage {
            image,
            source_geometry,
            output_transform: None,
            needs_output_transform: true,
            source: FrozenCaptureSource::ActiveOutput,
        });
    }

    pub(super) fn set_pending_output_image_with_transform(
        &mut self,
        image: FrozenImage,
        source_geometry: Option<OutputGeometry>,
        output_transform: Option<wl_output::Transform>,
    ) {
        self.pending_image = Some(PendingFrozenImage {
            image,
            source_geometry,
            output_transform,
            needs_output_transform: true,
            source: FrozenCaptureSource::ActiveOutput,
        });
    }

    pub fn set_pending_desktop_image(
        &mut self,
        image: FrozenImage,
        source_geometry: Option<OutputGeometry>,
    ) {
        self.pending_image = Some(PendingFrozenImage {
            image,
            source_geometry,
            output_transform: None,
            needs_output_transform: false,
            source: FrozenCaptureSource::Desktop,
        });
    }

    pub fn has_pending_image(&self) -> bool {
        self.pending_image.is_some()
    }

    pub fn is_in_progress(&self) -> bool {
        self.capture.is_some()
            || self.ext_capture.is_some()
            || self.direct_capture.is_some()
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

    pub fn take_capture_done(&mut self) -> bool {
        let done = self.capture_done;
        self.capture_done = false;
        done
    }

    pub(in crate::backend::wayland) fn direct_capture_timeout(
        &self,
        now: Instant,
    ) -> Option<Duration> {
        self.direct_capture
            .as_ref()
            .map(|capture| capture.timeout(now))
    }

    pub(in crate::backend::wayland) fn take_timed_out_direct_capture(
        &mut self,
        now: Instant,
    ) -> Option<FrozenCaptureBackend> {
        let capture = self.direct_capture.as_ref()?;
        if !capture.timeout(now).is_zero() {
            return None;
        }
        let backend = capture.backend;
        self.direct_capture = None;
        match backend {
            DirectCaptureBackend::WlrScreencopy => {
                if let Some(capture) = self.capture.take() {
                    capture.frame.destroy();
                }
            }
            DirectCaptureBackend::ExtImageCopy => {
                if let Some(capture) = self.ext_capture.take() {
                    capture.destroy();
                }
            }
        }
        Some(backend.capture_backend())
    }

    pub fn activate_pending_image(
        &mut self,
        phys_width: u32,
        phys_height: u32,
        input_state: &mut InputState,
    ) -> Result<bool, String> {
        let Some(pending) = self.pending_image.take() else {
            return Ok(false);
        };
        let mut image = pending.image;

        if pending.needs_output_transform {
            let output_transform = pending.output_transform.unwrap_or_else(|| {
                pending
                    .source_geometry
                    .as_ref()
                    .or(self.active_geometry.as_ref())
                    .map(|geo| geo.transform)
                    .unwrap_or(wl_output::Transform::Normal)
            });
            image = image.with_output_transform(output_transform);
        }

        if matches!(pending.source, FrozenCaptureSource::Desktop)
            && (image.width != phys_width || image.height != phys_height)
        {
            let Some(cropped) = self.crop_pending_image(
                image,
                pending.source_geometry.as_ref(),
                phys_width,
                phys_height,
            ) else {
                self.capture_done = true;
                input_state.set_frozen_active(false);
                input_state.needs_redraw = true;
                return Err("Freeze failed after the display changed size".to_string());
            };
            image = cropped;
        }

        self.image_target_dimensions = Some((phys_width, phys_height));
        self.image = Some(image);
        self.bump_image_generation();
        input_state.set_frozen_active(true);
        input_state.dirty_tracker.mark_full();
        input_state.needs_redraw = true;
        self.capture_done = true;
        Ok(true)
    }

    fn crop_pending_image(
        &self,
        image: FrozenImage,
        source_geometry: Option<&OutputGeometry>,
        target_width: u32,
        target_height: u32,
    ) -> Option<FrozenImage> {
        if target_width == 0 || target_height == 0 {
            return None;
        }
        let (origin_x, origin_y) = source_geometry
            .or(self.active_geometry.as_ref())
            .map(|geo| geo.physical_origin())
            .unwrap_or((0, 0));
        let (width, height, data) = crop_argb(
            &image.data,
            image.width,
            image.height,
            origin_x.max(0) as u32,
            origin_y.max(0) as u32,
            target_width,
            target_height,
        )?;
        if width != target_width || height != target_height {
            return None;
        }
        Some(FrozenImage {
            width: target_width,
            height: target_height,
            stride: (target_width * 4) as i32,
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
        if let Some(capture) = self.capture.take() {
            capture.frame.destroy();
        }
        if let Some(capture) = self.ext_capture.take() {
            capture.destroy();
        }
        self.direct_capture = None;
        self.preflight_pending = false;
        self.preflight_backend = None;
        self.portal_in_progress = false;
        if let Some(mut task) = self.portal_task.take() {
            task.cancel();
        }
        self.portal_target_output_id = None;
        self.pending_image = None;
        self.capture_done = true;
        input_state.set_frozen_active(false);
        input_state.needs_redraw = true;
    }

    fn clear_image(&mut self) -> bool {
        let had_image = self.image.take().is_some();
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
    use crate::input::state::test_support::make_test_input_state;

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
    fn direct_capture_deadline_expires_and_keeps_its_backend_identity() {
        let started_at = Instant::now();
        let mut state = FrozenState::new(None);
        state.direct_capture = Some(DirectCaptureContext::new_at(
            DirectCaptureBackend::ExtImageCopy,
            7,
            None,
            started_at,
        ));

        assert_eq!(
            state.direct_capture_timeout(started_at),
            Some(DIRECT_CAPTURE_TIMEOUT)
        );
        assert_eq!(
            state.take_timed_out_direct_capture(started_at + DIRECT_CAPTURE_TIMEOUT),
            Some(FrozenCaptureBackend::ExtImageCopy)
        );
        assert!(state.direct_capture.is_none());
        assert_eq!(state.direct_capture_timeout(started_at), None);
    }

    #[test]
    fn direct_capture_context_rejects_a_different_or_missing_output() {
        let capture = DirectCaptureContext::new_at(
            DirectCaptureBackend::WlrScreencopy,
            7,
            None,
            Instant::now(),
        );

        assert!(capture.output_matches(Some(7)));
        assert!(!capture.output_matches(Some(8)));
        assert!(!capture.output_matches(None));
    }

    #[test]
    fn active_output_capture_accepts_native_fractional_scale_dimensions() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.set_pending_output_image(
            FrozenImage {
                width: 10,
                height: 10,
                stride: 40,
                data: vec![0; 10 * 10 * 4],
            },
            None,
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
    fn active_output_capture_uses_protocol_transform_without_output_geometry() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.set_pending_output_image_with_transform(
            FrozenImage {
                width: 2,
                height: 1,
                stride: 8,
                data: vec![1, 0, 0, 255, 2, 0, 0, 255],
            },
            None,
            Some(wl_output::Transform::_90),
        );

        state
            .activate_pending_image(1, 2, &mut input_state)
            .expect("capture transform should orient the frozen image");

        let image = state.image().expect("the frozen image should be active");
        assert_eq!((image.width, image.height), (1, 2));
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
    fn cancel_clears_an_in_flight_portal_capture() {
        let mut state = FrozenState::new(None);
        let mut input_state = make_test_input_state();
        state.portal_in_progress = true;

        state.cancel(&mut input_state);

        assert!(!state.is_in_progress());
        assert!(state.take_capture_done());
    }
}
