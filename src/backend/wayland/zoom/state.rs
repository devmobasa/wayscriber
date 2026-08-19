use wayland_client::protocol::wl_output;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::backend::wayland::RuntimeWakeHandle;
use crate::backend::wayland::frozen::FrozenImage;
use crate::backend::wayland::frozen_geometry::OutputGeometry;
use crate::backend::wayland::portal_capture::layout_token_matches;
use crate::backend::wayland::portal_task::PortalTask;
use crate::input::InputState;
use crate::input::state::{Toast, ToastPriority};

use super::capture::CaptureSession;
use super::{MIN_ZOOM_SCALE, PortalCaptureResult};

/// Zoom state, capture logic, and pan/lock bookkeeping.
pub struct ZoomState {
    pub(super) manager: Option<ZwlrScreencopyManagerV1>,
    pub(super) active_output: Option<wl_output::WlOutput>,
    pub(super) active_output_id: Option<u32>,
    pub(super) active_geometry: Option<OutputGeometry>,
    /// Bumped when freeze/zoom crop geometry actually changes so in-flight
    /// portal captures can be rejected after a layout change.
    pub(super) output_layout_generation: u64,
    pub(super) capture: Option<CaptureSession>,
    pub(super) image: Option<FrozenImage>,
    pub(super) image_target_dimensions: Option<(u32, u32)>,
    image_generation: u64,
    pub(super) portal_task: Option<PortalTask<PortalCaptureResult>>,
    pub(super) portal_in_progress: bool,
    pub(super) portal_target_output_id: Option<u32>,
    pub(super) runtime_wake: Option<RuntimeWakeHandle>,
    pub(super) preflight_pending: bool,
    pub(super) preflight_use_fallback: bool,
    preflight_output_id: Option<u32>,
    preflight_layout_generation: Option<u64>,
    pub(super) capture_done: bool,
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
            output_layout_generation: 0,
            capture: None,
            image: None,
            image_target_dimensions: None,
            image_generation: 0,
            portal_task: None,
            portal_in_progress: false,
            portal_target_output_id: None,
            runtime_wake,
            preflight_pending: false,
            preflight_use_fallback: false,
            preflight_output_id: None,
            preflight_layout_generation: None,
            capture_done: false,
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
        if self.active_geometry != geometry {
            self.output_layout_generation = self.output_layout_generation.wrapping_add(1);
        }
        self.active_geometry = geometry;
    }

    pub fn image(&self) -> Option<&FrozenImage> {
        self.image.as_ref()
    }

    pub fn image_generation(&self) -> u64 {
        self.image_generation
    }

    pub fn set_image(&mut self, image: FrozenImage) {
        self.image_target_dimensions = self
            .active_geometry
            .as_ref()
            .map(OutputGeometry::buffer_size)
            .or(Some((image.width, image.height)));
        self.image = Some(image);
        self.bump_image_generation();
    }

    pub fn clear_image(&mut self) -> bool {
        let had_image = self.image.take().is_some();
        self.image_target_dimensions = None;
        if had_image {
            self.bump_image_generation();
        }
        had_image
    }

    pub fn is_in_progress(&self) -> bool {
        self.capture.is_some() || self.portal_in_progress || self.preflight_pending
    }

    #[cfg(test)]
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
            Err("Zoom failed after the display layout changed".to_string())
        }
    }

    #[cfg(test)]
    pub fn preflight_layout_is_current(&self) -> bool {
        self.ensure_preflight_layout_current().is_ok()
    }

    pub(super) fn push_stale_layout_toast(input_state: &mut InputState) {
        input_state.push_toast(
            ToastPriority::Critical,
            "zoom",
            Toast::error("Zoom failed after the display layout changed"),
        );
    }

    pub(super) fn finish_stale_direct_capture(&mut self, input_state: &mut InputState) {
        Self::push_stale_layout_toast(input_state);
        self.cancel(input_state, false);
    }

    fn clear_preflight_layout_snapshot(&mut self) {
        self.preflight_output_id = None;
        self.preflight_layout_generation = None;
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
        let mut changed = self.pending_activation;
        if let Some(capture) = self.capture.take() {
            capture.frame.destroy();
            changed = true;
        }
        if self.preflight_pending || self.portal_in_progress {
            changed = true;
        }
        self.preflight_pending = false;
        self.preflight_use_fallback = false;
        self.clear_preflight_layout_snapshot();
        self.portal_in_progress = false;
        if let Some(mut task) = self.portal_task.take() {
            task.cancel();
        }
        self.portal_target_output_id = None;
        self.pending_activation = false;
        if changed {
            self.capture_done = true;
        }
        changed
    }

    pub fn deactivate(&mut self, input_state: &mut InputState) {
        self.cancel(input_state, true);
    }

    pub fn reset_view(&mut self) {
        self.scale = MIN_ZOOM_SCALE;
        self.view_offset = (0.0, 0.0);
        self.panning = false;
        self.last_pan_pos = (0.0, 0.0);
    }

    pub fn cancel(&mut self, input_state: &mut InputState, force_reset: bool) {
        if let Some(capture) = self.capture.take() {
            capture.frame.destroy();
        }
        self.preflight_pending = false;
        self.preflight_use_fallback = false;
        self.clear_preflight_layout_snapshot();
        self.capture_done = true;
        self.portal_in_progress = false;
        if let Some(mut task) = self.portal_task.take() {
            task.cancel();
        }
        self.portal_target_output_id = None;
        self.pending_activation = false;

        if force_reset || self.image.is_none() {
            self.active = false;
            self.locked = false;
            self.reset_view();
            self.clear_image();
        }

        input_state.set_zoom_status(self.active, self.locked, self.scale, self.view_offset);
        input_state.dirty_tracker.mark_full();
        input_state.needs_redraw = true;
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
    fn stale_direct_capture_toasts_and_preserves_the_current_image() {
        let mut state = ZoomState::new(None);
        let mut input_state = make_test_input_state();
        state.set_image(FrozenImage {
            width: 1,
            height: 1,
            stride: 4,
            data: vec![4; 4],
        });
        let generation = state.image_generation();

        state.finish_stale_direct_capture(&mut input_state);

        let toast = input_state
            .ui_toast
            .as_ref()
            .expect("visible stale rejection");
        assert!(toast.message.contains("display layout changed"));
        assert_eq!(state.image_generation(), generation);
        assert_eq!(state.image().unwrap().data, vec![4; 4]);
        assert!(state.take_capture_done());
    }
}
