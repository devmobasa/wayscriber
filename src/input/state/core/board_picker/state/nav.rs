use crate::input::events::Key;
use crate::input::state::{Toast, ToastPriority};

use super::super::super::base::InputState;
use super::super::{
    BOARD_PICKER_PAGE_JUMP_MAX_LEN, BOARD_PICKER_PAGE_SEARCH_MAX_LEN, BoardPickerPageNavMode,
    BoardPickerState,
};

impl InputState {
    pub(crate) fn board_picker_page_nav_mode(&self) -> BoardPickerPageNavMode {
        match &self.board_picker_state {
            BoardPickerState::Open { page_nav_mode, .. } => *page_nav_mode,
            BoardPickerState::Hidden => BoardPickerPageNavMode::Normal,
        }
    }

    pub(crate) fn board_picker_page_jump_buffer(&self) -> Option<&str> {
        match &self.board_picker_state {
            BoardPickerState::Open {
                page_nav_mode,
                page_jump_buffer,
                ..
            } if *page_nav_mode == BoardPickerPageNavMode::Jump => Some(page_jump_buffer.as_str()),
            _ => None,
        }
    }

    pub(crate) fn board_picker_page_search_query(&self) -> Option<&str> {
        match &self.board_picker_state {
            BoardPickerState::Open {
                page_nav_mode,
                page_search_query,
                ..
            } if *page_nav_mode == BoardPickerPageNavMode::Search => {
                Some(page_search_query.as_str())
            }
            _ => None,
        }
    }

    pub(crate) fn board_picker_page_search_cursor(&self) -> Option<usize> {
        match &self.board_picker_state {
            BoardPickerState::Open {
                page_nav_mode,
                page_search_cursor,
                ..
            } if *page_nav_mode == BoardPickerPageNavMode::Search => *page_search_cursor,
            _ => None,
        }
    }

    pub(crate) fn board_picker_begin_page_jump(&mut self) {
        self.board_picker_clear_search();
        if let BoardPickerState::Open {
            page_nav_mode,
            page_search_query,
            page_search_cursor,
            page_jump_buffer,
            ..
        } = &mut self.board_picker_state
        {
            *page_nav_mode = BoardPickerPageNavMode::Jump;
            page_search_query.clear();
            *page_search_cursor = None;
            page_jump_buffer.clear();
            self.needs_redraw = true;
            self.dirty_tracker.mark_full();
        }
    }

    pub(crate) fn board_picker_begin_page_search(&mut self) {
        self.board_picker_clear_search();
        if let BoardPickerState::Open {
            page_nav_mode,
            page_search_query,
            page_search_cursor,
            page_jump_buffer,
            ..
        } = &mut self.board_picker_state
        {
            *page_nav_mode = BoardPickerPageNavMode::Search;
            page_search_query.clear();
            *page_search_cursor = None;
            page_jump_buffer.clear();
            self.needs_redraw = true;
            self.dirty_tracker.mark_full();
        }
    }

    pub(crate) fn board_picker_clear_page_nav(&mut self) -> bool {
        let BoardPickerState::Open {
            page_nav_mode,
            page_search_query,
            page_search_cursor,
            page_jump_buffer,
            ..
        } = &mut self.board_picker_state
        else {
            return false;
        };
        let changed = *page_nav_mode != BoardPickerPageNavMode::Normal
            || !page_search_query.is_empty()
            || page_search_cursor.is_some()
            || !page_jump_buffer.is_empty();
        if changed {
            *page_nav_mode = BoardPickerPageNavMode::Normal;
            page_search_query.clear();
            *page_search_cursor = None;
            page_jump_buffer.clear();
            self.needs_redraw = true;
            self.dirty_tracker.mark_full();
        }
        changed
    }

    pub(crate) fn handle_board_picker_page_nav_key(&mut self, key: Key) -> Option<bool> {
        match self.board_picker_page_nav_mode() {
            BoardPickerPageNavMode::Normal => None,
            BoardPickerPageNavMode::Jump => Some(self.handle_board_picker_page_jump_key(key)),
            BoardPickerPageNavMode::Search => Some(self.handle_board_picker_page_search_key(key)),
        }
    }

    fn handle_board_picker_page_jump_key(&mut self, key: Key) -> bool {
        match key {
            Key::Escape => {
                self.board_picker_clear_page_nav();
                true
            }
            Key::Return => {
                self.board_picker_commit_page_jump();
                true
            }
            Key::Backspace | Key::Delete => {
                if let BoardPickerState::Open {
                    page_jump_buffer, ..
                } = &mut self.board_picker_state
                {
                    page_jump_buffer.pop();
                    self.needs_redraw = true;
                }
                true
            }
            Key::Char(ch) if ch.is_ascii_digit() => {
                if let BoardPickerState::Open {
                    page_jump_buffer, ..
                } = &mut self.board_picker_state
                    && page_jump_buffer.len() < BOARD_PICKER_PAGE_JUMP_MAX_LEN
                {
                    page_jump_buffer.push(ch);
                    self.needs_redraw = true;
                }
                true
            }
            _ => true,
        }
    }

    fn board_picker_commit_page_jump(&mut self) {
        let buffer = match self.board_picker_page_jump_buffer() {
            Some(buffer) => buffer.trim().to_string(),
            None => return,
        };
        if buffer.is_empty() {
            return;
        }
        let Ok(page_number) = buffer.parse::<usize>() else {
            return;
        };
        let page_count = self.board_picker_selected_board_page_count();
        if page_number == 0 || page_number > page_count {
            self.push_toast(
                ToastPriority::Info,
                "board_picker",
                Toast::warning("Page number out of range."),
            );
            self.needs_redraw = true;
            return;
        }
        let page_index = page_number - 1;
        self.board_picker_clear_page_nav();
        self.board_picker_set_page_focus_page_index(page_index);
    }

    fn handle_board_picker_page_search_key(&mut self, key: Key) -> bool {
        match key {
            Key::Escape => {
                self.board_picker_clear_page_nav();
                true
            }
            Key::Return => {
                if let Some(page_index) = self.board_picker_page_search_active_match() {
                    self.board_picker_activate_page(page_index);
                }
                true
            }
            Key::Backspace | Key::Delete => {
                if let BoardPickerState::Open {
                    page_search_query, ..
                } = &mut self.board_picker_state
                {
                    page_search_query.pop();
                }
                self.board_picker_reconcile_page_search_after_query_change();
                true
            }
            Key::F3 => {
                self.board_picker_cycle_page_search_match(self.modifiers.shift);
                true
            }
            Key::Space if !self.modifiers.ctrl && !self.modifiers.alt => {
                self.board_picker_append_page_search_char(' ');
                true
            }
            Key::Char(ch) if !self.modifiers.ctrl && !self.modifiers.alt && !ch.is_control() => {
                self.board_picker_append_page_search_char(ch);
                true
            }
            _ => true,
        }
    }

    fn board_picker_append_page_search_char(&mut self, ch: char) {
        if let BoardPickerState::Open {
            page_search_query, ..
        } = &mut self.board_picker_state
        {
            if page_search_query.len() >= BOARD_PICKER_PAGE_SEARCH_MAX_LEN {
                return;
            }
            page_search_query.push(ch);
        }
        self.board_picker_reconcile_page_search_after_query_change();
    }

    fn board_picker_reconcile_page_search_after_query_change(&mut self) {
        let matches = self.board_picker_page_search_match_indexes();
        if let BoardPickerState::Open {
            page_search_cursor, ..
        } = &mut self.board_picker_state
        {
            *page_search_cursor = if matches.is_empty() { None } else { Some(0) };
        }
        if let Some(page_index) = matches.first().copied() {
            self.board_picker_set_page_focus_page_index(page_index);
        } else {
            self.needs_redraw = true;
            self.dirty_tracker.mark_full();
        }
    }

    pub(crate) fn board_picker_reconcile_page_nav_after_page_change(&mut self) {
        if self.board_picker_page_nav_mode() != BoardPickerPageNavMode::Search {
            return;
        }
        let matches = self.board_picker_page_search_match_indexes();
        let page_count = self.board_picker_selected_board_page_count();
        let current_focus = self.board_picker_page_focus_page_index();
        let mut focus_page = None;

        if let BoardPickerState::Open {
            page_search_cursor, ..
        } = &mut self.board_picker_state
        {
            if matches.is_empty() {
                *page_search_cursor = None;
                if page_count > 0
                    && let Some(current_focus) = current_focus
                    && current_focus >= page_count
                {
                    focus_page = Some(page_count.saturating_sub(1));
                }
            } else {
                let next_cursor = page_search_cursor
                    .unwrap_or(0)
                    .min(matches.len().saturating_sub(1));
                *page_search_cursor = Some(next_cursor);
                focus_page = matches.get(next_cursor).copied();
            }
        }

        if let Some(page_index) = focus_page {
            self.board_picker_set_page_focus_page_index(page_index);
        } else {
            self.needs_redraw = true;
            self.dirty_tracker.mark_full();
        }
    }

    fn board_picker_cycle_page_search_match(&mut self, reverse: bool) {
        let matches = self.board_picker_page_search_match_indexes();
        if matches.is_empty() {
            if let BoardPickerState::Open {
                page_search_cursor, ..
            } = &mut self.board_picker_state
            {
                *page_search_cursor = None;
            }
            self.needs_redraw = true;
            return;
        }
        let current = self
            .board_picker_page_search_cursor()
            .map(|cursor| cursor.min(matches.len().saturating_sub(1)));
        let next = match (current, reverse) {
            (Some(current), true) => current
                .checked_sub(1)
                .unwrap_or_else(|| matches.len().saturating_sub(1)),
            (Some(current), false) => (current + 1) % matches.len(),
            (None, true) => matches.len().saturating_sub(1),
            (None, false) => 0,
        };
        if let BoardPickerState::Open {
            page_search_cursor, ..
        } = &mut self.board_picker_state
        {
            *page_search_cursor = Some(next);
        }
        if let Some(page_index) = matches.get(next).copied() {
            self.board_picker_set_page_focus_page_index(page_index);
        }
    }

    pub(crate) fn board_picker_page_search_active_match(&self) -> Option<usize> {
        let cursor = self.board_picker_page_search_cursor()?;
        let matches = self.board_picker_page_search_match_indexes();
        matches
            .get(cursor.min(matches.len().saturating_sub(1)))
            .copied()
    }

    pub(crate) fn board_picker_page_search_visible_match(
        &self,
        first_visible: usize,
        last_visible: usize,
        prefer_last: bool,
    ) -> Option<(usize, usize)> {
        let matches = self.board_picker_page_search_match_indexes();
        if matches.is_empty() {
            return None;
        }
        let current = self
            .board_picker_page_search_cursor()
            .unwrap_or(0)
            .min(matches.len().saturating_sub(1));
        if let Some(page_index) = matches.get(current).copied()
            && page_index >= first_visible
            && page_index <= last_visible
        {
            return Some((current, page_index));
        }
        let mut visible_matches = matches
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, page_index)| *page_index >= first_visible && *page_index <= last_visible);
        if prefer_last {
            visible_matches.next_back()
        } else {
            visible_matches.next()
        }
    }

    pub(crate) fn board_picker_page_search_match_count(&self) -> usize {
        self.board_picker_page_search_match_indexes().len()
    }

    pub(crate) fn board_picker_page_search_match_indexes(&self) -> Vec<usize> {
        let Some(query) = self.board_picker_page_search_query() else {
            return Vec::new();
        };
        let Some(board_index) = self.board_picker_page_panel_board_index() else {
            return Vec::new();
        };
        let Some(board) = self.boards.board_states().get(board_index) else {
            return Vec::new();
        };
        let page_count = board.pages.page_count();
        let normalized = query.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return Vec::new();
        }
        if let Some(page_number) = exact_page_number_query(&normalized) {
            return page_number
                .checked_sub(1)
                .filter(|index| *index < page_count)
                .into_iter()
                .collect();
        }
        board
            .pages
            .pages()
            .iter()
            .enumerate()
            .filter_map(|(index, page)| {
                page.page_name()
                    .is_some_and(|name| name.to_ascii_lowercase().contains(&normalized))
                    .then_some(index)
            })
            .collect()
    }

    pub(crate) fn board_picker_page_matches_current_search(&self, page_index: usize) -> bool {
        let Some(query) = self.board_picker_page_search_query() else {
            return false;
        };
        let normalized = query.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return false;
        }
        if let Some(page_number) = exact_page_number_query(&normalized) {
            return page_number.checked_sub(1) == Some(page_index);
        }
        let Some(board_index) = self.board_picker_page_panel_board_index() else {
            return false;
        };
        self.boards
            .board_states()
            .get(board_index)
            .and_then(|board| board.pages.pages().get(page_index))
            .and_then(|page| page.page_name())
            .is_some_and(|name| name.to_ascii_lowercase().contains(&normalized))
    }

    fn board_picker_selected_board_page_count(&self) -> usize {
        self.board_picker_page_panel_board_index()
            .and_then(|board_index| self.boards.board_states().get(board_index))
            .map_or(0, |board| board.pages.page_count())
    }
}

fn exact_page_number_query(normalized: &str) -> Option<usize> {
    let digits = if normalized.chars().all(|ch| ch.is_ascii_digit()) {
        normalized
    } else {
        normalized.strip_prefix("page ")?.trim()
    };
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    digits.parse::<usize>().ok()
}
