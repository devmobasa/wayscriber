use super::super::super::base::InputState;
use super::super::BoardPickerMode;

impl InputState {
    fn board_picker_board_order(&self) -> Vec<usize> {
        self.board_picker_board_order_for_mode(self.board_picker_mode())
    }

    fn board_picker_board_order_for_mode(&self, _mode: BoardPickerMode) -> Vec<usize> {
        let board_count = self.boards.board_count();
        let mut order = Vec::with_capacity(board_count);
        for (index, board) in self.boards.board_states().iter().enumerate() {
            if board.spec.pinned {
                order.push(index);
            }
        }
        for (index, board) in self.boards.board_states().iter().enumerate() {
            if !board.spec.pinned {
                order.push(index);
            }
        }
        order
    }

    pub(super) fn board_picker_clamp_drag_row(&self, row: usize, source_board: usize) -> usize {
        let board_count = self.boards.board_count();
        if board_count == 0 {
            return row;
        }
        let pinned_count = self
            .boards
            .board_states()
            .iter()
            .filter(|board| board.spec.pinned)
            .count();
        let is_pinned = self
            .boards
            .board_states()
            .get(source_board)
            .is_some_and(|board| board.spec.pinned);
        let min = if is_pinned { 0 } else { pinned_count };
        let max = if is_pinned {
            pinned_count.saturating_sub(1)
        } else {
            board_count.saturating_sub(1)
        };
        row.clamp(min, max)
    }

    pub(crate) fn board_picker_board_index_for_row(&self, row: usize) -> Option<usize> {
        self.board_picker_board_index_for_row_in_mode(row, self.board_picker_mode())
    }

    /// Returns the count of pinned boards.
    pub fn board_picker_pinned_count(&self) -> usize {
        self.boards
            .board_states()
            .iter()
            .filter(|board| board.spec.pinned)
            .count()
    }

    pub(crate) fn board_picker_row_for_board(&self, board_index: usize) -> Option<usize> {
        self.board_picker_row_for_board_in_mode(board_index, self.board_picker_mode())
    }

    pub(super) fn board_picker_board_index_for_row_in_mode(
        &self,
        row: usize,
        mode: BoardPickerMode,
    ) -> Option<usize> {
        if row >= self.boards.board_count() {
            return None;
        }
        let order = self.board_picker_board_order_for_mode(mode);
        order.get(row).copied()
    }

    pub(super) fn board_picker_row_for_board_in_mode(
        &self,
        board_index: usize,
        mode: BoardPickerMode,
    ) -> Option<usize> {
        if board_index >= self.boards.board_count() {
            return None;
        }
        let order = self.board_picker_board_order_for_mode(mode);
        order.iter().position(|&index| index == board_index)
    }
}
