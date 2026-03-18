use std::time::Instant;

use super::super::base::InputState;
use super::{
    BOARD_PICKER_SEARCH_MAX_LEN, BOARD_PICKER_SEARCH_TIMEOUT, BoardPickerFocus, BoardPickerState,
};

impl InputState {
    pub(crate) fn board_picker_clear_search(&mut self) -> bool {
        if self.board_picker_search.is_empty() {
            return false;
        }
        self.board_picker_search.clear();
        self.board_picker_search_last_input = None;
        self.needs_redraw = true;
        true
    }

    pub(crate) fn board_picker_backspace_search(&mut self) -> bool {
        if self.board_picker_search.is_empty() {
            return false;
        }
        self.board_picker_search.pop();
        if self.board_picker_search.is_empty() {
            self.board_picker_search_last_input = None;
        } else {
            self.board_picker_search_last_input = Some(Instant::now());
        }
        self.board_picker_select_search_match();
        self.needs_redraw = true;
        true
    }

    pub(crate) fn board_picker_append_search(&mut self, ch: char) -> bool {
        self.board_picker_reset_search_if_stale();
        if self.board_picker_search.len() >= BOARD_PICKER_SEARCH_MAX_LEN {
            return false;
        }
        self.board_picker_search.push(ch);
        self.board_picker_search_last_input = Some(Instant::now());
        // Typing always returns focus to the board list
        if let BoardPickerState::Open {
            focus,
            page_focus_index,
            ..
        } = &mut self.board_picker_state
        {
            *focus = BoardPickerFocus::BoardList;
            *page_focus_index = None;
        }
        self.board_picker_select_search_match();
        self.needs_redraw = true;
        true
    }

    fn board_picker_reset_search_if_stale(&mut self) {
        let Some(last_input) = self.board_picker_search_last_input else {
            return;
        };
        if last_input.elapsed() > BOARD_PICKER_SEARCH_TIMEOUT {
            self.board_picker_search.clear();
            self.board_picker_search_last_input = None;
            self.needs_redraw = true;
        }
    }

    fn board_picker_select_search_match(&mut self) {
        let Some(index) = self.board_picker_match_index(self.board_picker_search.trim()) else {
            return;
        };
        self.board_picker_set_selected(index);
    }

    fn board_picker_match_index(&self, query: &str) -> Option<usize> {
        let query = query.trim();
        if query.is_empty() {
            return None;
        }
        let board_count = self.boards.board_count();
        let lower = query.to_ascii_lowercase();
        match lower.parse::<usize>() {
            Ok(value) if (1..=board_count).contains(&value) => {
                return self.board_picker_row_for_board(value - 1);
            }
            _ => {}
        }
        let mut best: Option<(usize, i32)> = None;
        for (idx, board) in self.boards.board_states().iter().enumerate() {
            let name = board.spec.name.to_ascii_lowercase();
            let id = board.spec.id.to_ascii_lowercase();
            let score = match (
                fuzzy_score_relaxed(&lower, &name),
                fuzzy_score_relaxed(&lower, &id),
            ) {
                (Some(a), Some(b)) => a.max(b),
                (Some(a), None) => a,
                (None, Some(b)) => b,
                (None, None) => continue,
            };
            let mut score = score;
            if name.starts_with(&lower) || id.starts_with(&lower) {
                score += 1000;
            } else if name.contains(&lower) || id.contains(&lower) {
                score += 500;
            }
            if best
                .map(|(_, best_score)| score > best_score)
                .unwrap_or(true)
            {
                best = Some((idx, score));
            }
        }
        best.and_then(|(idx, _)| self.board_picker_row_for_board(idx))
    }
}

fn fuzzy_score(needle: &str, haystack: &str) -> Option<i32> {
    if needle.is_empty() {
        return None;
    }
    let mut score = 0i32;
    let mut last_match: Option<usize> = None;
    let mut hay_idx = 0usize;
    let hay_chars: Vec<char> = haystack.chars().collect();
    for n in needle.chars() {
        let mut found = None;
        for (i, ch) in hay_chars.iter().enumerate().skip(hay_idx) {
            if *ch == n {
                found = Some(i);
                break;
            }
        }
        let idx = found?;
        if let Some(prev) = last_match {
            if idx == prev + 1 {
                score += 15;
            } else {
                score += 8;
            }
        } else {
            score += 10;
        }
        if idx == 0 {
            score += 20;
        } else if matches!(
            hay_chars.get(idx.saturating_sub(1)),
            Some(' ' | '-' | '_' | '/')
        ) {
            score += 12;
        }
        last_match = Some(idx);
        hay_idx = idx + 1;
    }
    score -= hay_chars.len() as i32;
    Some(score)
}

fn fuzzy_score_relaxed(needle: &str, haystack: &str) -> Option<i32> {
    if let Some(score) = fuzzy_score(needle, haystack) {
        return Some(score);
    }
    fuzzy_score_with_single_swap(needle, haystack)
}

fn fuzzy_score_with_single_swap(needle: &str, haystack: &str) -> Option<i32> {
    let mut chars: Vec<char> = needle.chars().collect();
    if chars.len() < 2 {
        return None;
    }
    let mut best: Option<i32> = None;
    for i in 0..chars.len().saturating_sub(1) {
        chars.swap(i, i + 1);
        let swapped: String = chars.iter().collect();
        if let Some(score) = fuzzy_score(&swapped, haystack) {
            let score = score - 25;
            if best.map(|best| score > best).unwrap_or(true) {
                best = Some(score);
            }
        }
        chars.swap(i, i + 1);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::{
        BOARD_PICKER_SEARCH_TIMEOUT, BoardPickerFocus, fuzzy_score, fuzzy_score_relaxed,
    };
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode, InputState};
    use std::time::{Duration, Instant};

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");
        let action_bindings = keybindings
            .build_action_bindings()
            .expect("default keybindings bindings");

        let mut state = InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            4.0,
            4.0,
            EraserMode::Brush,
            0.32,
            false,
            32.0,
            FontDescriptor::default(),
            false,
            20.0,
            30.0,
            false,
            true,
            BoardsConfig::default(),
            action_map,
            usize::MAX,
            ClickHighlightSettings::disabled(),
            0,
            0,
            true,
            0,
            0,
            5,
            5,
            PresenterModeConfig::default(),
        );
        state.set_action_bindings(action_bindings);
        state
    }

    #[test]
    fn fuzzy_score_relaxed_handles_transpose() {
        assert!(fuzzy_score("balckboard", "blackboard").is_none());
        assert!(fuzzy_score_relaxed("balckboard", "blackboard").is_some());
    }

    #[test]
    fn fuzzy_score_relaxed_rejects_unrelated() {
        assert!(fuzzy_score_relaxed("zz", "blackboard").is_none());
    }

    #[test]
    fn board_picker_match_index_accepts_numeric_board_selection() {
        let mut state = make_state();
        state.open_board_picker();

        assert_eq!(state.board_picker_match_index("3"), state.board_picker_row_for_board(2));
    }

    #[test]
    fn board_picker_match_index_trims_surrounding_whitespace() {
        let mut state = make_state();
        state.open_board_picker();
        let blueprint_index = state
            .boards
            .board_states()
            .iter()
            .position(|board| board.spec.id == "blueprint")
            .expect("blueprint board");

        assert_eq!(
            state.board_picker_match_index("  blue  "),
            state.board_picker_row_for_board(blueprint_index)
        );
    }

    #[test]
    fn board_picker_match_index_rejects_whitespace_only_queries() {
        let mut state = make_state();
        state.open_board_picker();

        assert_eq!(state.board_picker_match_index("   \t  "), None);
    }

    #[test]
    fn board_picker_append_search_returns_focus_to_board_list() {
        let mut state = make_state();
        state.open_board_picker();
        state.board_picker_set_focus(BoardPickerFocus::PagePanel);
        assert_eq!(state.board_picker_focus(), BoardPickerFocus::PagePanel);

        state.board_picker_append_search('b');

        assert_eq!(state.board_picker_focus(), BoardPickerFocus::BoardList);
        assert_eq!(state.board_picker_page_focus_index(), None);
    }

    #[test]
    fn board_picker_append_search_resets_stale_query_before_appending() {
        let mut state = make_state();
        state.open_board_picker();
        state.board_picker_search = "old".to_string();
        state.board_picker_search_last_input = Some(
            Instant::now() - BOARD_PICKER_SEARCH_TIMEOUT - Duration::from_millis(1),
        );

        state.board_picker_append_search('b');

        assert_eq!(state.board_picker_search, "b");
        assert!(state.board_picker_search_last_input.is_some());
    }

    #[test]
    fn board_picker_clear_search_reports_when_work_was_done() {
        let mut state = make_state();
        state.board_picker_search = "blue".to_string();
        state.board_picker_search_last_input = Some(Instant::now());
        state.needs_redraw = false;

        assert!(state.board_picker_clear_search());
        assert!(state.board_picker_search.is_empty());
        assert!(state.board_picker_search_last_input.is_none());
        assert!(state.needs_redraw);
        assert!(!state.board_picker_clear_search());
    }

    #[test]
    fn board_picker_backspace_search_clears_timestamp_when_last_character_removed() {
        let mut state = make_state();
        state.board_picker_search = "b".to_string();
        state.board_picker_search_last_input = Some(Instant::now());

        assert!(state.board_picker_backspace_search());
        assert!(state.board_picker_search.is_empty());
        assert!(state.board_picker_search_last_input.is_none());
    }

    #[test]
    fn board_picker_append_search_rejects_input_past_max_len() {
        let mut state = make_state();
        state.board_picker_search = "x".repeat(super::BOARD_PICKER_SEARCH_MAX_LEN);

        assert!(!state.board_picker_append_search('y'));
        assert_eq!(
            state.board_picker_search.len(),
            super::BOARD_PICKER_SEARCH_MAX_LEN
        );
    }

    #[test]
    fn fuzzy_score_prefers_prefix_matches_over_internal_matches() {
        assert!(fuzzy_score("blue", "blueprint") > fuzzy_score("blue", "my blueprint"));
    }

    #[test]
    fn fuzzy_score_relaxed_penalizes_transposed_matches_vs_exact_matches() {
        let exact = fuzzy_score_relaxed("blackboard", "blackboard").unwrap();
        let swapped = fuzzy_score_relaxed("balckboard", "blackboard").unwrap();
        assert!(exact > swapped);
    }
}
