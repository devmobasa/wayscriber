use super::super::super::base::InputState;
use super::super::{BoardPickerDrag, BoardPickerPageDrag, BoardPickerState};

impl InputState {
    pub(crate) fn board_picker_start_drag(&mut self, row: usize) -> bool {
        if self.board_picker_is_quick() || self.board_picker_is_new_row(row) {
            return false;
        }
        let Some(board_index) = self.board_picker_board_index_for_row(row) else {
            return false;
        };
        self.board_picker_drag = Some(BoardPickerDrag {
            source_row: row,
            source_board: board_index,
            current_row: row,
        });
        self.board_picker_set_selected(row);
        self.needs_redraw = true;
        true
    }

    pub(crate) fn board_picker_start_page_drag(&mut self, page_index: usize) -> bool {
        if self.board_picker_is_quick() {
            return false;
        }
        let Some(board_index) = self.board_picker_page_panel_board_index() else {
            return false;
        };
        let page_count = self
            .boards
            .board_states()
            .get(board_index)
            .map_or(0, |board| board.pages.page_count());
        if page_index >= page_count {
            return false;
        }
        self.board_picker_page_drag = Some(BoardPickerPageDrag {
            source_index: page_index,
            current_index: page_index,
            board_index,
            target_board: Some(board_index),
        });
        self.needs_redraw = true;
        true
    }

    pub(crate) fn board_picker_update_drag_from_pointer(&mut self, _x: i32, y: i32) {
        let Some(layout) = self.board_picker_layout else {
            return;
        };
        let Some(source_board) = self
            .board_picker_drag
            .as_ref()
            .map(|drag| drag.source_board)
        else {
            return;
        };
        let board_count = self.boards.board_count();
        if board_count == 0 {
            return;
        }
        let rows_top = layout.origin_y + layout.padding_y + layout.header_height;
        let row = ((y as f64 - rows_top) / layout.row_height).floor() as isize;
        let max_row = board_count.saturating_sub(1) as isize;
        let clamped = row.clamp(0, max_row) as usize;
        let target_row = self.board_picker_clamp_drag_row(clamped, source_board);
        if let Some(drag) = &mut self.board_picker_drag
            && drag.current_row != target_row
        {
            drag.current_row = target_row;
            self.board_picker_set_selected(target_row);
            self.needs_redraw = true;
        }
    }

    pub(crate) fn board_picker_update_page_drag_from_pointer(&mut self, x: i32, y: i32) {
        let Some(layout) = self.board_picker_layout else {
            return;
        };
        let Some(drag) = self.board_picker_page_drag else {
            return;
        };
        if !layout.page_panel_enabled {
            return;
        }
        let mut next_target_board = Some(drag.board_index);
        let mut next_current_index = drag.current_index;
        let mut next_hover_row = None;

        if let Some(row) = self.board_picker_index_at(x, y)
            && !self.board_picker_is_new_row(row)
            && let Some(board_index) = self.board_picker_board_index_for_row(row)
        {
            next_target_board = Some(board_index);
            next_hover_row = Some(row);
        } else if let Some(index) = self.board_picker_page_index_at(x, y) {
            next_current_index = index.min(layout.page_count.saturating_sub(1));
        }

        let mut updated = false;
        if let Some(drag) = &mut self.board_picker_page_drag {
            if drag.target_board != next_target_board {
                drag.target_board = next_target_board;
                updated = true;
            }
            if drag.current_index != next_current_index {
                drag.current_index = next_current_index;
                updated = true;
            }
        }

        if let BoardPickerState::Open { hover_index, .. } = &mut self.board_picker_state
            && *hover_index != next_hover_row
        {
            *hover_index = next_hover_row;
            updated = true;
        }

        if updated {
            self.needs_redraw = true;
        }
    }

    pub(crate) fn board_picker_finish_drag(&mut self) -> bool {
        let Some(drag) = self.board_picker_drag.take() else {
            return false;
        };
        let target_row = self.board_picker_clamp_drag_row(drag.current_row, drag.source_board);
        if target_row == drag.source_row {
            self.needs_redraw = true;
            return true;
        }
        let Some(target_board) = self.board_picker_board_index_for_row(target_row) else {
            return true;
        };
        let source_id = self
            .boards
            .board_states()
            .get(drag.source_board)
            .map(|board| board.spec.id.clone());
        if !self.reorder_board(drag.source_board, target_board) {
            return true;
        }
        let Some(source_id) = source_id else {
            return true;
        };
        let Some(new_index) = self
            .boards
            .board_states()
            .iter()
            .position(|board| board.spec.id == source_id)
        else {
            return true;
        };
        if let Some(row) = self.board_picker_row_for_board(new_index) {
            self.board_picker_set_selected(row);
        }
        true
    }

    pub(crate) fn board_picker_finish_page_drag(&mut self) -> bool {
        let Some(drag) = self.board_picker_page_drag.take() else {
            return false;
        };
        let target_board = drag.target_board.unwrap_or(drag.board_index);
        if target_board != drag.board_index {
            let copy = self.modifiers.alt;
            if self.move_page_between_boards(
                drag.board_index,
                drag.source_index,
                target_board,
                copy,
            ) {
                self.switch_board_slot(target_board);
                if let Some(row) = self.board_picker_row_for_board(target_board) {
                    self.board_picker_set_selected(row);
                }
            }
            self.needs_redraw = true;
            return true;
        }
        if drag.source_index == drag.current_index {
            self.needs_redraw = true;
            return true;
        }
        self.reorder_page_in_board(drag.board_index, drag.source_index, drag.current_index);
        self.needs_redraw = true;
        true
    }
}
