use crate::draw::TextMeasurer;
use crate::input::InputState;
use crate::util::Rect;

impl InputState {
    /// Returns the combined bounding box of all selected shapes (public for rendering).
    pub fn selection_bounds(&self) -> Option<Rect> {
        let measurer = TextMeasurer::default();
        self.selection_bounds_with(&measurer)
    }

    /// Combined selection bounds using the supplied text measurement owner.
    pub fn selection_bounds_with(&self, measurer: &TextMeasurer) -> Option<Rect> {
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
            if let Some(shape) = frame.shape(*id)
                && let Some(bounds) = shape.bounding_box_with(measurer)
            {
                min_x = min_x.min(bounds.x);
                min_y = min_y.min(bounds.y);
                max_x = max_x.max(bounds.x + bounds.width);
                max_y = max_y.max(bounds.y + bounds.height);
                found = true;
            }
        }

        if found {
            Rect::from_min_max(min_x, min_y, max_x, max_y)
        } else {
            None
        }
    }

    pub(super) fn movable_selection_bounds(&self, measurer: &TextMeasurer) -> Option<Rect> {
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
                if let Some(bounds) = shape.bounding_box_with(measurer) {
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

    fn clamp_axis_delta(position: i32, size: i32, start: i32, end_limit: i32, delta: i32) -> i32 {
        if end_limit <= start || size <= 0 {
            return delta;
        }

        let extent = end_limit.saturating_sub(start);
        let end = position.saturating_add(size);
        let (min_delta, max_delta) = if size <= extent {
            (
                start.saturating_sub(position),
                end_limit.saturating_sub(end),
            )
        } else {
            (
                end_limit.saturating_sub(end),
                start.saturating_sub(position),
            )
        };

        delta.clamp(min_delta, max_delta)
    }

    pub(super) fn clamp_selection_translation(
        &self,
        measurer: &TextMeasurer,
        dx: i32,
        dy: i32,
    ) -> Option<(i32, i32)> {
        // Pannable boards (Blackboard, Whiteboard, etc.) have an unbounded canvas;
        // clamping against the screen viewport creates artificial boundaries
        // ("invisible walls") and breaks dragging across the canvas.
        if self.boards.pan_enabled() && !self.board_is_transparent() {
            return Some((dx, dy));
        }

        let bounds = self.movable_selection_bounds(measurer)?;
        let (screen_width, screen_height) = self.view.screen_size();
        let screen_width = screen_width.min(i32::MAX as u32) as i32;
        let screen_height = screen_height.min(i32::MAX as u32) as i32;

        let clamped_dx = if dx == 0 || screen_width <= 0 {
            dx
        } else {
            Self::clamp_axis_delta(bounds.x, bounds.width, 0, screen_width, dx)
        };
        let clamped_dy = if dy == 0 || screen_height <= 0 {
            dy
        } else {
            Self::clamp_axis_delta(bounds.y, bounds.height, 0, screen_height, dy)
        };

        Some((clamped_dx, clamped_dy))
    }
}
