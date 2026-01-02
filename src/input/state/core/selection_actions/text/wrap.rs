use crate::draw::{Shape, ShapeId};
use crate::input::InputState;

const TEXT_WRAP_MIN_WIDTH: i32 = 40;

impl InputState {
    pub(crate) fn clamp_text_wrap_width(&self, base_x: i32, cursor_x: i32, size: f64) -> i32 {
        let min_width = (size * 2.0).round().max(TEXT_WRAP_MIN_WIDTH as f64) as i32;
        let raw = cursor_x - base_x;
        let mut width = raw.max(1);
        let screen_width = self.screen_width.min(i32::MAX as u32) as i32;
        if screen_width > 0 {
            let max_width = screen_width.saturating_sub(base_x).max(1);
            let target_min = min_width.min(max_width);
            width = width.max(target_min);
            width = width.min(max_width);
        } else {
            width = width.max(min_width);
        }
        width
    }

    pub(crate) fn update_text_wrap_width(&mut self, shape_id: ShapeId, new_width: i32) -> bool {
        let updated = {
            let frame = self.boards.active_frame_mut();
            if let Some(shape) = frame.shape_mut(shape_id) {
                if shape.locked {
                    return false;
                }
                let before = shape.shape.bounding_box();
                match &mut shape.shape {
                    Shape::Text { wrap_width, .. } | Shape::StickyNote { wrap_width, .. } => {
                        if *wrap_width == Some(new_width) {
                            return false;
                        }
                        *wrap_width = Some(new_width);
                    }
                    _ => return false,
                }
                let after = shape.shape.bounding_box();
                Some((before, after))
            } else {
                None
            }
        };

        if let Some((before, after)) = updated {
            self.mark_selection_dirty_region(before);
            self.mark_selection_dirty_region(after);
            self.invalidate_hit_cache_for(shape_id);
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }
}
