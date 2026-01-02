use crate::input::InputState;
use crate::util::Rect;

impl InputState {
    pub(super) fn movable_selection_bounds(&self) -> Option<Rect> {
        let ids = self.selected_shape_ids();
        if ids.is_empty() {
            return None;
        }

        let frame = self.boards.active_frame();
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        let mut found = false;

        for id in ids {
            if let Some(shape) = frame.shape(*id) {
                if shape.locked {
                    continue;
                }
                if let Some(bounds) = shape.shape.bounding_box() {
                    min_x = min_x.min(bounds.x);
                    min_y = min_y.min(bounds.y);
                    max_x = max_x.max(bounds.x + bounds.width);
                    max_y = max_y.max(bounds.y + bounds.height);
                    found = true;
                }
            }
        }

        if found {
            Rect::from_min_max(min_x, min_y, max_x, max_y)
        } else {
            None
        }
    }

    fn clamp_axis_delta(position: i32, size: i32, screen: i32, delta: i32) -> i32 {
        if screen <= 0 || size <= 0 {
            return delta;
        }

        let end = position.saturating_add(size);
        let (min_delta, max_delta) = if size <= screen {
            (0i32.saturating_sub(position), screen.saturating_sub(end))
        } else {
            (screen.saturating_sub(end), 0i32.saturating_sub(position))
        };

        delta.clamp(min_delta, max_delta)
    }

    pub(super) fn clamp_selection_translation(&self, dx: i32, dy: i32) -> Option<(i32, i32)> {
        let bounds = self.movable_selection_bounds()?;
        let screen_width = self.screen_width.min(i32::MAX as u32) as i32;
        let screen_height = self.screen_height.min(i32::MAX as u32) as i32;

        let clamped_dx = if dx == 0 {
            0
        } else {
            Self::clamp_axis_delta(bounds.x, bounds.width, screen_width, dx)
        };
        let clamped_dy = if dy == 0 {
            0
        } else {
            Self::clamp_axis_delta(bounds.y, bounds.height, screen_height, dy)
        };

        Some((clamped_dx, clamped_dy))
    }
}
