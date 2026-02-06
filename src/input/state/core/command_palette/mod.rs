//! Command palette for fuzzy action search.

mod registry;

pub use registry::{CommandEntry, command_palette_entries};

use super::base::{InputState, UiToastKind};
use crate::config::action_meta::ActionCategory;
use crate::config::keybindings::Action;
use crate::input::events::Key;

/// Maximum number of visible items in the command palette.
pub const COMMAND_PALETTE_MAX_VISIBLE: usize = 10;

// Layout constants (must match ui/command_palette.rs)
const PALETTE_MIN_WIDTH: f64 = 400.0;
const PALETTE_MAX_WIDTH: f64 = 820.0;
const PALETTE_HORIZONTAL_MARGIN: f64 = 12.0;
const ITEM_HEIGHT: f64 = 32.0;
const PADDING: f64 = 12.0;
const PADDING_BOTTOM: f64 = 48.0;
const INPUT_HEIGHT: f64 = 36.0;
const LIST_GAP: f64 = 8.0;
const PALETTE_MAX_HEIGHT: f64 = 420.0;
const COMMAND_PALETTE_RECENT_LIMIT: usize = 10;
const LABEL_LEFT_PAD: f64 = 10.0;
const LABEL_DESC_GAP: f64 = 12.0;
const DESC_BADGE_GAP: f64 = 12.0;
const BADGE_RIGHT_PAD: f64 = 8.0;
const BADGE_TEXT_PADDING_X: f64 = 5.0;
const BADGE_BASE_PADDING: f64 = BADGE_TEXT_PADDING_X * 2.0;
const APPROX_LABEL_CHAR_WIDTH: f64 = 8.4;
const APPROX_DESC_CHAR_WIDTH: f64 = 6.7;
const APPROX_SHORTCUT_CHAR_WIDTH: f64 = 6.8;
const DESC_ESTIMATE_CHAR_CAP: usize = 42;
const QUERY_CHAR_WIDTH: f64 = 8.0;
const QUERY_INPUT_EXTRA_PADDING: f64 = 20.0;
const QUERY_PLACEHOLDER: &str = "Type to search commands...";

struct CommandMatch {
    command: &'static CommandEntry,
    score: i32,
    index: usize,
}

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
    pub fn command_palette_width(&self, screen_width: u32) -> f64 {
        let commands = self.filtered_commands();
        let mut required_inner_width: f64 = 0.0;

        for command in &commands {
            let label_chars = command.label.chars().count() as f64;
            let desc_chars = command
                .description
                .chars()
                .count()
                .min(DESC_ESTIMATE_CHAR_CAP) as f64;
            let shortcut_chars = self
                .action_binding_primary_label(command.action)
                .map_or(0.0, |label| label.chars().count() as f64);

            let mut row_inner = LABEL_LEFT_PAD + label_chars * APPROX_LABEL_CHAR_WIDTH;
            if desc_chars > 0.0 {
                row_inner += LABEL_DESC_GAP + desc_chars * APPROX_DESC_CHAR_WIDTH;
            }
            if shortcut_chars > 0.0 {
                let badge_width = shortcut_chars * APPROX_SHORTCUT_CHAR_WIDTH + BADGE_BASE_PADDING;
                row_inner += DESC_BADGE_GAP + badge_width;
            }
            row_inner += BADGE_RIGHT_PAD;
            required_inner_width = required_inner_width.max(row_inner);
        }

        let query_chars = self
            .command_palette_query
            .chars()
            .count()
            .max(QUERY_PLACEHOLDER.chars().count()) as f64;
        let query_inner_width =
            LABEL_LEFT_PAD + query_chars * QUERY_CHAR_WIDTH + QUERY_INPUT_EXTRA_PADDING;
        required_inner_width = required_inner_width.max(query_inner_width);

        let requested_width = required_inner_width + PADDING * 2.0;
        let max_available = (screen_width as f64 - PALETTE_HORIZONTAL_MARGIN * 2.0).max(240.0);
        requested_width.clamp(
            PALETTE_MIN_WIDTH.min(max_available),
            PALETTE_MAX_WIDTH.min(max_available),
        )
    }

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
                if let Some(command) = self.selected_command() {
                    self.command_palette_open = false;
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;
                    self.record_command_palette_action(command.action);
                    self.handle_action(command.action);
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

        let palette_width = self.command_palette_width(screen_width);
        let palette_x = (screen_width as f64 - palette_width) / 2.0;
        let palette_y = screen_height as f64 * 0.2;

        let local_x = x as f64 - palette_x;
        let local_y = y as f64 - palette_y;

        let inner_x = PADDING;
        let inner_width = palette_width - PADDING * 2.0;

        // Calculate palette height
        let filtered = self.filtered_commands();
        let visible_count = filtered.len().min(COMMAND_PALETTE_MAX_VISIBLE);
        let items_top = PADDING + INPUT_HEIGHT + LIST_GAP;
        let items_height = visible_count as f64 * ITEM_HEIGHT;
        let palette_height = (items_top + items_height + PADDING_BOTTOM).min(PALETTE_MAX_HEIGHT);

        // Check if click is outside palette bounds - close it
        if !(0.0..=palette_width).contains(&local_x) || !(0.0..=palette_height).contains(&local_y) {
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
                if let Some(command) = filtered.get(actual_index).copied() {
                    self.command_palette_open = false;
                    self.dirty_tracker.mark_full();
                    self.needs_redraw = true;
                    self.record_command_palette_action(command.action);

                    // Show brief toast feedback
                    self.set_ui_toast_with_duration(
                        UiToastKind::Info,
                        label,
                        self.command_palette_toast_duration_ms,
                    );

                    self.handle_action(command.action);
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

        let palette_width = self.command_palette_width(screen_width);
        let palette_x = (screen_width as f64 - palette_width) / 2.0;
        let palette_y = screen_height as f64 * 0.2;

        let local_x = x as f64 - palette_x;
        let local_y = y as f64 - palette_y;

        let filtered = self.filtered_commands();
        let visible_count = filtered.len().min(COMMAND_PALETTE_MAX_VISIBLE);
        let items_top = PADDING + INPUT_HEIGHT + LIST_GAP;
        let items_height = visible_count as f64 * ITEM_HEIGHT;
        let palette_height = (items_top + items_height + PADDING_BOTTOM).min(PALETTE_MAX_HEIGHT);

        // Check if outside palette bounds.
        if !(0.0..=palette_width).contains(&local_x) || !(0.0..=palette_height).contains(&local_y) {
            return None;
        }

        let inner_x = PADDING;
        let inner_width = palette_width - PADDING * 2.0;

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
        let items_top = input_bottom + LIST_GAP;

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
        let query = normalize_query(&self.command_palette_query);
        let tokens = query_tokens(&query);

        let mut results: Vec<CommandMatch> = command_palette_entries()
            .enumerate()
            .filter_map(|(index, command)| {
                self.score_command(command, &query, &tokens)
                    .map(|score| CommandMatch {
                        command,
                        score,
                        index,
                    })
            })
            .collect();

        results.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.index.cmp(&b.index)));
        results.into_iter().map(|result| result.command).collect()
    }

    fn selected_command(&self) -> Option<&'static CommandEntry> {
        let filtered = self.filtered_commands();
        filtered.get(self.command_palette_selected).copied()
    }

    fn score_command(
        &self,
        command: &'static CommandEntry,
        query: &str,
        tokens: &[&str],
    ) -> Option<i32> {
        let recent_bonus = self.recent_bonus(command.action);
        if query.is_empty() {
            return Some(recent_bonus);
        }

        let category_name = action_category_name(command.category);
        let shortcuts = self.action_binding_labels(command.action).join(" ");
        let aliases = command.search_aliases.join(" ");
        let mut score = 0;

        // Require all tokens to match somewhere for cleaner result sets.
        for token in tokens {
            let token_score = fuzzy_score(token, command.label).max(
                fuzzy_score(token, command.description)
                    .max(fuzzy_score(token, category_name))
                    .max(fuzzy_score(token, &shortcuts))
                    .max(fuzzy_score(token, &aliases))
                    .max(
                        command
                            .short_label
                            .map_or(0, |label| fuzzy_score(token, label)),
                    ),
            );
            if token_score == 0 {
                return None;
            }
            score += token_score;
        }

        score += fuzzy_score(query, command.label) * 2;
        score += fuzzy_score(query, command.description);
        score += fuzzy_score(query, &shortcuts) * 2;
        score += fuzzy_score(query, &aliases) * 2;
        score += fuzzy_score(query, category_name);
        if let Some(short_label) = command.short_label {
            score += fuzzy_score(query, short_label);
        }

        if score == 0 {
            return None;
        }

        Some(score + (recent_bonus / 2))
    }

    fn recent_bonus(&self, action: Action) -> i32 {
        self.command_palette_recent
            .iter()
            .position(|recent| *recent == action)
            .map(|idx| (COMMAND_PALETTE_RECENT_LIMIT.saturating_sub(idx) as i32) * 20)
            .unwrap_or(0)
    }

    fn record_command_palette_action(&mut self, action: Action) {
        self.command_palette_recent
            .retain(|recent| *recent != action);
        self.command_palette_recent.insert(0, action);
        if self.command_palette_recent.len() > COMMAND_PALETTE_RECENT_LIMIT {
            self.command_palette_recent
                .truncate(COMMAND_PALETTE_RECENT_LIMIT);
        }
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

fn normalize_query(query: &str) -> String {
    query.trim().to_lowercase()
}

fn query_tokens(query: &str) -> Vec<&str> {
    query
        .split(|c: char| c.is_whitespace() || c == '+' || c == '/')
        .filter(|token| !token.is_empty())
        .collect()
}

fn action_category_name(category: ActionCategory) -> &'static str {
    match category {
        ActionCategory::Core => "core",
        ActionCategory::Drawing => "drawing",
        ActionCategory::Tools => "tools",
        ActionCategory::Colors => "colors",
        ActionCategory::UI => "ui",
        ActionCategory::Board => "board",
        ActionCategory::Zoom => "zoom",
        ActionCategory::Capture => "capture",
        ActionCategory::Selection => "selection",
        ActionCategory::History => "history",
        ActionCategory::Presets => "presets",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode, InputState};
    use std::collections::HashSet;

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
    fn shortcut_query_prioritizes_bound_command() {
        let mut state = make_state();
        state.command_palette_query = "ctrl+shift+f".to_string();

        let results = state.filtered_commands();
        assert!(!results.is_empty());
        assert_eq!(results[0].action, Action::ToggleFrozenMode);
    }

    #[test]
    fn multi_token_query_returns_file_capture_first() {
        let mut state = make_state();
        state.command_palette_query = "capture file".to_string();

        let results = state.filtered_commands();
        assert!(!results.is_empty());
        assert_eq!(results[0].action, Action::CaptureFileFull);
    }

    #[test]
    fn recent_commands_rank_first_for_empty_query() {
        let mut state = make_state();
        state.record_command_palette_action(Action::CaptureFileFull);
        state.record_command_palette_action(Action::TogglePresenterMode);
        state.command_palette_query.clear();

        let results = state.filtered_commands();
        assert!(results.len() >= 2);
        assert_eq!(results[0].action, Action::TogglePresenterMode);
        assert_eq!(results[1].action, Action::CaptureFileFull);
    }

    #[test]
    fn monitor_query_matches_output_focus_actions() {
        let mut state = make_state();
        state.command_palette_query = "monitor".to_string();

        let results = state.filtered_commands();
        let actions: HashSet<Action> = results.iter().map(|cmd| cmd.action).collect();
        assert!(actions.contains(&Action::FocusNextOutput));
        assert!(actions.contains(&Action::FocusPrevOutput));
    }

    #[test]
    fn display_query_matches_output_focus_actions() {
        let mut state = make_state();
        state.command_palette_query = "display".to_string();

        let results = state.filtered_commands();
        let actions: HashSet<Action> = results.iter().map(|cmd| cmd.action).collect();
        assert!(actions.contains(&Action::FocusNextOutput));
        assert!(actions.contains(&Action::FocusPrevOutput));
    }

    #[test]
    fn cursor_hint_rejects_strip_below_clamped_panel_height() {
        let mut state = make_state();
        state.toggle_command_palette();
        assert!(state.filtered_commands().len() > COMMAND_PALETTE_MAX_VISIBLE);

        // screen_height=1000 -> panel y=200; clamped panel height=420 => bottom=620.
        // y=623 is below the rendered panel and must be treated as outside.
        let hint = state.command_palette_cursor_hint_at(960, 623, 1920, 1000);
        assert!(hint.is_none());
    }
}
