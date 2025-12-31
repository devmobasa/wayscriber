use wayland_client::protocol::wl_output;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::backend::wayland::frozen::FrozenImage;
use crate::backend::wayland::frozen_geometry::OutputGeometry;
use crate::input::InputState;

use super::capture::CaptureSession;
use super::{MIN_ZOOM_SCALE, PortalCaptureRx};

/// Zoom state, capture logic, and pan/lock bookkeeping.
pub struct ZoomState {
    pub(super) manager: Option<ZwlrScreencopyManagerV1>,
    pub(super) active_output: Option<wl_output::WlOutput>,
    pub(super) active_output_id: Option<u32>,
    pub(super) active_geometry: Option<OutputGeometry>,
    pub(super) capture: Option<CaptureSession>,
    pub(super) image: Option<FrozenImage>,
    pub(super) portal_rx: Option<PortalCaptureRx>,
    pub(super) portal_in_progress: bool,
    pub(super) portal_target_output_id: Option<u32>,
    pub(super) portal_started_at: Option<std::time::Instant>,
    pub(super) preflight_pending: bool,
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
    pub fn new(manager: Option<ZwlrScreencopyManagerV1>) -> Self {
        Self {
            manager,
            active_output: None,
            active_output_id: None,
            active_geometry: None,
            capture: None,
            image: None,
            portal_rx: None,
            portal_in_progress: false,
            portal_target_output_id: None,
            portal_started_at: None,
            preflight_pending: false,
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
        self.active_geometry = geometry;
    }

    pub fn active_output_matches(&self, info_id: u32) -> bool {
        self.active_output_id == Some(info_id)
    }

    pub fn image(&self) -> Option<&FrozenImage> {
        self.image.as_ref()
    }

    pub fn clear_image(&mut self) -> bool {
        let had_image = self.image.is_some();
        self.image = None;
        had_image
    }

    pub fn is_in_progress(&self) -> bool {
        self.capture.is_some() || self.portal_in_progress || self.preflight_pending
    }

    pub fn preflight_pending(&self) -> bool {
        self.preflight_pending
    }

    pub fn take_preflight_pending(&mut self) -> bool {
        let pending = self.preflight_pending;
        self.preflight_pending = false;
        pending
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
        let mut changed = false;
        if let Some(capture) = self.capture.take() {
            capture.frame.destroy();
            changed = true;
        }
        if self.preflight_pending || self.portal_in_progress {
            changed = true;
        }
        self.preflight_pending = false;
        self.portal_in_progress = false;
        self.portal_rx = None;
        self.portal_target_output_id = None;
        self.portal_started_at = None;
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
        self.capture_done = true;
        self.portal_in_progress = false;
        self.portal_rx = None;
        self.portal_target_output_id = None;
        self.portal_started_at = None;
        self.pending_activation = false;

        if force_reset || self.image.is_none() {
            self.active = false;
            self.locked = false;
            self.reset_view();
            self.image = None;
        }

        input_state.set_zoom_status(self.active, self.locked, self.scale);
        input_state.dirty_tracker.mark_full();
        input_state.needs_redraw = true;
    }
}
