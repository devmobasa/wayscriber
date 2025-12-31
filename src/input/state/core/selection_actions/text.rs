use super::super::base::{DrawingState, InputState, TextInputMode};
use crate::draw::frame::{ShapeSnapshot, UndoAction};
use crate::draw::{Shape, ShapeId};
use crate::util::Rect;

const TEXT_RESIZE_HANDLE_SIZE: i32 = 10;
const TEXT_RESIZE_HANDLE_OFFSET: i32 = 6;
const TEXT_WRAP_MIN_WIDTH: i32 = 40;

impl InputState {
    fn text_resize_handle_rect(bounds: Rect) -> Option<Rect> {
        let size = TEXT_RESIZE_HANDLE_SIZE;
        let half = size / 2;
        let center_x = bounds.x + bounds.width + TEXT_RESIZE_HANDLE_OFFSET;
        let center_y = bounds.y + bounds.height + TEXT_RESIZE_HANDLE_OFFSET;
        Rect::new(center_x - half, center_y - half, size, size)
    }

    pub(crate) fn selected_text_resize_handle(&self) -> Option<(ShapeId, Rect)> {
        if self.selected_shape_ids().len() != 1 {
            return None;
        }
        let shape_id = self.selected_shape_ids()[0];
        let frame = self.canvas_set.active_frame();
        let shape = frame.shape(shape_id)?;
        if shape.locked {
            return None;
        }
        if !matches!(shape.shape, Shape::Text { .. } | Shape::StickyNote { .. }) {
            return None;
        }
        let bounds = shape.shape.bounding_box()?;
        let handle = Self::text_resize_handle_rect(bounds)?;
        Some((shape_id, handle))
    }

    pub(crate) fn hit_text_resize_handle(&self, x: i32, y: i32) -> Option<ShapeId> {
        let (shape_id, handle) = self.selected_text_resize_handle()?;
        let tolerance = self.hit_test_tolerance.ceil() as i32;
        let hit_rect = handle.inflated(tolerance).unwrap_or(handle);
        if hit_rect.contains(x, y) {
            Some(shape_id)
        } else {
            None
        }
    }

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
            let frame = self.canvas_set.active_frame_mut();
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
            let frame = self.canvas_set.active_frame();
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
        }
        self.text_wrap_width = wrap_width;

        self.text_edit_target = Some((shape_id, snapshot));
        self.state = DrawingState::TextInput { x, y, buffer: text };
        self.last_text_preview_bounds = None;
        self.update_text_preview_dirty();

        let cleared = {
            let frame = self.canvas_set.active_frame_mut();
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
            let frame = self.canvas_set.active_frame_mut();
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
            let frame = self.canvas_set.active_frame_mut();
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
            true
        } else {
            false
        }
    }
}
