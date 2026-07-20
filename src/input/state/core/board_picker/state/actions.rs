use super::super::super::base::InputState;
use super::super::{BoardPickerEditMode, BoardPickerState};
use crate::draw::PageDeleteOutcome;
use crate::input::state::{Toast, ToastPriority};

impl InputState {
    pub(crate) fn board_picker_row_count(&self) -> usize {
        let board_count = self.boards.board_count();
        if self.board_picker_is_quick() {
            board_count
        } else {
            board_count + 1
        }
    }

    pub(crate) fn board_picker_is_new_row(&self, index: usize) -> bool {
        !self.board_picker_is_quick() && index >= self.boards.board_count()
    }

    pub(crate) fn board_picker_activate_row(&mut self, index: usize) {
        let board_count = self.boards.board_count();
        if index < board_count {
            if let Some(board_index) = self.board_picker_board_index_for_row(index) {
                self.switch_board_slot(board_index);
                self.close_board_picker();
            }
        } else {
            self.board_picker_create_new();
        }
    }

    pub(crate) fn board_picker_activate_page(&mut self, page_index: usize) {
        let Some(board_index) = self.board_picker_page_panel_board_index() else {
            return;
        };
        let page_count = self
            .boards
            .board_states()
            .get(board_index)
            .map_or(0, |board| board.pages.page_count());
        if page_index >= page_count {
            return;
        }
        if self.boards.active_index() != board_index {
            self.switch_board_slot(board_index);
        }
        self.switch_to_page(page_index);
        self.close_board_picker();
    }

    pub(crate) fn board_picker_add_page(&mut self) {
        let Some(board_index) = self.board_picker_page_panel_board_index() else {
            return;
        };
        if self.add_page_in_board(board_index)
            && let Some(page_index) = self
                .boards
                .board_states()
                .get(board_index)
                .map(|board| board.pages.active_index())
        {
            self.board_picker_queue_page_scroll_to(page_index);
            self.board_picker_set_page_focus_page_index(page_index);
            self.board_picker_reconcile_page_nav_after_page_change();
        }
    }

    pub(crate) fn board_picker_delete_page(&mut self, page_index: usize) -> PageDeleteOutcome {
        let Some(board_index) = self.board_picker_page_panel_board_index() else {
            return PageDeleteOutcome::Pending;
        };
        let outcome = self.delete_page_in_board(board_index, page_index);
        if !matches!(outcome, PageDeleteOutcome::Pending) {
            self.board_picker_reconcile_page_nav_after_page_change();
        }
        outcome
    }

    pub(crate) fn board_picker_duplicate_page(&mut self, page_index: usize) {
        let Some(board_index) = self.board_picker_page_panel_board_index() else {
            return;
        };
        if self.duplicate_page_in_board(board_index, page_index)
            && let Some(new_page_index) = self
                .boards
                .board_states()
                .get(board_index)
                .map(|board| board.pages.active_index())
        {
            self.board_picker_queue_page_scroll_to(new_page_index);
            self.board_picker_set_page_focus_page_index(new_page_index);
            self.board_picker_reconcile_page_nav_after_page_change();
        }
    }

    pub(crate) fn board_picker_create_new(&mut self) {
        if self.board_picker_is_quick() {
            self.board_picker_promote_to_full();
        }
        if !self.create_board() {
            self.push_toast(
                ToastPriority::Info,
                "board_picker",
                Toast::warning("Board limit reached."),
            );
            return;
        }
        let index = self.boards.active_index();
        if let Some(row) = self.board_picker_row_for_board(index) {
            self.board_picker_set_selected(row);
        }
        let name = self.boards.active_board_name().to_string();
        self.board_picker_start_edit(BoardPickerEditMode::Name, name);
        self.needs_redraw = true;
    }

    pub(crate) fn board_picker_delete_selected(&mut self) {
        let Some(index) = self.board_picker_selected_index() else {
            return;
        };
        if self.board_picker_is_new_row(index) {
            return;
        }
        let Some(board_index) = self.board_picker_board_index_for_row(index) else {
            return;
        };
        if self.boards.active_index() != board_index {
            self.switch_board_slot(board_index);
        }
        self.delete_active_board();
        if let Some(row) = self.board_picker_row_for_board(self.boards.active_index()) {
            self.board_picker_set_selected(row);
        }
    }

    pub(crate) fn board_picker_toggle_pin_selected(&mut self) {
        let Some(index) = self.board_picker_selected_index() else {
            return;
        };
        if self.board_picker_is_new_row(index) {
            return;
        }
        let Some(board_index) = self.board_picker_board_index_for_row(index) else {
            return;
        };
        if !self.toggle_board_pinned(board_index) {
            return;
        }
        let selected_row = self.board_picker_row_for_board(board_index);
        if let (Some(row), BoardPickerState::Open { selected, .. }) =
            (selected_row, &mut self.board_picker_state)
        {
            *selected = row;
        }
        self.board_picker_layout = None;
        self.needs_redraw = true;
    }
}
