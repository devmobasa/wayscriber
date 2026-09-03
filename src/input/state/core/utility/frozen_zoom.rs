use super::super::base::{InputEffect, InputState};

impl InputState {
    /// Marks a frozen-mode toggle request for the backend.
    pub(crate) fn request_frozen_toggle(&mut self) {
        self.emit_input_effect(InputEffect::FrozenPass {
            user_requested: true,
        });
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
    pub fn set_zoom_status(
        &mut self,
        active: bool,
        locked: bool,
        scale: f64,
        view_offset: (f64, f64),
    ) {
        let changed = self.zoom_active != active
            || self.zoom_locked != locked
            || (self.zoom_scale - scale).abs() > f64::EPSILON
            || (self.zoom_view_offset.0 - view_offset.0).abs() > f64::EPSILON
            || (self.zoom_view_offset.1 - view_offset.1).abs() > f64::EPSILON;
        if changed {
            self.zoom_active = active;
            self.zoom_locked = locked;
            self.zoom_scale = scale;
            self.zoom_view_offset = view_offset;
            self.sync_canvas_pointer_to_current_transform();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KeybindingsConfig;

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let _action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        crate::input::state::test_support::make_test_input_state()
    }

    #[test]
    fn frozen_toggle_request_is_consumed_once() {
        let mut state = make_state();
        state.request_frozen_toggle();

        assert!(state.take_pending_frozen_toggle());
        assert!(!state.take_pending_frozen_toggle());
    }

    #[test]
    fn set_frozen_active_marks_redraw_only_on_change() {
        let mut state = make_state();
        state.needs_redraw = false;

        state.set_frozen_active(true);
        assert!(state.frozen_active());
        assert!(state.needs_redraw);

        state.needs_redraw = false;
        state.set_frozen_active(true);
        assert!(!state.needs_redraw);
    }

    #[test]
    fn set_zoom_status_updates_accessors_and_marks_redraw_only_on_change() {
        let mut state = make_state();
        state.needs_redraw = false;

        state.set_zoom_status(true, true, 2.0, (40.0, 60.0));
        assert!(state.zoom_active());
        assert!(state.zoom_locked());
        assert_eq!(state.zoom_scale(), 2.0);
        assert!(state.needs_redraw);

        state.needs_redraw = false;
        state.set_zoom_status(true, true, 2.0, (40.0, 60.0));
        assert!(!state.needs_redraw);
    }
}
