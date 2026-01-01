use super::super::base::InputState;

impl InputState {
    /// Marks a frozen-mode toggle request for the backend.
    pub(crate) fn request_frozen_toggle(&mut self) {
        self.pending_frozen_toggle = true;
    }

    /// Returns and clears any pending frozen-mode toggle request.
    pub fn take_pending_frozen_toggle(&mut self) -> bool {
        let pending = self.pending_frozen_toggle;
        self.pending_frozen_toggle = false;
        pending
    }

    /// Updates the cached frozen-mode status and triggers a redraw when it changes.
    pub fn set_frozen_active(&mut self, active: bool) {
        if self.frozen_active != active {
            self.frozen_active = active;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Returns whether frozen mode is active.
    pub fn frozen_active(&self) -> bool {
        self.frozen_active
    }

    /// Updates cached zoom status and triggers a redraw when it changes.
    pub fn set_zoom_status(&mut self, active: bool, locked: bool, scale: f64) {
        let changed = self.zoom_active != active
            || self.zoom_locked != locked
            || (self.zoom_scale - scale).abs() > f64::EPSILON;
        if changed {
            self.zoom_active = active;
            self.zoom_locked = locked;
            self.zoom_scale = scale;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Returns whether zoom mode is active.
    pub fn zoom_active(&self) -> bool {
        self.zoom_active
    }

    /// Returns whether zoom view is locked.
    pub fn zoom_locked(&self) -> bool {
        self.zoom_locked
    }

    /// Returns the current zoom scale.
    pub fn zoom_scale(&self) -> f64 {
        self.zoom_scale
    }
}
