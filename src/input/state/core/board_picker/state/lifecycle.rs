use super::super::super::base::InputState;
use super::super::{BoardPickerFocus, BoardPickerMode, BoardPickerState};

impl InputState {
    pub(crate) fn is_board_picker_open(&self) -> bool {
        matches!(self.board_picker_state, BoardPickerState::Open { .. })
    }

    pub(crate) fn board_picker_mode(&self) -> BoardPickerMode {
        match &self.board_picker_state {
            BoardPickerState::Open { mode, .. } => *mode,
            BoardPickerState::Hidden => BoardPickerMode::Full,
        }
    }

    pub(crate) fn board_picker_is_quick(&self) -> bool {
        self.board_picker_mode() == BoardPickerMode::Quick
    }

    pub(crate) fn open_board_picker(&mut self) {
        if self.show_help {
            self.toggle_help_overlay();
        }
        self.cancel_active_interaction();
        self.close_context_menu();
        self.close_properties_panel();
        self.board_picker_clear_search();
        self.board_picker_drag = None;
        self.board_picker_page_drag = None;
        self.board_picker_page_edit = None;
        let active_index = self.boards.active_index();
        self.board_picker_state = BoardPickerState::Open {
            selected: active_index,
            hover_index: None,
            edit: None,
            mode: BoardPickerMode::Full,
            focus: BoardPickerFocus::BoardList,
            page_focus_index: None,
        };
        let selected_row = self.board_picker_row_for_board(active_index);
        if let (Some(selected), BoardPickerState::Open { selected: row, .. }) =
            (selected_row, &mut self.board_picker_state)
        {
            *row = selected;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn open_board_picker_quick(&mut self) {
        if self.show_help {
            self.toggle_help_overlay();
        }
        self.cancel_active_interaction();
        self.close_context_menu();
        self.close_properties_panel();
        self.board_picker_clear_search();
        self.board_picker_drag = None;
        self.board_picker_page_drag = None;
        self.board_picker_page_edit = None;
        let active_index = self.boards.active_index();
        self.board_picker_state = BoardPickerState::Open {
            selected: active_index,
            hover_index: None,
            edit: None,
            mode: BoardPickerMode::Quick,
            focus: BoardPickerFocus::BoardList,
            page_focus_index: None,
        };
        let selected_row = self.board_picker_row_for_board(active_index);
        if let (Some(selected), BoardPickerState::Open { selected: row, .. }) =
            (selected_row, &mut self.board_picker_state)
        {
            *row = selected;
        }
        self.board_picker_select_recent();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn close_board_picker(&mut self) {
        if let Some(layout) = self.board_picker_layout {
            self.mark_board_picker_region(layout);
        }
        // Board picker dims the entire screen; ensure full redraw when closing.
        self.dirty_tracker.mark_full();
        self.board_picker_state = BoardPickerState::Hidden;
        self.board_picker_drag = None;
        self.board_picker_page_drag = None;
        self.board_picker_page_edit = None;
        self.board_picker_layout = None;
        self.last_board_picker_click = None;
        self.board_picker_clear_search();
        self.needs_redraw = true;
    }

    pub(crate) fn toggle_board_picker(&mut self) {
        if self.is_board_picker_open() {
            self.close_board_picker();
        } else {
            self.open_board_picker();
        }
    }

    pub(crate) fn toggle_board_picker_quick(&mut self) {
        if self.is_board_picker_open() {
            self.close_board_picker();
        } else {
            self.open_board_picker_quick();
        }
    }

    pub(crate) fn board_picker_active_index(&self) -> Option<usize> {
        match &self.board_picker_state {
            BoardPickerState::Open {
                hover_index,
                selected,
                ..
            } => hover_index.or(Some(*selected)),
            BoardPickerState::Hidden => None,
        }
    }

    pub(crate) fn board_picker_selected_index(&self) -> Option<usize> {
        match &self.board_picker_state {
            BoardPickerState::Open { selected, .. } => Some(*selected),
            BoardPickerState::Hidden => None,
        }
    }

    pub(crate) fn board_picker_page_panel_board_index(&self) -> Option<usize> {
        let selected = self.board_picker_selected_index()?;
        if let Some(board_index) = self.board_picker_board_index_for_row(selected) {
            Some(board_index)
        } else {
            Some(self.boards.active_index())
        }
    }

    pub(crate) fn board_picker_focus(&self) -> BoardPickerFocus {
        match &self.board_picker_state {
            BoardPickerState::Open { focus, .. } => *focus,
            BoardPickerState::Hidden => BoardPickerFocus::BoardList,
        }
    }

    pub(crate) fn board_picker_set_focus(&mut self, new_focus: BoardPickerFocus) {
        if self.board_picker_focus() == new_focus {
            return;
        }
        // Compute active page index before mutably borrowing state,
        // clamped to visible thumbnail count so focus never lands off-screen.
        let active_page = if new_focus == BoardPickerFocus::PagePanel {
            let visible = self
                .board_picker_layout
                .map(|layout| layout.page_visible_count)
                .unwrap_or(0);
            if visible == 0 {
                0
            } else {
                let board_index = self.board_picker_page_panel_board_index_inner();
                let raw = board_index
                    .and_then(|index| self.boards.board_states().get(index))
                    .map_or(0, |board| board.pages.active_index());
                raw.min(visible.saturating_sub(1))
            }
        } else {
            0
        };
        let BoardPickerState::Open {
            focus,
            page_focus_index,
            ..
        } = &mut self.board_picker_state
        else {
            return;
        };
        *focus = new_focus;
        match new_focus {
            BoardPickerFocus::PagePanel => {
                if page_focus_index.is_none() {
                    *page_focus_index = Some(active_page);
                }
            }
            BoardPickerFocus::BoardList => {
                *page_focus_index = None;
            }
        }
        self.needs_redraw = true;
        self.dirty_tracker.mark_full();
    }

    pub(crate) fn board_picker_page_focus_index(&self) -> Option<usize> {
        match &self.board_picker_state {
            BoardPickerState::Open {
                page_focus_index, ..
            } => *page_focus_index,
            BoardPickerState::Hidden => None,
        }
    }

    pub(crate) fn board_picker_set_page_focus_index(&mut self, index: usize) {
        let visible = self
            .board_picker_layout
            .map(|layout| layout.page_visible_count)
            .unwrap_or(0);
        let clamped = if visible == 0 {
            0
        } else {
            index.min(visible.saturating_sub(1))
        };
        if let BoardPickerState::Open {
            page_focus_index, ..
        } = &mut self.board_picker_state
        {
            *page_focus_index = Some(clamped);
            self.needs_redraw = true;
            self.dirty_tracker.mark_full();
        }
    }

    /// Internal helper that reads page panel board index without borrowing &mut self.
    fn board_picker_page_panel_board_index_inner(&self) -> Option<usize> {
        let selected = self.board_picker_selected_index()?;
        if let Some(board_index) = self.board_picker_board_index_for_row(selected) {
            Some(board_index)
        } else {
            Some(self.boards.active_index())
        }
    }

    pub(crate) fn board_picker_set_selected(&mut self, index: usize) {
        let row_count = self.board_picker_row_count().max(1);
        let next = index.min(row_count.saturating_sub(1));
        if let BoardPickerState::Open {
            selected,
            focus,
            page_focus_index,
            ..
        } = &mut self.board_picker_state
        {
            *selected = next;
            *focus = BoardPickerFocus::BoardList;
            *page_focus_index = None;
            self.needs_redraw = true;
            self.dirty_tracker.mark_full();
        }
    }

    pub(crate) fn board_picker_is_dragging(&self) -> bool {
        self.board_picker_drag.is_some()
    }

    pub(crate) fn board_picker_is_page_dragging(&self) -> bool {
        self.board_picker_page_drag.is_some()
    }
}
