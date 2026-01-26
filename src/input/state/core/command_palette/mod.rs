//! Command palette for fuzzy action search.

mod registry;

pub use registry::{CommandEntry, command_palette_entries};

use super::base::{InputState, UiToastKind};
use crate::config::keybindings::Action;
use crate::input::events::Key;

/// Duration for command palette action toast (ms).
const COMMAND_TOAST_DURATION_MS: u64 = 1200;

/// Maximum number of visible items in the command palette.
pub const COMMAND_PALETTE_MAX_VISIBLE: usize = 10;

// Layout constants (must match ui/command_palette.rs)
const PALETTE_WIDTH: f64 = 400.0;
const ITEM_HEIGHT: f64 = 32.0;
const PADDING: f64 = 12.0;
const INPUT_HEIGHT: f64 = 36.0;

/// Cursor hint for different regions of the command palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandPaletteCursorHint {
    /// Default arrow cursor.
    Default,
    /// Text editing cursor (I-beam) for input field.
    Text,
    /// Pointer/hand cursor for command items.
    Pointer,
}

impl InputState {
    /// Toggle the command palette visibility.
    pub(crate) fn toggle_command_palette(&mut self) {
        self.command_palette_open = !self.command_palette_open;
        if self.command_palette_open {
            self.command_palette_query.clear();
            self.command_palette_selected = 0;
            self.command_palette_scroll = 0;
            // Close other overlays
            if self.show_help {
                self.show_help = false;
            }
            if self.tour_active {
                self.tour_active = false;
            }
            self.close_context_menu();
            self.close_properties_panel();
        }
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    /// Handle a key press while the command palette is open.
    /// Returns true if the key was handled.
    pub(crate) fn handle_command_palette_key(&mut self, key: Key) -> bool {
        if !self.command_palette_open {
            return false;
        }

        match key {
            Key::Escape => {
                self.command_palette_open = false;
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                true
            }
            Key::Return => {
                if let Some(action) = self.execute_selected_command() {
                    self.command_palette_open = false;
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;
                    self.handle_action(action);
                }
                true
            }
            Key::Up => {
                if self.command_palette_selected > 0 {
                    self.command_palette_selected -= 1;
                    // Adjust scroll if selection moves above visible window
                    if self.command_palette_selected < self.command_palette_scroll {
                        self.command_palette_scroll = self.command_palette_selected;
                    }
                    self.needs_redraw = true;
                }
                true
            }
            Key::Down => {
                let filtered = self.filtered_commands();
                if self.command_palette_selected + 1 < filtered.len() {
                    self.command_palette_selected += 1;
                    // Adjust scroll if selection moves below visible window
                    if self.command_palette_selected
                        >= self.command_palette_scroll + COMMAND_PALETTE_MAX_VISIBLE
                    {
                        self.command_palette_scroll =
                            self.command_palette_selected - COMMAND_PALETTE_MAX_VISIBLE + 1;
                    }
                    self.needs_redraw = true;
                }
                true
            }
            Key::Backspace => {
                if !self.command_palette_query.is_empty() {
                    self.command_palette_query.pop();
                    self.command_palette_selected = 0;
                    self.command_palette_scroll = 0;
                    self.needs_redraw = true;
                }
                true
            }
            Key::Char(ch) if !ch.is_control() => {
                self.command_palette_query.push(ch);
                self.command_palette_selected = 0;
                self.command_palette_scroll = 0;
                self.needs_redraw = true;
                true
            }
            Key::Space => {
                self.command_palette_query.push(' ');
                self.command_palette_selected = 0;
                self.command_palette_scroll = 0;
                self.needs_redraw = true;
                true
            }
            _ => true, // Consume all other keys while palette is open
        }
    }

    /// Handle a mouse click while the command palette is open.
    /// Returns true if the click was handled (either on an item or to close the palette).
    pub fn handle_command_palette_click(
        &mut self,
        x: i32,
        y: i32,
        screen_width: u32,
        screen_height: u32,
    ) -> bool {
        if !self.command_palette_open {
            return false;
        }

        let palette_x = (screen_width as f64 - PALETTE_WIDTH) / 2.0;
        let palette_y = screen_height as f64 * 0.2;

        let local_x = x as f64 - palette_x;
        let local_y = y as f64 - palette_y;

        let inner_x = PADDING;
        let inner_width = PALETTE_WIDTH - PADDING * 2.0;

        // Calculate palette height
        let filtered = self.filtered_commands();
        let visible_count = filtered.len().min(COMMAND_PALETTE_MAX_VISIBLE);
        let items_top = PADDING + INPUT_HEIGHT + 8.0;
        let items_height = visible_count as f64 * ITEM_HEIGHT;
        let palette_height = items_top + items_height + PADDING;

        // Check if click is outside palette bounds - close it
        if !(0.0..=PALETTE_WIDTH).contains(&local_x) || !(0.0..=palette_height).contains(&local_y) {
            self.command_palette_open = false;
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            return true;
        }

        // Check command items region
        for i in 0..visible_count {
            let item_top = items_top + (i as f64 * ITEM_HEIGHT);
            let item_bottom = item_top + ITEM_HEIGHT;
            if local_y >= item_top
                && local_y <= item_bottom
                && local_x >= inner_x
                && local_x <= inner_x + inner_width
            {
                // Clicked on item at visible index i, actual index accounts for scroll
                let actual_index = self.command_palette_scroll + i;
                self.command_palette_selected = actual_index;

                // Get the command label for feedback before executing
                let label = filtered
                    .get(actual_index)
                    .map(|cmd| cmd.label)
                    .unwrap_or("Command");

                // Execute the command
                if let Some(action) = self.execute_selected_command() {
                    self.command_palette_open = false;
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;

                    // Show brief toast feedback
                    self.set_ui_toast_with_duration(
                        UiToastKind::Info,
                        label,
                        COMMAND_TOAST_DURATION_MS,
                    );

                    self.handle_action(action);
                }
                return true;
            }
        }

        // Click was inside palette but not on an item (e.g., on input field or padding)
        true
    }

    /// Determine the cursor type for a given point within the command palette.
    /// Returns `None` if the command palette is not open or the point is outside.
    pub fn command_palette_cursor_hint_at(
        &self,
        x: i32,
        y: i32,
        screen_width: u32,
        screen_height: u32,
    ) -> Option<CommandPaletteCursorHint> {
        if !self.command_palette_open {
            return None;
        }

        let palette_x = (screen_width as f64 - PALETTE_WIDTH) / 2.0;
        let palette_y = screen_height as f64 * 0.2;

        let local_x = x as f64 - palette_x;
        let local_y = y as f64 - palette_y;

        // Check if outside palette bounds (rough check)
        if !(0.0..=PALETTE_WIDTH).contains(&local_x) || local_y < 0.0 {
            return None;
        }

        let inner_x = PADDING;
        let inner_width = PALETTE_WIDTH - PADDING * 2.0;

        // Check input field region
        let input_top = PADDING;
        let input_bottom = input_top + INPUT_HEIGHT;
        if local_y >= input_top
            && local_y <= input_bottom
            && local_x >= inner_x
            && local_x <= inner_x + inner_width
        {
            return Some(CommandPaletteCursorHint::Text);
        }

        // Check command items region
        let items_top = input_bottom + 8.0;
        let filtered = self.filtered_commands();
        let visible_count = filtered.len().min(COMMAND_PALETTE_MAX_VISIBLE);

        for i in 0..visible_count {
            let item_top = items_top + (i as f64 * ITEM_HEIGHT);
            let item_bottom = item_top + ITEM_HEIGHT;
            if local_y >= item_top
                && local_y <= item_bottom
                && local_x >= inner_x
                && local_x <= inner_x + inner_width
            {
                return Some(CommandPaletteCursorHint::Pointer);
            }
        }

        Some(CommandPaletteCursorHint::Default)
    }

    /// Get the filtered list of commands matching the current query.
    pub fn filtered_commands(&self) -> Vec<&'static CommandEntry> {
        let query = self.command_palette_query.to_lowercase();
        if query.is_empty() {
            return command_palette_entries().collect();
        }

        let mut results: Vec<(&'static CommandEntry, i32)> = command_palette_entries()
            .filter_map(|cmd| {
                let score = fuzzy_score(&query, cmd.label) + fuzzy_score(&query, cmd.description);
                if score > 0 { Some((cmd, score)) } else { None }
            })
            .collect();

        results.sort_by(|a, b| b.1.cmp(&a.1));
        results.into_iter().map(|(cmd, _)| cmd).collect()
    }

    /// Execute the currently selected command.
    fn execute_selected_command(&self) -> Option<Action> {
        let filtered = self.filtered_commands();
        filtered
            .get(self.command_palette_selected)
            .map(|cmd| cmd.action)
    }
}

/// Simple fuzzy matching score.
fn fuzzy_score(query: &str, text: &str) -> i32 {
    let text_lower = text.to_lowercase();

    // Exact prefix match
    if text_lower.starts_with(query) {
        return 100;
    }

    // Word boundary matches
    let words: Vec<&str> = text_lower.split_whitespace().collect();
    for word in &words {
        if word.starts_with(query) {
            return 75;
        }
    }

    // Substring match
    if text_lower.contains(query) {
        return 25;
    }

    // Check if all query chars appear in order
    let mut text_chars = text_lower.chars().peekable();
    let mut matched = 0;
    let query_len = query.chars().count();
    for qc in query.chars() {
        while let Some(&tc) = text_chars.peek() {
            text_chars.next();
            if tc == qc {
                matched += 1;
                break;
            }
        }
    }
    if matched == query_len {
        return 10;
    }

    0
}
