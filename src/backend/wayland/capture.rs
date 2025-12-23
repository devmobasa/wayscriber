//! Capture controller for managing screenshot capture state.
//!
//! Keeps the capture manager and in-progress flag together so the main
//! Wayland loop can coordinate capture requests and results.

use crate::capture::CaptureManager;
/// Tracks capture manager state and in-progress flag.
pub struct CaptureState {
    manager: CaptureManager,
    in_progress: bool,
    exit_on_success: bool,
}

impl CaptureState {
    /// Creates a new capture state wrapper.
    pub fn new(manager: CaptureManager) -> Self {
        Self {
            manager,
            in_progress: false,
            exit_on_success: false,
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
