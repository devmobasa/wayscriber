use super::super::base::InputState;

/// Upper bound for page navigation. The actual page count is calculated
/// dynamically by the render state. Navigation clamps to the actual count.
const HELP_OVERLAY_MAX_PAGES: usize = 10;

/// Cursor hint for the help overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpOverlayCursorHint {
    /// Default arrow cursor.
    Default,
    /// Text editing cursor (I-beam) for search input.
    Text,
}

impl InputState {
    fn open_help_overlay_internal(&mut self, quick_mode: bool, track_usage: bool) {
        self.show_help = true;
        self.help_overlay_quick_mode = quick_mode;
        self.help_overlay_scroll = 0.0;
        self.help_overlay_scroll_max = 0.0;
        if track_usage {
            self.pending_onboarding_usage.used_help_overlay = true;
        }
        self.help_overlay_page = 0;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn toggle_help_overlay(&mut self) {
        if self.show_help {
            self.show_help = false;
            self.help_overlay_quick_mode = false;
            self.help_overlay_scroll = 0.0;
            self.help_overlay_scroll_max = 0.0;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return;
        }
        self.open_help_overlay_internal(false, true);
    }

    pub(crate) fn toggle_quick_help(&mut self) {
        if self.show_help && self.help_overlay_quick_mode {
            self.show_help = false;
            self.help_overlay_quick_mode = false;
            self.help_overlay_scroll = 0.0;
            self.help_overlay_scroll_max = 0.0;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return;
        }
        self.open_help_overlay_internal(true, true);
    }

    pub(crate) fn help_overlay_next_page(&mut self) -> bool {
        // Use upper bound; render state clamps to actual page count
        let next_page = self.help_overlay_page + 1;
        if next_page < HELP_OVERLAY_MAX_PAGES {
            self.help_overlay_page = next_page;
            self.help_overlay_scroll = 0.0;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return true;
        }
        false
    }

    pub(crate) fn help_overlay_prev_page(&mut self) -> bool {
        if self.help_overlay_page > 0 {
            self.help_overlay_page -= 1;
            self.help_overlay_scroll = 0.0;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            true
        } else {
            false
        }
    }

    /// Clear help search and reset cursor position.
    #[allow(dead_code)]
    pub(crate) fn clear_help_search(&mut self) {
        self.help_overlay_search.clear();
        self.help_overlay_search_cursor = 0;
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Move help search cursor left.
    #[allow(dead_code)]
    pub(crate) fn help_search_cursor_left(&mut self) {
        if self.help_overlay_search_cursor > 0 {
            // Move back by one character (handle UTF-8 properly)
            let text = &self.help_overlay_search;
            if let Some((idx, _)) = text
                .char_indices()
                .take(self.help_overlay_search_cursor)
                .last()
            {
                self.help_overlay_search_cursor = text[..idx].chars().count();
            }
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Move help search cursor right.
    #[allow(dead_code)]
    pub(crate) fn help_search_cursor_right(&mut self) {
        let char_count = self.help_overlay_search.chars().count();
        if self.help_overlay_search_cursor < char_count {
            self.help_overlay_search_cursor += 1;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Insert text at cursor position.
    #[allow(dead_code)]
    pub(crate) fn help_search_insert(&mut self, text: &str) {
        let cursor = self.help_overlay_search_cursor;
        let current = &self.help_overlay_search;
        let byte_idx = current
            .char_indices()
            .nth(cursor)
            .map(|(i, _)| i)
            .unwrap_or(current.len());
        self.help_overlay_search.insert_str(byte_idx, text);
        self.help_overlay_search_cursor += text.chars().count();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Delete character before cursor (backspace).
    #[allow(dead_code)]
    pub(crate) fn help_search_backspace(&mut self) {
        if self.help_overlay_search_cursor > 0 {
            let current = &self.help_overlay_search;
            let cursor = self.help_overlay_search_cursor;
            // Find byte index of previous character
            let char_indices: Vec<_> = current.char_indices().collect();
            if cursor <= char_indices.len() {
                let _start_idx = if cursor >= 2 {
                    char_indices[cursor - 2].0 + char_indices[cursor - 2].1.len_utf8()
                } else {
                    0
                };
                let end_idx = if cursor - 1 < char_indices.len() {
                    char_indices[cursor - 1].0 + char_indices[cursor - 1].1.len_utf8()
                } else {
                    current.len()
                };
                self.help_overlay_search
                    .replace_range(char_indices[cursor - 1].0..end_idx, "");
                self.help_overlay_search_cursor -= 1;
            }
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Determine the cursor type for the help overlay.
    /// Returns `None` if the help overlay is not open.
    /// The help overlay search accepts keyboard input, so we show Text cursor
    /// in the top navigation/search area.
    pub fn help_overlay_cursor_hint_at(
        &self,
        x: i32,
        y: i32,
        screen_width: u32,
        screen_height: u32,
    ) -> Option<HelpOverlayCursorHint> {
        if !self.show_help {
            return None;
        }

        // Calculate approximate overlay bounds (centered, ~80% of screen)
        let margin_x = screen_width as f64 * 0.1;
        let margin_y = screen_height as f64 * 0.05;
        let box_x = margin_x;
        let box_y = margin_y;
        let box_width = screen_width as f64 - margin_x * 2.0;
        let box_height = screen_height as f64 - margin_y * 2.0;

        let local_x = x as f64 - box_x;
        let local_y = y as f64 - box_y;

        // Check if outside overlay bounds
        if local_x < 0.0 || local_x > box_width || local_y < 0.0 || local_y > box_height {
            return None;
        }

        // The search box is in the top ~80px of the overlay (nav area)
        // Show text cursor there since typing goes to search
        let nav_height = 80.0;
        if local_y <= nav_height {
            return Some(HelpOverlayCursorHint::Text);
        }

        Some(HelpOverlayCursorHint::Default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode};

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        InputState::with_defaults(
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
        )
    }

    #[test]
    fn toggle_help_overlay_opens_and_tracks_usage() {
        let mut state = make_state();
        state.toggle_help_overlay();

        assert!(state.show_help);
        assert!(!state.help_overlay_quick_mode);
        assert!(state.pending_onboarding_usage.used_help_overlay);
        assert_eq!(state.help_overlay_page, 0);
    }

    #[test]
    fn toggle_quick_help_closes_when_already_in_quick_mode() {
        let mut state = make_state();
        state.toggle_quick_help();
        assert!(state.show_help);
        assert!(state.help_overlay_quick_mode);

        state.toggle_quick_help();
        assert!(!state.show_help);
        assert!(!state.help_overlay_quick_mode);
    }

    #[test]
    fn help_overlay_page_navigation_resets_scroll_and_respects_bounds() {
        let mut state = make_state();
        state.help_overlay_scroll = 123.0;
        assert!(state.help_overlay_next_page());
        assert_eq!(state.help_overlay_page, 1);
        assert_eq!(state.help_overlay_scroll, 0.0);

        state.help_overlay_page = HELP_OVERLAY_MAX_PAGES - 1;
        assert!(!state.help_overlay_next_page());
        assert!(state.help_overlay_prev_page());
        assert_eq!(state.help_overlay_page, HELP_OVERLAY_MAX_PAGES - 2);
    }

    #[test]
    fn help_search_insert_and_cursor_movement_handle_unicode_scalars() {
        let mut state = make_state();
        state.help_search_insert("a🙂");
        assert_eq!(state.help_overlay_search, "a🙂");
        assert_eq!(state.help_overlay_search_cursor, 2);

        state.help_search_cursor_left();
        assert_eq!(state.help_overlay_search_cursor, 1);
        state.help_search_cursor_right();
        assert_eq!(state.help_overlay_search_cursor, 2);
    }

    #[test]
    fn help_search_backspace_removes_previous_unicode_character() {
        let mut state = make_state();
        state.help_overlay_search = "a🙂b".to_string();
        state.help_overlay_search_cursor = 2;

        state.help_search_backspace();

        assert_eq!(state.help_overlay_search, "ab");
        assert_eq!(state.help_overlay_search_cursor, 1);
    }

    #[test]
    fn help_overlay_cursor_hint_uses_nav_region_and_overlay_bounds() {
        let mut state = make_state();
        state.toggle_help_overlay();

        assert_eq!(
            state.help_overlay_cursor_hint_at(200, 60, 1000, 800),
            Some(HelpOverlayCursorHint::Text)
        );
        assert_eq!(
            state.help_overlay_cursor_hint_at(200, 200, 1000, 800),
            Some(HelpOverlayCursorHint::Default)
        );
        assert_eq!(state.help_overlay_cursor_hint_at(5, 5, 1000, 800), None);
    }
}
