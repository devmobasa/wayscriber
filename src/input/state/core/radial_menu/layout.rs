use super::super::base::InputState;
use super::types::{RADIAL_INNER_RADIUS, RADIAL_OUTER_RADIUS, RadialMenuLayout, RadialMenuState};
use std::f64::consts::PI;

impl InputState {
    /// Updates the radial menu layout based on current screen dimensions.
    /// Called during rendering to ensure layout is up-to-date.
    pub fn update_radial_menu_layout(&mut self, screen_width: u32, screen_height: u32) {
        let (anchor, slot_count) = match &self.radial_menu_state {
            RadialMenuState::Open {
                anchor, slot_count, ..
            } => (*anchor, *slot_count),
            RadialMenuState::Hidden => return,
        };

        // Clamp anchor to keep menu fully on screen
        let outer = RADIAL_OUTER_RADIUS;
        let margin = 4.0;
        let center_x = (anchor.0 as f64)
            .max(outer + margin)
            .min(screen_width as f64 - outer - margin);
        let center_y = (anchor.1 as f64)
            .max(outer + margin)
            .min(screen_height as f64 - outer - margin);

        self.radial_menu_layout = Some(RadialMenuLayout {
            center_x,
            center_y,
            inner_radius: RADIAL_INNER_RADIUS,
            outer_radius: RADIAL_OUTER_RADIUS,
            segment_count: slot_count,
            start_angle: -PI / 2.0, // First segment points up
        });
    }

    /// Returns which preset slot (0-indexed) is at the given screen position,
    /// or None if the position is in the center cancel zone or outside the menu.
    pub fn radial_menu_segment_at(&self, x: i32, y: i32) -> Option<usize> {
        let layout = self.radial_menu_layout.as_ref()?;

        let dx = x as f64 - layout.center_x;
        let dy = y as f64 - layout.center_y;
        let distance = (dx * dx + dy * dy).sqrt();

        // In center zone (cancel) or outside menu
        if distance < layout.inner_radius || distance > layout.outer_radius {
            return None;
        }

        // Calculate angle and determine segment
        // atan2 returns angle from positive x-axis, we want from top (negative y)
        let mut angle = dy.atan2(dx) - layout.start_angle;
        if angle < 0.0 {
            angle += 2.0 * PI;
        }

        let segment_angle = 2.0 * PI / layout.segment_count as f64;
        let segment = (angle / segment_angle).floor() as usize;

        if segment < layout.segment_count {
            Some(segment)
        } else {
            None
        }
    }

    /// Updates the hovered segment based on pointer position.
    pub fn update_radial_menu_hover(&mut self, x: i32, y: i32) {
        let new_index = self.radial_menu_segment_at(x, y);

        if let RadialMenuState::Open {
            ref mut hover_index,
            ..
        } = self.radial_menu_state
            && *hover_index != new_index
        {
            *hover_index = new_index;
            self.needs_redraw = true;
        }
    }
}
