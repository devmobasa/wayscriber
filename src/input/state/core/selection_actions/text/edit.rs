use crate::draw::{TextMeasurer, with_legacy_measurer};
use crate::input::{DrawingState, InputState};
use std::time::Instant;

impl InputState {
    pub(crate) fn edit_selected_text(&mut self) -> bool {
        with_legacy_measurer(|measurer| self.edit_selected_text_with(measurer))
    }

    pub(crate) fn edit_selected_text_with(&mut self, measurer: &TextMeasurer) -> bool {
        if self.selected_shape_ids().len() != 1 {
            return false;
        }
        let shape_id = self.selected_shape_ids()[0];
        if let (DrawingState::TextInput { .. }, Some((editing_id, _))) =
            (&self.state, self.text_editing.edit_target())
            && *editing_id == shape_id
        {
            return true;
        }
        if !self
            .text_editing
            .can_begin_existing(self.boards.active_frame(), shape_id)
        {
            return false;
        }

        if matches!(self.state, DrawingState::TextInput { .. }) {
            self.cancel_text_input_with(measurer);
        }

        let Some(start) = self.text_editing.begin_existing(
            measurer,
            self.boards.active_frame_mut(),
            shape_id,
            Instant::now(),
        ) else {
            return false;
        };
        self.clear_pending_text_pastes();

        let _ = self.set_color(start.color);
        let _ = self.set_font_size(start.size);
        let _ = self.set_font_descriptor(start.font_descriptor);
        if let Some(background_enabled) = start.background_enabled
            && self.style.text_background_enabled != background_enabled
        {
            self.style.text_background_enabled = background_enabled;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            self.mark_session_dirty();
        }
        self.style.text_wrap_width = start.wrap_width;
        self.state = DrawingState::text_input(start.x, start.y, start.text);
        self.update_text_preview_dirty_with(measurer);

        self.dirty_tracker.mark_optional_rect(start.before_bounds);
        self.dirty_tracker.mark_optional_rect(start.after_bounds);
        self.invalidate_hit_cache_for_with(measurer, start.shape_id);
        self.needs_redraw = true;
        true
    }

    pub(crate) fn cancel_text_edit_with(&mut self, measurer: &TextMeasurer) -> bool {
        let Some(change) = self
            .text_editing
            .cancel_existing(measurer, self.boards.active_frame_mut())
        else {
            return false;
        };
        self.dirty_tracker.mark_optional_rect(change.before_bounds);
        self.dirty_tracker.mark_optional_rect(change.after_bounds);
        self.invalidate_hit_cache_for_with(measurer, change.shape_id);
        self.needs_redraw = true;
        true
    }

    pub(crate) fn commit_text_edit_with(
        &mut self,
        measurer: &TextMeasurer,
        new_shape: crate::draw::Shape,
    ) -> bool {
        let undo_limit = self.history_limits.undo_stack_limit();
        let Some(change) = self.text_editing.commit_existing(
            measurer,
            self.boards.active_frame_mut(),
            new_shape,
            undo_limit,
        ) else {
            return false;
        };
        self.dirty_tracker.mark_optional_rect(change.before_bounds);
        self.dirty_tracker.mark_optional_rect(change.after_bounds);
        self.invalidate_hit_cache_for_with(measurer, change.shape_id);
        self.needs_redraw = true;
        self.mark_session_dirty();
        true
    }
}
