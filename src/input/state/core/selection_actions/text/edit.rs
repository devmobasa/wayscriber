use crate::draw::Shape;
use crate::draw::frame::{ShapeSnapshot, UndoAction};
use crate::input::state::core::base::TextEditEntryFeedback;
use crate::input::{DrawingState, InputState, TextInputMode};
use std::time::Instant;

impl InputState {
    pub(crate) fn edit_selected_text(&mut self) -> bool {
        if self.selected_shape_ids().len() != 1 {
            return false;
        }
        let shape_id = self.selected_shape_ids()[0];
        if let (DrawingState::TextInput { .. }, Some((editing_id, _))) =
            (&self.state, self.text_edit_target.as_ref())
            && *editing_id == shape_id
        {
            return true;
        }
        let (
            mode,
            x,
            y,
            text,
            color,
            size,
            font_descriptor,
            background_enabled,
            wrap_width,
            snapshot,
            locked,
        ) = {
            let frame = self.boards.active_frame();
            let Some(drawn) = frame.shape(shape_id) else {
                return false;
            };
            let snapshot = ShapeSnapshot {
                shape: drawn.shape.clone(),
                locked: drawn.locked,
            };
            match &drawn.shape {
                Shape::Text {
                    x,
                    y,
                    text,
                    color,
                    size,
                    font_descriptor,
                    background_enabled,
                    wrap_width,
                } => (
                    TextInputMode::Plain,
                    *x,
                    *y,
                    text.clone(),
                    *color,
                    *size,
                    font_descriptor.clone(),
                    Some(*background_enabled),
                    *wrap_width,
                    snapshot,
                    drawn.locked,
                ),
                Shape::StickyNote {
                    x,
                    y,
                    text,
                    background,
                    size,
                    font_descriptor,
                    wrap_width,
                } => (
                    TextInputMode::StickyNote,
                    *x,
                    *y,
                    text.clone(),
                    *background,
                    *size,
                    font_descriptor.clone(),
                    None,
                    *wrap_width,
                    snapshot,
                    drawn.locked,
                ),
                _ => return false,
            }
        };

        if locked {
            return false;
        }

        if matches!(self.state, DrawingState::TextInput { .. }) {
            self.cancel_text_input();
        }

        self.text_input_mode = mode;
        let _ = self.set_color(color);
        let _ = self.set_font_size(size);
        let _ = self.set_font_descriptor(font_descriptor);
        if let Some(background_enabled) = background_enabled
            && self.text_background_enabled != background_enabled
        {
            self.text_background_enabled = background_enabled;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            self.mark_session_dirty();
        }
        self.text_wrap_width = wrap_width;

        self.text_edit_target = Some((shape_id, snapshot));
        self.text_edit_entry_feedback = Some(TextEditEntryFeedback {
            started: Instant::now(),
        });
        self.state = DrawingState::TextInput { x, y, buffer: text };
        self.last_text_preview_bounds = None;
        self.update_text_preview_dirty();

        let cleared = {
            let frame = self.boards.active_frame_mut();
            if let Some(shape) = frame.shape_mut(shape_id) {
                let before = shape.shape.bounding_box();
                match &mut shape.shape {
                    Shape::Text { text, .. } => {
                        text.clear();
                    }
                    Shape::StickyNote { text, .. } => {
                        text.clear();
                    }
                    _ => {}
                }
                let after = shape.shape.bounding_box();
                Some((before, after))
            } else {
                None
            }
        };

        if let Some((before, after)) = cleared {
            self.dirty_tracker.mark_optional_rect(before);
            self.dirty_tracker.mark_optional_rect(after);
            self.invalidate_hit_cache_for(shape_id);
            self.needs_redraw = true;
        } else {
            self.text_edit_target = None;
        }

        true
    }

    pub(crate) fn cancel_text_edit(&mut self) -> bool {
        let Some((shape_id, snapshot)) = self.text_edit_target.take() else {
            return false;
        };

        let restored = {
            let frame = self.boards.active_frame_mut();
            if let Some(shape) = frame.shape_mut(shape_id) {
                let before = shape.shape.bounding_box();
                shape.shape = snapshot.shape.clone();
                shape.locked = snapshot.locked;
                let after = shape.shape.bounding_box();
                Some((before, after))
            } else {
                None
            }
        };

        if let Some((before, after)) = restored {
            self.dirty_tracker.mark_optional_rect(before);
            self.dirty_tracker.mark_optional_rect(after);
            self.invalidate_hit_cache_for(shape_id);
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }

    pub(crate) fn commit_text_edit(&mut self, new_shape: Shape) -> bool {
        let Some((shape_id, before_snapshot)) = self.text_edit_target.take() else {
            return false;
        };

        let updated = {
            let frame = self.boards.active_frame_mut();
            if let Some(shape) = frame.shape_mut(shape_id) {
                let before_bounds = shape.shape.bounding_box();
                shape.shape = new_shape;
                let after_bounds = shape.shape.bounding_box();
                let after_snapshot = ShapeSnapshot {
                    shape: shape.shape.clone(),
                    locked: shape.locked,
                };
                frame.push_undo_action(
                    UndoAction::Modify {
                        shape_id,
                        before: before_snapshot,
                        after: after_snapshot,
                    },
                    self.undo_stack_limit,
                );
                Some((before_bounds, after_bounds))
            } else {
                None
            }
        };

        if let Some((before_bounds, after_bounds)) = updated {
            self.dirty_tracker.mark_optional_rect(before_bounds);
            self.dirty_tracker.mark_optional_rect(after_bounds);
            self.invalidate_hit_cache_for(shape_id);
            self.needs_redraw = true;
            self.mark_session_dirty();
            true
        } else {
            false
        }
    }
}
