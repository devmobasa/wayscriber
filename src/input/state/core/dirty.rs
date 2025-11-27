use super::base::{DrawingState, InputState};
use crate::draw::shape::{
    bounding_box_for_arrow, bounding_box_for_ellipse, bounding_box_for_eraser,
    bounding_box_for_line, bounding_box_for_points, bounding_box_for_rect, bounding_box_for_text,
};
use crate::input::tool::Tool;
use crate::util::{self, Rect};

impl InputState {
    /// Clears any cached provisional shape bounds and marks their damage region.
    pub(crate) fn clear_provisional_dirty(&mut self) {
        if let Some(prev) = self.last_provisional_bounds.take() {
            self.dirty_tracker.mark_rect(prev);
        }
    }

    /// Updates tracked provisional shape bounds for dirty-region purposes.
    pub(crate) fn update_provisional_dirty(&mut self, current_x: i32, current_y: i32) {
        let new_bounds = self.compute_provisional_bounds(current_x, current_y);
        let previous = self.last_provisional_bounds;

        if new_bounds != previous {
            if let Some(prev) = previous {
                self.dirty_tracker.mark_rect(prev);
            }
        }

        if let Some(bounds) = new_bounds {
            self.dirty_tracker.mark_rect(bounds);
            self.last_provisional_bounds = Some(bounds);
        } else {
            self.last_provisional_bounds = None;
        }
    }

    fn compute_provisional_bounds(&self, current_x: i32, current_y: i32) -> Option<Rect> {
        if let DrawingState::Drawing {
            tool,
            start_x,
            start_y,
            points,
        } = &self.state
        {
            match tool {
                Tool::Pen => bounding_box_for_points(points, self.current_thickness),
                Tool::Marker => {
                    let inflated =
                        (self.current_thickness * 1.35).max(self.current_thickness + 1.0);
                    bounding_box_for_points(points, inflated)
                }
                Tool::Eraser => bounding_box_for_eraser(points, self.eraser_size),
                Tool::Line => bounding_box_for_line(
                    *start_x,
                    *start_y,
                    current_x,
                    current_y,
                    self.current_thickness,
                ),
                Tool::Rect => {
                    let (x, w) = if current_x >= *start_x {
                        (*start_x, current_x - start_x)
                    } else {
                        (current_x, start_x - current_x)
                    };
                    let (y, h) = if current_y >= *start_y {
                        (*start_y, current_y - start_y)
                    } else {
                        (current_y, start_y - current_y)
                    };
                    bounding_box_for_rect(x, y, w, h, self.current_thickness)
                }
                Tool::Ellipse => {
                    let (cx, cy, rx, ry) =
                        util::ellipse_bounds(*start_x, *start_y, current_x, current_y);
                    bounding_box_for_ellipse(cx, cy, rx, ry, self.current_thickness)
                }
                Tool::Arrow => bounding_box_for_arrow(
                    *start_x,
                    *start_y,
                    current_x,
                    current_y,
                    self.current_thickness,
                    self.arrow_length,
                    self.arrow_angle,
                ),
                Tool::Highlight => None,
                Tool::Select => None,
            }
        } else {
            None
        }
    }

    /// Updates dirty tracking for the live text preview/caret overlay.
    pub(crate) fn update_text_preview_dirty(&mut self) {
        let new_bounds = self.compute_text_preview_bounds();
        let previous = self.last_text_preview_bounds;

        if new_bounds != previous {
            if let Some(prev) = previous {
                self.dirty_tracker.mark_rect(prev);
            }
        }

        if let Some(bounds) = new_bounds {
            self.dirty_tracker.mark_rect(bounds);
            self.last_text_preview_bounds = Some(bounds);
        } else {
            self.last_text_preview_bounds = None;
        }
    }

    /// Clears the cached text preview bounds.
    pub(crate) fn clear_text_preview_dirty(&mut self) {
        if let Some(prev) = self.last_text_preview_bounds.take() {
            self.dirty_tracker.mark_rect(prev);
        }
    }

    fn compute_text_preview_bounds(&self) -> Option<Rect> {
        if let DrawingState::TextInput { x, y, buffer } = &self.state {
            let mut preview = buffer.clone();
            preview.push('_');
            bounding_box_for_text(
                *x,
                *y,
                &preview,
                self.current_font_size,
                &self.font_descriptor,
                self.text_background_enabled,
            )
        } else {
            None
        }
    }
}
