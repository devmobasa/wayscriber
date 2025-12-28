use super::base::{DrawingState, InputState};
use crate::input::tool::Tool;

impl InputState {
    /// Toggle click-through with Select tool semantics.
    pub fn toggle_clickthrough_mode(&mut self) {
        if self.tool_override() != Some(Tool::Select) {
            if matches!(self.state, DrawingState::TextInput { .. }) {
                self.cancel_text_input();
            }
            self.set_tool_override(Some(Tool::Select));
            self.clear_hold_to_draw();
            self.clear_clickthrough_override();
        } else {
            self.toggle_clickthrough_override();
        }
    }

    /// Returns whether the hold-to-draw modifier is active.
    pub fn hold_to_draw_active(&self) -> bool {
        !self.hold_to_draw_keys.is_empty()
    }

    /// Returns whether click-through is currently active on the main overlay.
    pub fn clickthrough_active(&self) -> bool {
        self.clickthrough_eligible() && !self.clickthrough_override && !self.hold_to_draw_active()
    }

    /// Returns whether click-through is eligible but overridden by interactive mode.
    pub fn clickthrough_overridden(&self) -> bool {
        self.clickthrough_eligible() && self.clickthrough_override
    }

    /// Toggles the click-through override; returns true when interactive mode is forced.
    pub fn toggle_clickthrough_override(&mut self) -> bool {
        self.clickthrough_override = !self.clickthrough_override;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        self.clickthrough_override
    }

    pub(crate) fn clear_clickthrough_override(&mut self) {
        if self.clickthrough_override {
            self.clickthrough_override = false;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    pub(crate) fn add_hold_to_draw_key(&mut self, key: &str) -> bool {
        let normalized = normalize_key(key);
        if self.hold_to_draw_keys.insert(normalized) {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }

    pub(crate) fn remove_hold_to_draw_key(&mut self, key: &str) -> bool {
        let normalized = normalize_key(key);
        if self.hold_to_draw_keys.remove(&normalized) {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }

    pub(crate) fn clear_hold_to_draw(&mut self) {
        if !self.hold_to_draw_keys.is_empty() {
            self.hold_to_draw_keys.clear();
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    fn clickthrough_eligible(&self) -> bool {
        matches!(self.tool_override, Some(Tool::Select))
    }
}

fn normalize_key(key: &str) -> String {
    key.trim().to_ascii_lowercase()
}
