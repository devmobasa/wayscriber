use super::WaylandState;
use crate::{
    config::Action,
    input::{InputState, Key},
};

/// Runtime presentation-aid preferences that persist when the user changes
/// them directly, but not when a mode transition flips them transiently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InputPreferenceSnapshot {
    click_highlight_enabled: bool,
    tool_ring_enabled: bool,
    input_hud_enabled: bool,
    presenter_mode: bool,
    light_mode: bool,
}

impl InputPreferenceSnapshot {
    fn from_input_state(input_state: &InputState) -> Self {
        Self {
            click_highlight_enabled: input_state.click_highlight_enabled(),
            tool_ring_enabled: input_state.highlight_tool_ring_enabled(),
            input_hud_enabled: input_state.input_hud_enabled(),
            presenter_mode: input_state.presenter_mode,
            light_mode: input_state.light_mode,
        }
    }

    /// A mode transition owns the flip it caused, so only same-mode changes
    /// are the user's own preference.
    fn same_modes_as(self, after: Self) -> bool {
        self.presenter_mode == after.presenter_mode && self.light_mode == after.light_mode
    }

    fn click_highlight_needs_persistence_after(self, after: Self) -> bool {
        self.same_modes_as(after)
            && (self.click_highlight_enabled != after.click_highlight_enabled
                || self.tool_ring_enabled != after.tool_ring_enabled)
    }

    fn input_hud_needs_persistence_after(self, after: Self) -> bool {
        self.same_modes_as(after) && self.input_hud_enabled != after.input_hud_enabled
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn apply_input_key(&mut self, key: Key) {
        self.apply_input_update(|input_state| input_state.on_key_press(key));
    }

    pub(in crate::backend::wayland) fn dispatch_input_action(&mut self, action: Action) {
        self.apply_input_update(|input_state| input_state.handle_action(action));
    }

    fn apply_input_update(&mut self, update: impl FnOnce(&mut InputState)) {
        #[cfg(feature = "tablet-input")]
        let prev_thickness = self.input_state.current_thickness;
        let preferences_before = InputPreferenceSnapshot::from_input_state(&self.input_state);

        update(&mut self.input_state);
        self.input_state.needs_redraw = true;
        self.sync_overlay_interactivity();

        let preferences_after = InputPreferenceSnapshot::from_input_state(&self.input_state);
        if preferences_before.click_highlight_needs_persistence_after(preferences_after) {
            self.save_click_highlight_preferences();
        }
        if preferences_before.input_hud_needs_persistence_after(preferences_after) {
            self.save_input_hud_preferences();
        }
        if preferences_before.input_hud_enabled != preferences_after.input_hud_enabled {
            // A mode transition can flip the HUD without persisting it, but the
            // reader thread must follow the live state either way.
            self.sync_input_monitor();
        }

        #[cfg(feature = "tablet-input")]
        self.sync_stylus_thickness_after_input_update(prev_thickness);

        self.drain_input_action_followups();
    }

    fn drain_input_action_followups(&mut self) {
        if let Some(action) = self.input_state.take_pending_zoom_action() {
            self.handle_zoom_action(action);
        }
        if let Some(action) = self.input_state.take_pending_preset_action() {
            self.handle_preset_action(action);
        }
        if let Some(edit) = self.input_state.take_pending_quick_color_edit() {
            self.handle_quick_color_edit(edit);
        }
        if let Some(color) = self.input_state.take_pending_copy_hex_request() {
            self.handle_copy_hex_color(color);
        }
        if let Some(target) = self.input_state.take_pending_paste_hex_request() {
            self.handle_paste_hex_color(target);
        }
        while let Some(request) = self.input_state.take_pending_text_copy() {
            self.handle_copy_text(request);
        }
        while let Some(target) = self.input_state.take_pending_text_paste() {
            self.handle_paste_text(target);
        }
        self.drain_clipboard_requests();
    }

    #[cfg(feature = "tablet-input")]
    fn sync_stylus_thickness_after_input_update(&mut self, prev: f64) {
        if !self.sync_stylus_thickness_cache(prev) {
            return;
        }

        if self.stylus_tip_down {
            self.record_stylus_peak(self.input_state.current_thickness);
        } else {
            self.stylus_peak_thickness = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::InputPreferenceSnapshot;

    fn snapshot(
        click_highlight_enabled: bool,
        tool_ring_enabled: bool,
        input_hud_enabled: bool,
        presenter_mode: bool,
        light_mode: bool,
    ) -> InputPreferenceSnapshot {
        InputPreferenceSnapshot {
            click_highlight_enabled,
            tool_ring_enabled,
            input_hud_enabled,
            presenter_mode,
            light_mode,
        }
    }

    #[test]
    fn click_highlight_snapshot_persists_direct_preference_changes() {
        let before = snapshot(false, false, false, false, false);

        assert!(
            before.click_highlight_needs_persistence_after(snapshot(
                true, false, false, false, false
            ))
        );
        assert!(
            before.click_highlight_needs_persistence_after(snapshot(
                false, true, false, false, false
            ))
        );
    }

    #[test]
    fn click_highlight_snapshot_ignores_mode_transitions() {
        let before = snapshot(false, false, false, false, false);

        assert!(
            !before
                .click_highlight_needs_persistence_after(snapshot(true, false, false, true, false))
        );
        assert!(
            !before
                .click_highlight_needs_persistence_after(snapshot(true, false, false, false, true))
        );
    }

    #[test]
    fn input_hud_snapshot_persists_direct_preference_changes() {
        let before = snapshot(false, false, false, false, false);

        assert!(
            before.input_hud_needs_persistence_after(snapshot(false, false, true, false, false))
        );
        assert!(
            !before.input_hud_needs_persistence_after(snapshot(true, true, false, false, false)),
            "a click-highlight-only change must not queue an input HUD write"
        );
    }

    #[test]
    fn input_hud_snapshot_ignores_mode_transitions() {
        let before = snapshot(false, false, false, false, false);

        assert!(
            !before.input_hud_needs_persistence_after(snapshot(false, false, true, true, false))
        );
        assert!(
            !before.input_hud_needs_persistence_after(snapshot(false, false, true, false, true))
        );
    }
}
