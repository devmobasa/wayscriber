//! Capture controller for managing screenshot capture state.
//!
//! Keeps the capture manager and in-progress flag together so the main
//! Wayland loop can coordinate capture requests and results.

use crate::capture::{CaptureManager, CaptureRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapturePreflight {
    None,
    AwaitingRender,
    AwaitingFrame,
}
/// Tracks capture manager state and in-progress flag.
pub struct CaptureState {
    manager: CaptureManager,
    in_progress: bool,
    exit_on_success: bool,
    preflight: CapturePreflight,
    pending_request: Option<CaptureRequest>,
}

impl CaptureState {
    /// Creates a new capture state wrapper.
    pub fn new(manager: CaptureManager) -> Self {
        Self {
            manager,
            in_progress: false,
            exit_on_success: false,
            preflight: CapturePreflight::None,
            pending_request: None,
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
    pub fn queue_preflight(&mut self, request: CaptureRequest) {
        self.pending_request = Some(request);
        self.preflight = CapturePreflight::AwaitingRender;
    }

    /// Returns true if capture is waiting on suppression render/callback.
    #[cfg(test)]
    pub fn preflight_pending(&self) -> bool {
        !matches!(self.preflight, CapturePreflight::None)
    }

    /// Returns true if the next render should request a frame callback.
    pub fn preflight_needs_frame_callback(&self) -> bool {
        matches!(self.preflight, CapturePreflight::AwaitingRender)
    }

    /// Mark that the suppression frame has been rendered and committed.
    pub fn mark_preflight_rendered(&mut self) {
        if matches!(self.preflight, CapturePreflight::AwaitingRender) {
            self.preflight = CapturePreflight::AwaitingFrame;
        }
    }

    /// Take the queued request once the suppression frame callback fires.
    pub fn take_preflight_request(&mut self) -> Option<CaptureRequest> {
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

    /// Marks capture as started.
    pub fn mark_in_progress(&mut self) {
        self.in_progress = true;
    }

    /// Marks capture as finished.
    pub fn clear_in_progress(&mut self) {
        self.in_progress = false;
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

        state.queue_preflight(request);
        assert!(state.preflight_pending());
        assert!(state.take_preflight_request().is_none());

        state.mark_preflight_rendered();
        assert!(state.take_preflight_request().is_some());
        assert!(!state.preflight_pending());
    }
}
