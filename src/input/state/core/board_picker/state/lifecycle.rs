use super::super::super::base::InputState;
use super::super::{BoardPickerFocus, BoardPickerMode, BoardPickerPageNavMode, BoardPickerState};

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
        self.close_radial_menu();
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
        let active_page = self.boards.active_page_index();
        self.board_picker_state = BoardPickerState::Open {
            selected: active_index,
            hover_index: None,
            edit: None,
            mode: BoardPickerMode::Full,
            focus: BoardPickerFocus::BoardList,
            page_focus_page_index: None,
            page_scroll_row: 0,
            page_scroll_target_page_index: Some(active_page),
            page_nav_mode: BoardPickerPageNavMode::Normal,
            page_search_query: String::new(),
            page_search_cursor: None,
            page_jump_buffer: String::new(),
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
        self.close_radial_menu();
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
        let active_page = self.boards.active_page_index();
        self.board_picker_state = BoardPickerState::Open {
            selected: active_index,
            hover_index: None,
            edit: None,
            mode: BoardPickerMode::Quick,
            focus: BoardPickerFocus::BoardList,
            page_focus_page_index: None,
            page_scroll_row: 0,
            page_scroll_target_page_index: Some(active_page),
            page_nav_mode: BoardPickerPageNavMode::Normal,
            page_search_query: String::new(),
            page_search_cursor: None,
            page_jump_buffer: String::new(),
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
        let active_page = if new_focus == BoardPickerFocus::PagePanel {
            self.board_picker_selected_board_active_page()
        } else {
            0
        };
        let BoardPickerState::Open {
            focus,
            page_focus_page_index,
            page_scroll_target_page_index,
            page_nav_mode,
            page_search_query,
            page_search_cursor,
            page_jump_buffer,
            ..
        } = &mut self.board_picker_state
        else {
            return;
        };
        *focus = new_focus;
        match new_focus {
            BoardPickerFocus::PagePanel => {
                if page_focus_page_index.is_none() {
                    *page_focus_page_index = Some(active_page);
                    *page_scroll_target_page_index = Some(active_page);
                }
            }
            BoardPickerFocus::BoardList => {
                *page_focus_page_index = None;
                *page_nav_mode = BoardPickerPageNavMode::Normal;
                page_search_query.clear();
                *page_search_cursor = None;
                page_jump_buffer.clear();
            }
        }
        self.needs_redraw = true;
        self.dirty_tracker.mark_full();
    }

    pub(crate) fn board_picker_page_focus_page_index(&self) -> Option<usize> {
        match &self.board_picker_state {
            BoardPickerState::Open {
                page_focus_page_index,
                ..
            } => *page_focus_page_index,
            BoardPickerState::Hidden => None,
        }
    }

    pub(crate) fn board_picker_set_page_focus_page_index(&mut self, index: usize) {
        let clamped = self.board_picker_clamp_page_index(index);
        if let BoardPickerState::Open {
            page_focus_page_index,
            page_scroll_target_page_index,
            ..
        } = &mut self.board_picker_state
        {
            *page_focus_page_index = Some(clamped);
            *page_scroll_target_page_index = Some(clamped);
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
        let previous_board = self.board_picker_page_panel_board_index();
        let next_board = self
            .board_picker_board_index_for_row(next)
            .unwrap_or_else(|| self.boards.active_index());
        let next_active_page = self
            .boards
            .board_states()
            .get(next_board)
            .map_or(0, |board| board.pages.active_index());
        if let BoardPickerState::Open {
            selected,
            focus,
            page_focus_page_index,
            page_scroll_row,
            page_scroll_target_page_index,
            page_nav_mode,
            page_search_query,
            page_search_cursor,
            page_jump_buffer,
            ..
        } = &mut self.board_picker_state
        {
            *selected = next;
            *focus = BoardPickerFocus::BoardList;
            *page_focus_page_index = None;
            *page_nav_mode = BoardPickerPageNavMode::Normal;
            page_search_query.clear();
            *page_search_cursor = None;
            page_jump_buffer.clear();
            if previous_board != Some(next_board) {
                *page_scroll_row = 0;
                *page_scroll_target_page_index = Some(next_active_page);
            }
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

    pub(crate) fn board_picker_page_panel_state_parts(
        &self,
    ) -> Option<(usize, Option<usize>, Option<usize>)> {
        match &self.board_picker_state {
            BoardPickerState::Open {
                page_scroll_row,
                page_focus_page_index,
                page_scroll_target_page_index,
                ..
            } => Some((
                *page_scroll_row,
                *page_focus_page_index,
                *page_scroll_target_page_index,
            )),
            BoardPickerState::Hidden => None,
        }
    }

    pub(crate) fn set_board_picker_page_panel_state_parts(
        &mut self,
        scroll_row: usize,
        focus_page_index: Option<usize>,
        scroll_target_page_index: Option<usize>,
    ) {
        if let BoardPickerState::Open {
            page_scroll_row,
            page_focus_page_index,
            page_scroll_target_page_index,
            ..
        } = &mut self.board_picker_state
        {
            *page_scroll_row = scroll_row;
            *page_focus_page_index = focus_page_index;
            *page_scroll_target_page_index = scroll_target_page_index;
        }
    }

    pub(crate) fn board_picker_queue_page_scroll_to(&mut self, page_index: usize) {
        let clamped = self.board_picker_clamp_page_index(page_index);
        if let BoardPickerState::Open {
            page_scroll_target_page_index,
            ..
        } = &mut self.board_picker_state
        {
            *page_scroll_target_page_index = Some(clamped);
            self.needs_redraw = true;
            self.dirty_tracker.mark_full();
        }
    }

    pub(crate) fn board_picker_scroll_page_panel_rows(&mut self, delta_rows: isize) -> bool {
        let Some(layout) = self.board_picker_layout else {
            return false;
        };
        if !layout.page_panel_enabled || delta_rows == 0 {
            return false;
        }
        let max = layout.page_max_scroll_row;
        let cols = layout.page_cols.max(1);
        let visible_slots = layout.page_visible_slots.max(1);
        let page_count = layout.page_count;
        let Some((scroll_row, _, _)) = self.board_picker_page_panel_state_parts() else {
            return false;
        };
        let current = scroll_row.min(max);
        let next = if delta_rows.is_negative() {
            current.saturating_sub(delta_rows.unsigned_abs())
        } else {
            current.saturating_add(delta_rows as usize).min(max)
        };
        if scroll_row == next {
            return false;
        }
        let first_visible = next.saturating_mul(cols).min(page_count);
        let last_visible = first_visible
            .saturating_add(visible_slots)
            .min(page_count)
            .saturating_sub(1);
        let visible_search_match = (self.board_picker_page_nav_mode()
            == BoardPickerPageNavMode::Search)
            .then(|| {
                self.board_picker_page_search_visible_match(
                    first_visible,
                    last_visible,
                    delta_rows.is_negative(),
                )
            })
            .flatten();
        if let BoardPickerState::Open {
            page_scroll_row,
            page_focus_page_index,
            page_scroll_target_page_index,
            page_nav_mode,
            page_search_cursor,
            ..
        } = &mut self.board_picker_state
        {
            *page_scroll_target_page_index = None;
            *page_scroll_row = next;
            if *page_nav_mode == BoardPickerPageNavMode::Search {
                if let Some((cursor, page_index)) = visible_search_match {
                    *page_search_cursor = Some(cursor);
                    *page_focus_page_index = Some(page_index);
                } else {
                    *page_search_cursor = None;
                    *page_focus_page_index = None;
                }
            } else if let Some(focus_page) = page_focus_page_index {
                if *focus_page < first_visible {
                    *page_focus_page_index = Some(first_visible.min(last_visible));
                } else if *focus_page > last_visible {
                    *page_focus_page_index = Some(last_visible);
                }
            }
            self.needs_redraw = true;
            self.dirty_tracker.mark_full();
            return true;
        }
        false
    }

    fn board_picker_selected_board_active_page(&self) -> usize {
        self.board_picker_page_panel_board_index_inner()
            .and_then(|index| self.boards.board_states().get(index))
            .map_or(0, |board| board.pages.active_index())
    }

    fn board_picker_clamp_page_index(&self, index: usize) -> usize {
        let page_count = self
            .board_picker_page_panel_board_index_inner()
            .and_then(|board_index| self.boards.board_states().get(board_index))
            .map_or(0, |board| board.pages.page_count());
        if page_count == 0 {
            0
        } else {
            index.min(page_count.saturating_sub(1))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::input::state::test_support::make_test_input_state;

    #[test]
    fn board_picker_openers_close_radial_menu() {
        let mut state = make_test_input_state();

        state.open_radial_menu(320.0, 240.0);
        state.open_board_picker();
        assert!(state.is_board_picker_open());
        assert!(!state.is_radial_menu_open());

        state.close_board_picker();
        state.open_radial_menu(320.0, 240.0);
        state.open_board_picker_quick();
        assert!(state.is_board_picker_open());
        assert!(state.board_picker_is_quick());
        assert!(!state.is_radial_menu_open());

        state.open_radial_menu(320.0, 240.0);
        assert!(state.is_radial_menu_open());
        assert!(!state.is_board_picker_open());
    }
}
