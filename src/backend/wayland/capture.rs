//! Capture controller for managing overlay visibility and capture state.
//!
//! Keeps the overlay hide/show logic alongside the CaptureManager so the
//! main Wayland loop only coordinates events instead of tracking flags.

use crate::capture::CaptureManager;
use log::{info, warn};
use smithay_client_toolkit::shell::WaylandSurface;

use super::surface::SurfaceState;

/// Tracks capture manager state along with overlay visibility flags.
pub struct CaptureState {
    manager: CaptureManager,
    in_progress: bool,
    overlay_hidden: bool,
}

impl CaptureState {
    /// Creates a new capture state wrapper.
    pub fn new(manager: CaptureManager) -> Self {
        Self {
            manager,
            in_progress: false,
            overlay_hidden: false,
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

    /// Hides the overlay before capture.
    ///
    /// Returns `true` if the overlay was hidden, `false` if it was already hidden.
    pub fn hide_overlay(&mut self, surface: &mut SurfaceState) -> bool {
        if self.overlay_hidden {
            warn!("Overlay already hidden for capture");
            return false;
        }

        info!("Hiding overlay for screenshot capture");
        if let Some(layer_surface) = surface.layer_surface_mut() {
            layer_surface.set_size(0, 0);
            let wl_surface = layer_surface.wl_surface();
            wl_surface.commit();
        }

        self.overlay_hidden = true;
        true
    }

    /// Restores the overlay after capture.
    ///
    /// Returns `true` if the overlay was restored, `false` if it wasn't hidden.
    pub fn show_overlay(&mut self, surface: &mut SurfaceState) -> bool {
        if !self.overlay_hidden {
            warn!("Overlay was not hidden, nothing to restore");
            return false;
        }

        info!("Restoring overlay after screenshot capture");

        let width = surface.width();
        let height = surface.height();
        if let Some(layer_surface) = surface.layer_surface_mut() {
            layer_surface.set_size(width, height);
            let wl_surface = layer_surface.wl_surface();
            wl_surface.commit();
        }

        self.overlay_hidden = false;
        true
    }
}
