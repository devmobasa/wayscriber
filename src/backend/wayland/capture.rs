//! Capture controller for managing screenshot capture state.
//!
//! Keeps the capture manager and in-progress flag together so the main
//! Wayland loop can coordinate capture requests and results.

use crate::{
    capture::{
        CaptureManager, CaptureRequest, CaptureRequestId, DesktopBackdropCaptureRequest,
        ImageOperationKind, file::FileSaveConfig,
    },
    config::Action,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapturePreflight {
    None,
    AwaitingRender,
    AwaitingFrame,
}

#[derive(Clone)]
pub(in crate::backend::wayland) enum CapturePreflightRequest {
    Screenshot(CaptureRequest),
    DesktopBackdrop(DesktopBackdropCaptureRequest),
}

impl std::fmt::Debug for CapturePreflightRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Screenshot(request) => f.debug_tuple("Screenshot").field(request).finish(),
            Self::DesktopBackdrop(request) => {
                f.debug_tuple("DesktopBackdrop").field(request).finish()
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(in crate::backend::wayland) struct PendingPdfExport {
    pub action: Action,
    pub operation: ImageOperationKind,
    pub save_config: FileSaveConfig,
}

/// A step capture waiting for its desktop-backdrop frame: the recorded
/// pointer position becomes the step marker on the appended page.
#[derive(Clone, Copy, Debug)]
pub(in crate::backend::wayland) struct PendingStepCapture {
    pub marker: Option<(i32, i32)>,
    pub logical_width: i32,
    pub logical_height: i32,
    /// The step came from an intercepted canvas click; re-send it beneath
    /// the overlay once the frame is captured.
    pub forward_click: bool,
}

/// Tracks capture manager state and in-progress flag.
pub struct CaptureState {
    manager: CaptureManager,
    in_progress: bool,
    accepted_id: Option<CaptureRequestId>,
    exit_on_success: bool,
    preflight: CapturePreflight,
    pending_request: Option<CapturePreflightRequest>,
    pending_pdf_export: Option<PendingPdfExport>,
    pending_step_capture: Option<PendingStepCapture>,
}

impl CaptureState {
    /// Creates a new capture state wrapper.
    pub fn new(manager: CaptureManager) -> Self {
        Self {
            manager,
            in_progress: false,
            accepted_id: None,
            exit_on_success: false,
            preflight: CapturePreflight::None,
            pending_request: None,
            pending_pdf_export: None,
            pending_step_capture: None,
        }
    }

    /// Returns a mutable reference to the underlying capture manager.
    pub fn manager_mut(&mut self) -> &mut CaptureManager {
        &mut self.manager
    }

    /// Returns `true` if a capture request is currently active.
    pub fn is_in_progress(&self) -> bool {
        self.in_progress
    }

    /// Queue a capture request that should wait for a suppression render + frame callback.
    pub fn queue_preflight(&mut self, request: CapturePreflightRequest) {
        self.pending_request = Some(request);
        self.preflight = CapturePreflight::AwaitingRender;
    }

    /// Returns true if capture is waiting on suppression render/callback.
    #[cfg(test)]
    pub fn preflight_pending(&self) -> bool {
        !matches!(self.preflight, CapturePreflight::None)
    }

    /// Mark that the suppression frame has been rendered and committed.
    pub fn mark_preflight_rendered(&mut self) {
        if matches!(self.preflight, CapturePreflight::AwaitingRender) {
            self.preflight = CapturePreflight::AwaitingFrame;
        }
    }

    /// Take the queued request once the suppression frame callback fires.
    pub fn take_preflight_request(&mut self) -> Option<CapturePreflightRequest> {
        if matches!(self.preflight, CapturePreflight::AwaitingFrame) {
            self.preflight = CapturePreflight::None;
            return self.pending_request.take();
        }
        None
    }

    /// Clear any pending preflight capture request.
    pub fn clear_preflight(&mut self) {
        self.preflight = CapturePreflight::None;
        self.pending_request = None;
    }

    pub fn set_pending_pdf_export(&mut self, request: PendingPdfExport) {
        self.pending_pdf_export = Some(request);
    }

    pub fn take_pending_pdf_export(&mut self) -> Option<PendingPdfExport> {
        self.pending_pdf_export.take()
    }

    pub fn clear_pending_pdf_export(&mut self) {
        self.pending_pdf_export = None;
    }

    pub fn set_pending_step_capture(&mut self, request: PendingStepCapture) {
        self.pending_step_capture = Some(request);
    }

    pub fn take_pending_step_capture(&mut self) -> Option<PendingStepCapture> {
        self.pending_step_capture.take()
    }

    pub fn has_pending_step_capture(&self) -> bool {
        self.pending_step_capture.is_some()
    }

    pub fn clear_pending_step_capture(&mut self) {
        self.pending_step_capture = None;
    }

    /// Marks capture as started.
    pub fn mark_in_progress(&mut self) {
        self.in_progress = true;
    }

    /// Marks capture as finished.
    pub fn clear_in_progress(&mut self) {
        self.in_progress = false;
        self.accepted_id = None;
    }

    /// Records the manager identity accepted for the current lifecycle.
    pub fn record_accepted(&mut self, id: CaptureRequestId) -> bool {
        if !self.in_progress || self.accepted_id.is_some() {
            return false;
        }
        self.accepted_id = Some(id);
        true
    }

    /// Consumes the accepted identity only when the completion matches it.
    pub fn consume_accepted(&mut self, id: CaptureRequestId) -> bool {
        if self.accepted_id != Some(id) {
            return false;
        }
        self.accepted_id = None;
        true
    }

    pub fn accepted_id(&self) -> Option<CaptureRequestId> {
        self.accepted_id
    }

    /// Marks whether the current capture should exit the overlay on success.
    pub fn set_exit_on_success(&mut self, value: bool) {
        self.exit_on_success = value;
    }

    /// Clears any pending exit request for the current capture.
    pub fn clear_exit_on_success(&mut self) {
        self.exit_on_success = false;
    }

    /// Returns and clears the exit-on-success flag for the last capture.
    pub fn take_exit_on_success(&mut self) -> bool {
        let value = self.exit_on_success;
        self.exit_on_success = false;
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::types::{CaptureDestination, CaptureType};

    #[test]
    fn preflight_waits_for_render_before_request() {
        let manager = CaptureManager::with_closed_channel_for_test();
        let mut state = CaptureState::new(manager);
        let request = CaptureRequest {
            capture_type: CaptureType::FullScreen,
            destination: CaptureDestination::ClipboardOnly,
            save_config: None,
        };

        state.queue_preflight(CapturePreflightRequest::Screenshot(request));
        assert!(state.preflight_pending());
        assert!(state.take_preflight_request().is_none());

        state.mark_preflight_rendered();
        assert!(matches!(
            state.take_preflight_request(),
            Some(CapturePreflightRequest::Screenshot(_))
        ));
        assert!(!state.preflight_pending());
    }

    #[test]
    fn accepted_identity_is_owned_by_exactly_one_active_lifecycle() {
        let manager = CaptureManager::with_closed_channel_for_test();
        let mut state = CaptureState::new(manager);
        let first = CaptureRequestId::for_test(7);
        let other = CaptureRequestId::for_test(8);

        assert!(!state.record_accepted(first));
        state.mark_in_progress();
        assert!(state.record_accepted(first));
        assert!(!state.record_accepted(other));
        assert!(!state.consume_accepted(other));
        assert_eq!(state.accepted_id(), Some(first));
        assert!(state.consume_accepted(first));
        assert_eq!(state.accepted_id(), None);

        assert!(state.record_accepted(other));
        state.clear_in_progress();
        assert_eq!(state.accepted_id(), None);
        assert!(!state.is_in_progress());
    }
}
