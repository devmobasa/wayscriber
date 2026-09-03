use std::time::Instant;

use super::{
    BOARD_PICKER_SEARCH_MAX_LEN, BOARD_PICKER_SEARCH_TIMEOUT, BoardPickerDrag, BoardPickerFocus,
    BoardPickerLayout, BoardPickerMode, BoardPickerPageDrag, BoardPickerPageEdit,
    BoardPickerPageNavMode, BoardPickerState,
};
use crate::input::state::core::base::BoardPickerClickState;

/// Modal, layout, search, edit, and drag state for the board picker.
#[derive(Debug)]
pub struct BoardPickerPanel {
    pub(in crate::input::state) state: BoardPickerState,
    pub(in crate::input::state) drag: Option<BoardPickerDrag>,
    pub(in crate::input::state) page_drag: Option<BoardPickerPageDrag>,
    pub(in crate::input::state) page_edit: Option<BoardPickerPageEdit>,
    pub(in crate::input::state) layout: Option<BoardPickerLayout>,
    pub(in crate::input::state) search: String,
    pub(in crate::input::state) search_last_input: Option<Instant>,
    pub(in crate::input::state) last_click: Option<BoardPickerClickState>,
}

impl BoardPickerPanel {
    pub fn page_drag(&self) -> Option<BoardPickerPageDrag> {
        self.page_drag
    }

    pub(crate) fn is_open(&self) -> bool {
        matches!(self.state, BoardPickerState::Open { .. })
    }

    pub(crate) fn mode(&self) -> BoardPickerMode {
        match self.state {
            BoardPickerState::Open { mode, .. } => mode,
            BoardPickerState::Hidden => BoardPickerMode::Full,
        }
    }

    pub(crate) fn open(
        &mut self,
        mode: BoardPickerMode,
        active_index: usize,
        active_page: usize,
        selected_row: Option<usize>,
    ) {
        self.clear_search();
        self.drag = None;
        self.page_drag = None;
        self.page_edit = None;
        self.state = BoardPickerState::Open {
            selected: selected_row.unwrap_or(active_index),
            hover_index: None,
            edit: None,
            mode,
            focus: BoardPickerFocus::BoardList,
            page_focus_page_index: None,
            page_scroll_row: 0,
            page_scroll_target_page_index: Some(active_page),
            page_nav_mode: BoardPickerPageNavMode::Normal,
            page_search_query: String::new(),
            page_search_cursor: None,
            page_jump_buffer: String::new(),
        };
    }

    pub(crate) fn close(&mut self) -> Option<BoardPickerLayout> {
        let layout = self.layout.take();
        self.state = BoardPickerState::Hidden;
        self.drag = None;
        self.page_drag = None;
        self.page_edit = None;
        self.last_click = None;
        self.clear_search();
        layout
    }

    pub(crate) fn active_index(&self) -> Option<usize> {
        match self.state {
            BoardPickerState::Open {
                hover_index,
                selected,
                ..
            } => hover_index.or(Some(selected)),
            BoardPickerState::Hidden => None,
        }
    }

    pub(crate) fn selected_index(&self) -> Option<usize> {
        match self.state {
            BoardPickerState::Open { selected, .. } => Some(selected),
            BoardPickerState::Hidden => None,
        }
    }

    pub(crate) fn focus(&self) -> BoardPickerFocus {
        match self.state {
            BoardPickerState::Open { focus, .. } => focus,
            BoardPickerState::Hidden => BoardPickerFocus::BoardList,
        }
    }

    pub(crate) fn set_focus(&mut self, new_focus: BoardPickerFocus, active_page: usize) -> bool {
        if self.focus() == new_focus {
            return false;
        }
        let BoardPickerState::Open {
            focus,
            page_focus_page_index,
            page_scroll_target_page_index,
            page_nav_mode,
            page_search_query,
            page_search_cursor,
            page_jump_buffer,
            ..
        } = &mut self.state
        else {
            return false;
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
        true
    }

    pub(crate) fn clear_search(&mut self) -> bool {
        if self.search.is_empty() {
            return false;
        }
        self.search.clear();
        self.search_last_input = None;
        true
    }

    pub(crate) fn backspace_search(&mut self, now: Instant) -> bool {
        if self.search.pop().is_none() {
            return false;
        }
        self.search_last_input = (!self.search.is_empty()).then_some(now);
        true
    }

    pub(crate) fn append_search(&mut self, ch: char, now: Instant) -> bool {
        if self
            .search_last_input
            .is_some_and(|last| now.saturating_duration_since(last) > BOARD_PICKER_SEARCH_TIMEOUT)
        {
            self.search.clear();
            self.search_last_input = None;
        }
        if self.search.len() >= BOARD_PICKER_SEARCH_MAX_LEN {
            return false;
        }
        self.search.push(ch);
        self.search_last_input = Some(now);
        if let BoardPickerState::Open {
            focus,
            page_focus_page_index,
            ..
        } = &mut self.state
        {
            *focus = BoardPickerFocus::BoardList;
            *page_focus_page_index = None;
        }
        true
    }
}

impl Default for BoardPickerPanel {
    fn default() -> Self {
        Self {
            state: BoardPickerState::Hidden,
            drag: None,
            page_drag: None,
            page_edit: None,
            layout: None,
            search: String::new(),
            search_last_input: None,
            last_click: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn open_and_close_own_the_panel_lifecycle() {
        let mut panel = BoardPickerPanel::default();
        panel.open(BoardPickerMode::Quick, 3, 2, Some(1));

        assert!(panel.is_open());
        assert_eq!(panel.mode(), BoardPickerMode::Quick);
        assert_eq!(panel.selected_index(), Some(1));

        assert!(panel.close().is_none());
        assert!(!panel.is_open());
        assert_eq!(panel.selected_index(), None);
    }

    #[test]
    fn search_input_expires_stale_text_and_returns_focus_to_boards() {
        let now = Instant::now();
        let mut panel = BoardPickerPanel::default();
        panel.open(BoardPickerMode::Full, 0, 0, None);
        assert!(panel.set_focus(BoardPickerFocus::PagePanel, 0));
        panel.search = "old".to_string();
        panel.search_last_input =
            Some(now - BOARD_PICKER_SEARCH_TIMEOUT - Duration::from_millis(1));

        assert!(panel.append_search('b', now));

        assert_eq!(panel.search, "b");
        assert_eq!(panel.focus(), BoardPickerFocus::BoardList);
        assert_eq!(panel.search_last_input, Some(now));
    }

    #[test]
    fn backspace_clears_the_search_timestamp_with_the_last_character() {
        let now = Instant::now();
        let mut panel = BoardPickerPanel {
            search: "b".to_string(),
            search_last_input: Some(now),
            ..Default::default()
        };

        assert!(panel.backspace_search(now + Duration::from_millis(1)));
        assert_eq!(panel.search, "");
        assert_eq!(panel.search_last_input, None);
        assert!(!panel.backspace_search(now));
    }
}
