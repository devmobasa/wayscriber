use crate::util::Rect;

use super::super::super::base::InputState;
use super::super::{
    BoardPickerCursorHint, BoardPickerEditMode, BoardPickerLayout, BoardPickerState,
};

impl InputState {
    pub(crate) fn update_board_picker_hover_from_pointer(&mut self, x: i32, y: i32) {
        if !self.is_board_picker_open() {
            return;
        }
        let hover = self.board_picker_index_at(x, y);
        if let BoardPickerState::Open { hover_index, .. } = &mut self.board_picker_state
            && *hover_index != hover
        {
            *hover_index = hover;
            self.needs_redraw = true;
        }
    }

    /// Determine the cursor type for a given point within the board picker.
    /// Returns `None` if the board picker is not open or the point is outside.
    pub(crate) fn board_picker_cursor_hint_at(
        &self,
        x: i32,
        y: i32,
    ) -> Option<BoardPickerCursorHint> {
        if !self.is_board_picker_open() {
            return None;
        }
        let layout = self.board_picker_layout?;

        // Check if point is within the panel
        if !self.board_picker_contains_point(x, y) {
            return None;
        }

        // Check if currently dragging a board (grabbing)
        if self.board_picker_drag.is_some() || self.board_picker_page_drag.is_some() {
            return Some(BoardPickerCursorHint::Grabbing);
        }

        // Check drag handles first (grab cursor)
        if self.board_picker_handle_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Grab);
        }
        if self.board_picker_page_handle_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Grab);
        }
        if self.board_picker_page_add_card_at(x, y) {
            return Some(BoardPickerCursorHint::Pointer);
        }
        if self.board_picker_page_overflow_at(x, y) {
            return Some(BoardPickerCursorHint::Pointer);
        }
        if self.board_picker_page_rename_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Pointer);
        }
        if self.board_picker_page_name_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Text);
        }
        if self.board_picker_open_icon_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Pointer);
        }

        // Check if in edit mode and hovering over the edit row
        if let Some((edit_mode, edit_index, _)) = self.board_picker_edit_state()
            && let Some(row_index) = self.board_picker_index_at(x, y)
            && row_index == edit_index
        {
            // If editing name or hex, show text cursor over the row
            match edit_mode {
                BoardPickerEditMode::Name => return Some(BoardPickerCursorHint::Text),
                BoardPickerEditMode::Color => {
                    // Check palette first
                    if layout.palette_rows > 0 && self.board_picker_palette_color_at(x, y).is_some()
                    {
                        return Some(BoardPickerCursorHint::Pointer);
                    }
                    // In color edit mode, text cursor for hex input
                    return Some(BoardPickerCursorHint::Text);
                }
            }
        }

        // Check palette swatches (pointer)
        if layout.palette_rows > 0 && self.board_picker_palette_color_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Pointer);
        }

        // Check color swatches (pointer for color edit)
        if self.board_picker_swatch_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Pointer);
        }

        // Check page thumbnails (pointer)
        if self.board_picker_page_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Pointer);
        }

        // Check pin icons (pointer)
        if self.board_picker_pin_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Pointer);
        }

        // Check board rows (pointer for selection)
        if self.board_picker_index_at(x, y).is_some() {
            return Some(BoardPickerCursorHint::Pointer);
        }

        // Default within the picker panel
        Some(BoardPickerCursorHint::Default)
    }

    pub(crate) fn mark_board_picker_region(&mut self, layout: BoardPickerLayout) {
        let x = layout.origin_x.floor() as i32;
        let y = layout.origin_y.floor() as i32;
        let width = layout.width.ceil() as i32 + 2;
        let height = layout.height.ceil() as i32 + 2;
        if let Some(rect) = Rect::new(x, y, width.max(1), height.max(1)) {
            self.dirty_tracker.mark_rect(rect);
        } else {
            self.dirty_tracker.mark_full();
        }
    }
}
