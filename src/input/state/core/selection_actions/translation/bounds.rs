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

    fn clamp_axis_delta(position: i32, size: i32, start: i32, extent: i32, delta: i32) -> i32 {
        if extent <= 0 || size <= 0 {
            return delta;
        }

        let position = i64::from(position);
        let end = position + i64::from(size);
        let start = i64::from(start);
        let view_end = start + i64::from(extent);
        let (min_delta, max_delta) = if size <= extent {
            (start - position, view_end - end)
        } else {
            (view_end - end, start - position)
        };

        // A panned or zoomed view may already clip the selection. Stop further
        // outward movement without forcing it back into view against the drag.
        // Oversized selections can align past the viewport, but their painted
        // bounds must stay within the coordinate range used by shape transforms.
        let min_delta = min_delta.min(0).max(i64::from(i32::MIN) - position);
        let max_delta = max_delta.max(0).min((i64::from(i32::MAX) - end).max(0));
        let clamped = i64::from(delta).clamp(min_delta, max_delta);
        i32::try_from(clamped)
            .expect("clamped movement lies between zero and the requested i32 delta")
    }

    pub(super) fn clamp_selection_translation(
        &self,
        measurer: &TextMeasurer,
        dx: i32,
        dy: i32,
    ) -> Option<(i32, i32)> {
        let bounds = self.movable_selection_bounds(measurer)?;
        let visible = self.visible_canvas_rect();
        let (screen_width, screen_height) = self.view.screen_size();

        // Shape bounds and pointer deltas are canvas coordinates, so clamp
        // against the viewport after applying the board pan and zoom.
        let clamped_dx = if dx == 0 || screen_width == 0 {
            dx
        } else {
            Self::clamp_axis_delta(bounds.x, bounds.width, visible.x, visible.width, dx)
        };
        let clamped_dy = if dy == 0 || screen_height == 0 {
            dy
        } else {
            Self::clamp_axis_delta(bounds.y, bounds.height, visible.y, visible.height, dy)
        };

        let frame = self.boards.active_frame();
        let translation = self
            .selected_shape_ids()
            .iter()
            .filter_map(|id| frame.shape(*id))
            .filter(|drawn| !drawn.locked)
            .fold((clamped_dx, clamped_dy), |(dx, dy), drawn| {
                drawn.shape.constrain_translation(dx, dy)
            });

        Some(translation)
    }
}
