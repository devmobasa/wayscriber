use super::WaylandState;
use crate::{
    config::Action,
    input::{InputState, Key},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ClickHighlightPreferencesSnapshot {
    enabled: bool,
    tool_ring_enabled: bool,
    presenter_mode: bool,
    light_mode: bool,
}

impl ClickHighlightPreferencesSnapshot {
    fn from_input_state(input_state: &InputState) -> Self {
        Self {
            enabled: input_state.click_highlight_enabled(),
            tool_ring_enabled: input_state.highlight_tool_ring_enabled(),
            presenter_mode: input_state.presenter_mode,
            light_mode: input_state.light_mode,
        }
    }

    fn needs_persistence_after(self, after: Self) -> bool {
        self.presenter_mode == after.presenter_mode
            && self.light_mode == after.light_mode
            && (self.enabled != after.enabled || self.tool_ring_enabled != after.tool_ring_enabled)
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
        let highlight_before =
            ClickHighlightPreferencesSnapshot::from_input_state(&self.input_state);

        update(&mut self.input_state);
        self.input_state.needs_redraw = true;
        self.sync_overlay_interactivity();

        let highlight_after =
            ClickHighlightPreferencesSnapshot::from_input_state(&self.input_state);
        if highlight_before.needs_persistence_after(highlight_after) {
            self.save_click_highlight_preferences();
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
        if let Some(color) = self.input_state.take_pending_copy_hex_request() {
            self.handle_copy_hex_color(color);
        }
        if let Some(target) = self.input_state.take_pending_paste_hex_request() {
            self.handle_paste_hex_color(target);
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
    use super::ClickHighlightPreferencesSnapshot;

    fn snapshot(
        enabled: bool,
        tool_ring_enabled: bool,
        presenter_mode: bool,
        light_mode: bool,
    ) -> ClickHighlightPreferencesSnapshot {
        ClickHighlightPreferencesSnapshot {
            enabled,
            tool_ring_enabled,
            presenter_mode,
            light_mode,
        }
    }

    #[test]
    fn click_highlight_snapshot_persists_direct_preference_changes() {
        let before = snapshot(false, false, false, false);

        assert!(before.needs_persistence_after(snapshot(true, false, false, false)));
        assert!(before.needs_persistence_after(snapshot(false, true, false, false)));
    }

    #[test]
    fn click_highlight_snapshot_ignores_mode_transitions() {
        let before = snapshot(false, false, false, false);

        assert!(!before.needs_persistence_after(snapshot(true, false, true, false)));
        assert!(!before.needs_persistence_after(snapshot(true, false, false, true)));
    }
}
