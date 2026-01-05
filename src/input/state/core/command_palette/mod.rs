//! Command palette for fuzzy action search.

mod registry;

pub use registry::{COMMAND_REGISTRY, CommandEntry};

use super::base::InputState;
use crate::config::keybindings::Action;
use crate::input::events::Key;

/// Maximum number of visible items in the command palette.
pub const COMMAND_PALETTE_MAX_VISIBLE: usize = 10;

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

    /// Get the filtered list of commands matching the current query.
    pub fn filtered_commands(&self) -> Vec<&'static CommandEntry> {
        let query = self.command_palette_query.to_lowercase();
        if query.is_empty() {
            return COMMAND_REGISTRY.iter().collect();
        }

        let mut results: Vec<(&'static CommandEntry, i32)> = COMMAND_REGISTRY
            .iter()
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
