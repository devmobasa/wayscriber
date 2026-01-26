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
    pub(crate) fn toggle_help_overlay(&mut self) {
        let now_visible = !self.show_help;
        self.show_help = now_visible;
        // Preserve search when reopening; Escape clears it
        self.help_overlay_scroll = 0.0;
        self.help_overlay_scroll_max = 0.0;
        if now_visible {
            self.help_overlay_page = 0;
            self.help_overlay_quick_mode = false;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn toggle_quick_help(&mut self) {
        let now_visible = !self.show_help || !self.help_overlay_quick_mode;
        self.show_help = now_visible;
        self.help_overlay_quick_mode = now_visible;
        self.help_overlay_scroll = 0.0;
        self.help_overlay_scroll_max = 0.0;
        if now_visible {
            self.help_overlay_page = 0;
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
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
