use super::base::{
    DrawingState, TextBlockDrag, TextClickState, TextEditEntryFeedback, TextInputMode,
};
use super::ime::ImeCompositionState;
use crate::draw::frame::{Frame, ShapeSnapshot, UndoAction};
use crate::draw::{Color, FontDescriptor, Shape, ShapeId};
use crate::util::Rect;

/// Text-editor mode, asynchronous identity, composition, and pointer state.
#[derive(Debug, Clone)]
pub(crate) struct TextEditing {
    pub(crate) text_input_mode: TextInputMode,
    pub(crate) text_input_cursor_rect_dirty: bool,
    pub(crate) text_input_external_change_dirty: bool,
    pub(crate) text_input_generation: u64,
    pub(crate) text_input_revision: u64,
    pub(crate) text_edit_target: Option<(ShapeId, ShapeSnapshot)>,
    pub(crate) text_block_drag: Option<TextBlockDrag>,
    pub(crate) text_edit_entry_feedback: Option<TextEditEntryFeedback>,
    pub(crate) ime: ImeCompositionState,
    pub(crate) last_text_click: Option<TextClickState>,
    pub(crate) last_text_preview_bounds: Option<Rect>,
}

pub(crate) struct TextEditStart {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) text: String,
    pub(crate) color: Color,
    pub(crate) size: f64,
    pub(crate) font_descriptor: FontDescriptor,
    pub(crate) background_enabled: Option<bool>,
    pub(crate) wrap_width: Option<i32>,
    pub(crate) shape_id: ShapeId,
    pub(crate) before_bounds: Option<Rect>,
    pub(crate) after_bounds: Option<Rect>,
}

pub(crate) struct TextShapeChange {
    pub(crate) shape_id: ShapeId,
    pub(crate) before_bounds: Option<Rect>,
    pub(crate) after_bounds: Option<Rect>,
}

impl Default for TextEditing {
    fn default() -> Self {
        Self {
            text_input_mode: TextInputMode::Plain,
            text_input_cursor_rect_dirty: false,
            text_input_external_change_dirty: false,
            text_input_generation: 0,
            text_input_revision: 0,
            text_edit_target: None,
            text_block_drag: None,
            text_edit_entry_feedback: None,
            ime: ImeCompositionState::default(),
            last_text_click: None,
            last_text_preview_bounds: None,
        }
    }
}

impl TextEditing {
    pub(crate) fn mode(&self) -> TextInputMode {
        self.text_input_mode
    }

    #[cfg(test)]
    pub(crate) fn set_mode(&mut self, mode: TextInputMode) {
        self.text_input_mode = mode;
    }

    pub(crate) fn prepare_new(&mut self, mode: TextInputMode) {
        self.text_input_mode = mode;
        self.text_edit_target = None;
        self.last_text_preview_bounds = None;
    }

    #[cfg(test)]
    pub(crate) fn revision(&self) -> u64 {
        self.text_input_revision
    }

    pub(crate) fn edit_target(&self) -> Option<&(ShapeId, ShapeSnapshot)> {
        self.text_edit_target.as_ref()
    }

    pub(crate) fn can_begin_existing(&self, frame: &Frame, shape_id: ShapeId) -> bool {
        frame.shape(shape_id).is_some_and(|drawn| {
            !drawn.locked && matches!(&drawn.shape, Shape::Text { .. } | Shape::StickyNote { .. })
        })
    }

    #[cfg(test)]
    pub(crate) fn set_edit_entry_feedback(&mut self, feedback: Option<TextEditEntryFeedback>) {
        self.text_edit_entry_feedback = feedback;
    }

    pub(crate) fn expire_edit_entry_feedback(&mut self, now: std::time::Instant) -> bool {
        let Some(feedback) = &self.text_edit_entry_feedback else {
            return false;
        };
        if now.saturating_duration_since(feedback.started).as_millis()
            < u128::from(super::base::TEXT_EDIT_ENTRY_DURATION_MS)
        {
            return true;
        }
        self.text_edit_entry_feedback = None;
        false
    }

    pub(crate) fn edit_entry_progress(&self, now: std::time::Instant) -> Option<f64> {
        let feedback = self.text_edit_entry_feedback.as_ref()?;
        let elapsed = now.saturating_duration_since(feedback.started).as_millis() as f64;
        let total = super::base::TEXT_EDIT_ENTRY_DURATION_MS as f64;
        Some((elapsed / total).min(1.0))
    }

    pub(crate) fn text_block_drag(&self) -> Option<TextBlockDrag> {
        self.text_block_drag
    }

    pub(crate) fn set_text_block_drag(&mut self, drag: Option<TextBlockDrag>) {
        self.text_block_drag = drag;
    }

    pub(crate) fn begin_block_drag(
        &mut self,
        state: &DrawingState,
        canvas_x: i32,
        canvas_y: i32,
    ) -> bool {
        let DrawingState::TextInput { x, y, .. } = state else {
            return false;
        };
        self.text_block_drag = Some(TextBlockDrag {
            grab_dx: canvas_x - *x,
            grab_dy: canvas_y - *y,
        });
        true
    }

    pub(crate) fn drag_block_to(
        &self,
        state: &mut DrawingState,
        canvas_x: i32,
        canvas_y: i32,
    ) -> bool {
        let Some(drag) = self.text_block_drag else {
            return false;
        };
        let DrawingState::TextInput { x, y, .. } = state else {
            return false;
        };
        *x = canvas_x - drag.grab_dx;
        *y = canvas_y - drag.grab_dy;
        true
    }

    #[cfg(test)]
    pub(crate) fn last_click(&self) -> Option<TextClickState> {
        self.last_text_click
    }

    pub(crate) fn set_last_click(&mut self, click: Option<TextClickState>) {
        self.last_text_click = click;
    }

    pub(crate) fn register_click(
        &mut self,
        shape_id: ShapeId,
        x: i32,
        y: i32,
        now: std::time::Instant,
        max_delay_ms: u64,
        max_distance: i32,
    ) -> bool {
        let is_double = self.last_text_click.is_some_and(|last| {
            last.shape_id == shape_id
                && now.duration_since(last.at).as_millis() <= u128::from(max_delay_ms)
                && (x - last.x).abs() <= max_distance
                && (y - last.y).abs() <= max_distance
        });
        self.last_text_click = (!is_double).then_some(TextClickState {
            shape_id,
            x,
            y,
            at: now,
        });
        is_double
    }

    pub(crate) fn preview_bounds(&self) -> Option<Rect> {
        self.last_text_preview_bounds
    }

    pub(crate) fn replace_preview_bounds(&mut self, bounds: Option<Rect>) -> Option<Rect> {
        std::mem::replace(&mut self.last_text_preview_bounds, bounds)
    }

    pub(crate) fn mark_cursor_rect_dirty(&mut self) {
        self.text_input_cursor_rect_dirty = true;
    }

    pub(crate) fn mark_external_change_dirty(&mut self) {
        self.text_input_external_change_dirty = true;
    }

    pub(crate) fn clear_preview_dirty(&mut self) -> Option<Rect> {
        self.text_input_cursor_rect_dirty = false;
        self.text_input_external_change_dirty = false;
        self.last_text_preview_bounds.take()
    }

    pub(crate) fn take_cursor_rect_dirty(&mut self) -> bool {
        std::mem::take(&mut self.text_input_cursor_rect_dirty)
    }

    pub(crate) fn take_external_change_dirty(&mut self) -> bool {
        std::mem::take(&mut self.text_input_external_change_dirty)
    }

    pub(crate) fn reset_composition_and_pointer(&mut self) {
        self.ime = ImeCompositionState::default();
        self.text_block_drag = None;
    }

    /// Begin editing one existing text shape and hide its committed text while
    /// the live editor owns it.
    pub(crate) fn begin_existing(
        &mut self,
        frame: &mut Frame,
        shape_id: ShapeId,
        now: std::time::Instant,
    ) -> Option<TextEditStart> {
        let drawn = frame.shape(shape_id)?;
        if drawn.locked {
            return None;
        }
        let snapshot = ShapeSnapshot {
            shape: drawn.shape.clone(),
            locked: drawn.locked,
        };
        let (mode, x, y, text, color, size, font_descriptor, background_enabled, wrap_width) =
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
                ),
                _ => return None,
            };

        let shape = frame.shape_mut(shape_id)?;
        let before_bounds = shape.bounding_box();
        match &mut shape.shape {
            Shape::Text { text, .. } | Shape::StickyNote { text, .. } => text.clear(),
            _ => return None,
        }
        shape.invalidate_bounds();
        let after_bounds = shape.bounding_box();

        self.text_input_mode = mode;
        self.text_edit_target = Some((shape_id, snapshot));
        self.text_edit_entry_feedback = Some(TextEditEntryFeedback { started: now });
        self.last_text_preview_bounds = None;
        self.begin_session();

        Some(TextEditStart {
            x,
            y,
            text,
            color,
            size,
            font_descriptor,
            background_enabled,
            wrap_width,
            shape_id,
            before_bounds,
            after_bounds,
        })
    }

    pub(crate) fn cancel_existing(&mut self, frame: &mut Frame) -> Option<TextShapeChange> {
        let (shape_id, snapshot) = self.text_edit_target.take()?;
        let shape = frame.shape_mut(shape_id)?;
        let before_bounds = shape.bounding_box();
        shape.set_shape(snapshot.shape);
        shape.locked = snapshot.locked;
        let after_bounds = shape.bounding_box();
        Some(TextShapeChange {
            shape_id,
            before_bounds,
            after_bounds,
        })
    }

    pub(crate) fn commit_existing(
        &mut self,
        frame: &mut Frame,
        new_shape: Shape,
        undo_stack_limit: usize,
    ) -> Option<TextShapeChange> {
        let (shape_id, before) = self.text_edit_target.take()?;
        let shape = frame.shape_mut(shape_id)?;
        let before_bounds = shape.bounding_box();
        shape.set_shape(new_shape);
        let after_bounds = shape.bounding_box();
        let after = ShapeSnapshot {
            shape: shape.shape.clone(),
            locked: shape.locked,
        };
        frame.push_undo_action(
            UndoAction::Modify {
                shape_id,
                before,
                after,
            },
            undo_stack_limit,
        );
        Some(TextShapeChange {
            shape_id,
            before_bounds,
            after_bounds,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_starts_a_plain_editor_without_an_active_session() {
        let editing = TextEditing::default();

        assert_eq!(editing.text_input_mode, TextInputMode::Plain);
        assert_eq!(
            (editing.text_input_generation, editing.text_input_revision),
            (0, 0)
        );
        assert!(editing.text_edit_target.is_none());
        assert!(editing.text_block_drag.is_none());
        assert!(editing.text_edit_entry_feedback.is_none());
        assert!(editing.ime.preedit().is_none());
        assert!(editing.last_text_click.is_none());
        assert!(editing.last_text_preview_bounds.is_none());
        assert!(!editing.text_input_cursor_rect_dirty);
        assert!(!editing.text_input_external_change_dirty);
    }

    fn text_shape(text: &str) -> Shape {
        Shape::Text {
            x: 20,
            y: 30,
            text: text.to_string(),
            color: crate::draw::RED,
            size: 24.0,
            font_descriptor: FontDescriptor::default(),
            background_enabled: false,
            wrap_width: Some(240),
        }
    }

    #[test]
    fn existing_edit_hides_then_restores_the_original_shape() {
        let mut frame = Frame::new();
        let shape_id = frame.add_shape(text_shape("before"));
        let mut editing = TextEditing::default();

        let started = editing
            .begin_existing(&mut frame, shape_id, std::time::Instant::now())
            .expect("editable text shape");

        assert_eq!(started.text, "before");
        assert_eq!(editing.edit_target().map(|(id, _)| *id), Some(shape_id));
        assert!(matches!(
            &frame.shape(shape_id).expect("shape remains").shape,
            Shape::Text { text, .. } if text.is_empty()
        ));

        let restored = editing
            .cancel_existing(&mut frame)
            .expect("active edit restores");
        assert_eq!(restored.shape_id, shape_id);
        assert!(editing.edit_target().is_none());
        assert!(matches!(
            &frame.shape(shape_id).expect("shape remains").shape,
            Shape::Text { text, .. } if text == "before"
        ));
    }

    #[test]
    fn committing_an_existing_edit_records_one_undoable_replacement() {
        let mut frame = Frame::new();
        let shape_id = frame.add_shape(text_shape("before"));
        let mut editing = TextEditing::default();
        editing
            .begin_existing(&mut frame, shape_id, std::time::Instant::now())
            .expect("editable text shape");

        editing
            .commit_existing(&mut frame, text_shape("after"), 10)
            .expect("active edit commits");

        assert!(editing.edit_target().is_none());
        assert!(matches!(
            &frame.shape(shape_id).expect("shape remains").shape,
            Shape::Text { text, .. } if text == "after"
        ));
        frame.undo_last().expect("replacement is undoable");
        assert!(matches!(
            &frame.shape(shape_id).expect("shape remains").shape,
            Shape::Text { text, .. } if text == "before"
        ));
    }
}
