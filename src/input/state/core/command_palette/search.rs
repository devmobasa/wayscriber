use super::super::base::InputState;
use super::{CommandEntry, command_palette_entries};
use crate::config::action_meta::ActionCategory;
use crate::config::keybindings::Action;

const COMMAND_PALETTE_RECENT_LIMIT: usize = 10;

struct CommandMatch {
    command: &'static CommandEntry,
    score: i32,
    index: usize,
}

impl InputState {
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

    pub(super) fn selected_command(&self) -> Option<&'static CommandEntry> {
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
            .map_or(0, |idx| {
                (COMMAND_PALETTE_RECENT_LIMIT.saturating_sub(idx) as i32) * 20
            })
    }

    pub(super) fn record_command_palette_action(&mut self, action: Action) {
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
