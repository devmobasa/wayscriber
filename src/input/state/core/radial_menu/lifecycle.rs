use super::super::base::{DrawingState, InputState};
use super::types::RadialMenuState;
use crate::util::Rect;

impl InputState {
    /// Opens the radial menu at the given screen position.
    pub fn open_radial_menu(&mut self, anchor: (i32, i32)) {
        // Don't open during active drawing or text input
        if !matches!(self.state, DrawingState::Idle) {
            return;
        }

        // Close any other overlays first
        self.close_context_menu();
        self.close_properties_panel();

        self.radial_menu_state = RadialMenuState::Open {
            anchor,
            hover_index: None,
            slot_count: self.preset_slot_count,
        };
        self.radial_menu_layout = None;

        // Mark the area dirty so it actually renders
        let outer = super::types::RADIAL_OUTER_RADIUS;
        let size = (outer * 2.0 + 4.0) as i32;
        if let Some(rect) = Rect::new(anchor.0 - size / 2, anchor.1 - size / 2, size, size) {
            self.dirty_tracker.mark_rect(rect);
        }
        self.needs_redraw = true;
    }

    /// Closes the radial menu.
    pub fn close_radial_menu(&mut self) {
        if !self.is_radial_menu_open() {
            return;
        }

        // Mark dirty region before closing
        if let Some(layout) = self.radial_menu_layout.take() {
            let size = (layout.outer_radius * 2.0 + 4.0) as i32;
            if let Some(rect) = Rect::new(
                (layout.center_x - layout.outer_radius - 2.0) as i32,
                (layout.center_y - layout.outer_radius - 2.0) as i32,
                size,
                size,
            ) {
                self.dirty_tracker.mark_rect(rect);
            }
        }

        self.radial_menu_state = RadialMenuState::Hidden;
        self.needs_redraw = true;
    }

    /// Returns true if the radial menu is currently open.
    pub fn is_radial_menu_open(&self) -> bool {
        matches!(self.radial_menu_state, RadialMenuState::Open { .. })
    }

    /// Returns the current radial menu layout if available.
    pub fn radial_menu_layout(&self) -> Option<&super::types::RadialMenuLayout> {
        self.radial_menu_layout.as_ref()
    }
}
