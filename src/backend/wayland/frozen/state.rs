use log::info;
use wayland_client::protocol::wl_output;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

use crate::backend::wayland::frozen::FrozenImage;
use crate::backend::wayland::frozen_geometry::OutputGeometry;
use crate::input::InputState;

use super::PortalCaptureRx;
use super::capture::CaptureSession;

/// End-to-end controller for frozen mode capture and image storage.
#[allow(clippy::type_complexity)]
pub struct FrozenState {
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
            portal_rx: None,
            portal_in_progress: false,
            portal_target_output_id: None,
            portal_started_at: None,
            preflight_pending: false,
            capture_done: false,
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

    /// Drop frozen image if the surface size no longer matches.
    pub fn handle_resize(
        &mut self,
        phys_width: u32,
        phys_height: u32,
        input_state: &mut InputState,
    ) {
        if let Some(img) = &self.image
            && (img.width != phys_width || img.height != phys_height)
        {
            info!("Surface resized; clearing frozen image");
            self.image = None;
            input_state.set_frozen_active(false);
        }
    }

    /// Toggle unfreeze: drop the image and mark redraw.
    pub fn unfreeze(&mut self, input_state: &mut InputState) {
        self.image = None;
        input_state.set_frozen_active(false);
        input_state.dirty_tracker.mark_full();
        input_state.needs_redraw = true;
    }

    pub fn cancel(&mut self, input_state: &mut InputState) {
        if let Some(capture) = self.capture.take() {
            capture.frame.destroy();
        }
        self.preflight_pending = false;
        self.capture_done = true;
        input_state.set_frozen_active(false);
        input_state.needs_redraw = true;
    }
}
