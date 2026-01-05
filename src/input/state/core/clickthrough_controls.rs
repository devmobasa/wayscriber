use super::base::InputState;
use crate::input::tool::Tool;
use crate::util::Rect;

const CLICKTHROUGH_HOTSPOT_SIZE: i32 = 32;
const CLICKTHROUGH_HOTSPOT_MARGIN: i32 = 12;

impl InputState {
    /// Returns whether the hold-to-draw modifier is active.
    pub fn hold_to_draw_active(&self) -> bool {
        !self.hold_to_draw_keys.is_empty()
    }

    /// Returns whether click-through mode is enabled.
    pub fn clickthrough_enabled(&self) -> bool {
        self.clickthrough_enabled
    }

    /// Returns whether click-through is currently active on the main overlay.
    pub fn clickthrough_active(&self) -> bool {
        self.clickthrough_enabled
            && self.clickthrough_eligible()
            && !self.clickthrough_override
            && !self.hold_to_draw_active()
    }

    /// Returns the click-through escape hatch area (bottom-right hot corner).
    pub fn clickthrough_hotspot_rect(&self) -> Option<Rect> {
        let screen_width = self.screen_width as i32;
        let screen_height = self.screen_height as i32;
        if screen_width <= CLICKTHROUGH_HOTSPOT_MARGIN
            || screen_height <= CLICKTHROUGH_HOTSPOT_MARGIN
        {
            return None;
        }

        let size = CLICKTHROUGH_HOTSPOT_SIZE
            .min(screen_width - CLICKTHROUGH_HOTSPOT_MARGIN)
            .min(screen_height - CLICKTHROUGH_HOTSPOT_MARGIN);
        let x = screen_width - CLICKTHROUGH_HOTSPOT_MARGIN - size;
        let y = screen_height - CLICKTHROUGH_HOTSPOT_MARGIN - size;
        Rect::new(x, y, size, size)
    }

    /// Returns whether click-through is eligible but overridden by interactive mode.
    pub fn clickthrough_overridden(&self) -> bool {
        self.clickthrough_enabled && self.clickthrough_eligible() && self.clickthrough_override
    }

    pub(crate) fn enable_clickthrough(&mut self) {
        if !self.clickthrough_enabled {
            self.clickthrough_enabled = true;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    pub(crate) fn disable_clickthrough(&mut self) {
        if self.clickthrough_enabled || self.clickthrough_override {
            self.clickthrough_enabled = false;
            self.clickthrough_override = false;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
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
