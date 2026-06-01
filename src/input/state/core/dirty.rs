use super::base::{DrawingState, InputState, TextInputMode};
use crate::draw::shape::{
    bounding_box_for_points, bounding_box_for_sticky_note, bounding_box_for_text,
};
use crate::input::tool::PROVISIONAL_POLYGON_DAMAGE_PADDING;
use crate::util::Rect;

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

        if new_bounds != previous
            && let Some(prev) = previous
        {
            self.dirty_tracker.mark_rect(prev);
        }

        if let Some(bounds) = new_bounds {
            self.dirty_tracker.mark_rect(bounds);
            self.last_provisional_bounds = Some(bounds);
        } else {
            self.last_provisional_bounds = None;
        }
    }

    fn compute_provisional_bounds(&self, current_x: i32, current_y: i32) -> Option<Rect> {
        match &self.state {
            DrawingState::Drawing { .. } => {
                self.provisional_tool_stroke(current_x, current_y).bounds()
            }
            DrawingState::Selecting {
                start_x, start_y, ..
            } => Self::selection_rect_from_points(*start_x, *start_y, current_x, current_y)
                .and_then(|rect| rect.inflated(2)),
            DrawingState::BuildingPolygon {
                points,
                preview,
                thick,
                ..
            } => {
                let mut preview_points = points.clone();
                if let Some(point) = preview.or(Some((current_x, current_y))) {
                    preview_points.push(point);
                }
                bounding_box_for_points(&preview_points, *thick)
                    .and_then(|rect| rect.inflated(PROVISIONAL_POLYGON_DAMAGE_PADDING))
            }
            _ => None,
        }
    }

    /// Updates dirty tracking for the live text preview/caret overlay.
    pub(crate) fn update_text_preview_dirty(&mut self) {
        let new_bounds = self.compute_text_preview_bounds();
        let previous = self.last_text_preview_bounds;

        if new_bounds != previous
            && let Some(prev) = previous
        {
            self.dirty_tracker.mark_rect(prev);
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
            match self.text_input_mode {
                TextInputMode::Plain => bounding_box_for_text(
                    *x,
                    *y,
                    &preview,
                    self.current_font_size,
                    &self.font_descriptor,
                    self.text_background_enabled,
                    self.text_wrap_width,
                ),
                TextInputMode::StickyNote => bounding_box_for_sticky_note(
                    *x,
                    *y,
                    &preview,
                    self.current_font_size,
                    &self.font_descriptor,
                    self.text_wrap_width,
                ),
            }
        } else {
            None
        }
    }
}
