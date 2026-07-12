use log::info;
use wayland_client::protocol::wl_output;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::backend::wayland::frozen::FrozenImage;
use crate::backend::wayland::frozen_geometry::OutputGeometry;
use crate::backend::wayland::portal_capture::crop_argb;
use crate::input::InputState;

use super::PortalCaptureRx;
use super::capture::CaptureSession;

struct PendingFrozenImage {
    image: FrozenImage,
    source_geometry: Option<OutputGeometry>,
    needs_output_transform: bool,
    source: FrozenCaptureSource,
}

#[derive(Clone, Copy)]
enum FrozenCaptureSource {
    ActiveOutput,
    Desktop,
}

/// End-to-end controller for frozen mode capture and image storage.
#[allow(clippy::type_complexity)]
pub struct FrozenState {
    pub(super) manager: Option<ZwlrScreencopyManagerV1>,
    pub(super) active_output: Option<wl_output::WlOutput>,
    pub(super) active_output_id: Option<u32>,
    pub(super) active_geometry: Option<OutputGeometry>,
    pub(super) capture: Option<CaptureSession>,
    pub(super) image: Option<FrozenImage>,
    image_target_dimensions: Option<(u32, u32)>,
    image_generation: u64,
    pub(super) portal_rx: Option<PortalCaptureRx>,
    pub(super) portal_in_progress: bool,
    pub(super) portal_target_output_id: Option<u32>,
    pub(super) portal_started_at: Option<std::time::Instant>,
    pub(super) preflight_pending: bool,
    pub(super) preflight_use_fallback: bool,
    pub(super) capture_done: bool,
    pending_image: Option<PendingFrozenImage>,
}

impl FrozenState {
    pub fn new(manager: Option<ZwlrScreencopyManagerV1>) -> Self {
        Self {
            manager,
            active_output: None,
            active_output_id: None,
            active_geometry: None,
            capture: None,
            image: None,
            image_target_dimensions: None,
            image_generation: 0,
            portal_rx: None,
            portal_in_progress: false,
            portal_target_output_id: None,
            portal_started_at: None,
            preflight_pending: false,
            preflight_use_fallback: false,
            capture_done: false,
            pending_image: None,
        }
    }

    #[allow(dead_code)]
    pub fn manager_available(&self) -> bool {
        self.manager.is_some()
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
            needs_output_transform: false,
            source: FrozenCaptureSource::Desktop,
        });
    }

    pub fn has_pending_image(&self) -> bool {
        self.pending_image.is_some()
    }

    pub fn is_in_progress(&self) -> bool {
        self.capture.is_some()
            || self.portal_in_progress
            || self.preflight_pending
            || self.pending_image.is_some()
    }

    pub fn preflight_pending(&self) -> bool {
        self.preflight_pending
    }

    pub fn take_preflight_pending(&mut self) -> Option<bool> {
        if !self.preflight_pending {
            return None;
        }
        let use_fallback = self.preflight_use_fallback;
        self.preflight_pending = false;
        self.preflight_use_fallback = false;
        Some(use_fallback)
    }

    pub fn take_capture_done(&mut self) -> bool {
        let done = self.capture_done;
        self.capture_done = false;
        done
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
            let output_transform = pending
                .source_geometry
                .as_ref()
                .or(self.active_geometry.as_ref())
                .map(|geo| geo.transform)
                .unwrap_or(wl_output::Transform::Normal);
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
        self.preflight_pending = false;
        self.preflight_use_fallback = false;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;

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
}
