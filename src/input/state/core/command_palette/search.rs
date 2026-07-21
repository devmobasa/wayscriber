use super::super::base::InputState;
use super::{CommandEntry, command_palette_entries};
use crate::config::action_meta::{ActionCategory, ActionMeta};
use crate::domain::Action;
use crate::palette_recents::PALETTE_RECENTS_CAP;

/// In-memory recents cap; mirrors the persisted file cap so the palette and
/// `palette_recents.toml` can never disagree about history length.
const COMMAND_PALETTE_RECENT_LIMIT: usize = PALETTE_RECENTS_CAP;

/// Group label shown above recent commands when the query is empty.
pub(crate) const COMMAND_PALETTE_RECENT_HEADER: &str = "Recent";

struct CommandMatch {
    command: &'static CommandEntry,
    score: i32,
    index: usize,
}

/// One visible row of the command palette list: either a non-interactive
/// group header or a command (carrying its index into `filtered_commands()`).
/// Headers and commands share the same row height, so hit-testing and
/// scrolling stay uniform.
#[derive(Debug, Clone, Copy)]
pub enum CommandPaletteListRow {
    Header(&'static str),
    Command {
        command: &'static CommandEntry,
        command_index: usize,
    },
}

impl CommandPaletteListRow {
    pub fn command_index(&self) -> Option<usize> {
        match self {
            Self::Header(_) => None,
            Self::Command { command_index, .. } => Some(*command_index),
        }
    }
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

    /// The palette's visible row list: `filtered_commands()` with group
    /// headers interleaved where they cannot lie about grouping.
    ///
    /// Header rule (recorded for M6): fuzzy ranking always wins — headers are
    /// only inserted between existing runs, never by reordering. With an
    /// empty query the list is grouped by construction ("Recent" block, then
    /// registry-ordered categories), so headers always show. With a query,
    /// headers show only when every category present forms exactly one
    /// contiguous run in score order; any interleaving renders flat.
    pub fn command_palette_rows(&self) -> Vec<CommandPaletteListRow> {
        let filtered = self.filtered_commands();
        let query_empty = normalize_query(&self.command_palette_query).is_empty();
        let recent_len = if query_empty {
            filtered
                .iter()
                .take_while(|command| self.command_palette_recent.contains(&command.action))
                .count()
        } else {
            0
        };
        build_command_palette_rows(&filtered, query_empty, recent_len)
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

        let shortcuts = self.action_binding_labels(command.action).join(" ");
        let mut score = 0;

        // Require all tokens to match somewhere for cleaner result sets. The
        // label/short-label/description/category/alias index is shared with the
        // help overlay via `action_meta_token_score`; only the per-user shortcut
        // labels stay palette-local (folded in here with `fuzzy_score`).
        for token in tokens {
            let token_score =
                action_meta_token_score(command, token).max(fuzzy_score(token, &shortcuts));
            if token_score == 0 {
                return None;
            }
            score += token_score;
        }

        score += action_meta_query_bonus(command, query);
        score += fuzzy_score(query, &shortcuts) * 2;

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
        self.command_palette_recents_dirty = true;

        // Shortcut-coach slow-path signal: running an action from the palette
        // when that action has its own keyboard shortcut is a canonical "you
        // could have pressed the key" case (the toolbar is the other, recorded
        // in `apply_toolbar_event`). Only counts actions that resolve to a
        // shortcut so the coach can always name it.
        if self.shortcut_for_action(action).is_some() {
            self.pending_onboarding_usage
                .note_shortcut_slow_path(action);
        }
    }

    /// Seed the in-memory recents from the persisted store at startup.
    pub fn set_command_palette_recents(&mut self, recents: Vec<Action>) {
        self.command_palette_recent = recents;
        self.command_palette_recent
            .truncate(COMMAND_PALETTE_RECENT_LIMIT);
        self.command_palette_recents_dirty = false;
    }

    /// True (and reset) when the recents changed since the last drain; the
    /// backend persists the history to `palette_recents.toml` when set.
    pub fn take_command_palette_recents_dirty(&mut self) -> bool {
        std::mem::take(&mut self.command_palette_recents_dirty)
    }

    /// True while the recents differ from what has been persisted. Unlike
    /// [`Self::take_command_palette_recents_dirty`] this only *peeks*, so the
    /// backend can retain the pending write when persistence fails and only
    /// [`Self::clear_command_palette_recents_dirty`] once the write succeeds.
    pub fn command_palette_recents_dirty(&self) -> bool {
        self.command_palette_recents_dirty
    }

    /// Clear the pending-persist flag after the recents were durably written.
    pub fn clear_command_palette_recents_dirty(&mut self) {
        self.command_palette_recents_dirty = false;
    }
}

fn build_command_palette_rows(
    filtered: &[&'static CommandEntry],
    query_empty: bool,
    recent_len: usize,
) -> Vec<CommandPaletteListRow> {
    if filtered.is_empty() {
        return Vec::new();
    }
    let with_headers = query_empty || category_runs_are_unique(filtered);
    if !with_headers {
        return filtered
            .iter()
            .enumerate()
            .map(|(command_index, command)| CommandPaletteListRow::Command {
                command,
                command_index,
            })
            .collect();
    }

    let mut rows = Vec::with_capacity(filtered.len() + 8);
    let mut last_header: Option<&'static str> = None;
    for (command_index, command) in filtered.iter().enumerate() {
        let header = if command_index < recent_len {
            COMMAND_PALETTE_RECENT_HEADER
        } else {
            action_category_display_name(command.category)
        };
        if last_header != Some(header) {
            rows.push(CommandPaletteListRow::Header(header));
            last_header = Some(header);
        }
        rows.push(CommandPaletteListRow::Command {
            command,
            command_index,
        });
    }
    rows
}

/// True when every category in the ranked results occupies exactly one
/// contiguous run, so headers can be inserted without lying about order.
fn category_runs_are_unique(filtered: &[&'static CommandEntry]) -> bool {
    let mut seen: Vec<ActionCategory> = Vec::new();
    let mut current: Option<ActionCategory> = None;
    for command in filtered {
        if current != Some(command.category) {
            if seen.contains(&command.category) {
                return false;
            }
            if let Some(previous) = current {
                seen.push(previous);
            }
            current = Some(command.category);
        }
    }
    true
}

/// Display index of a command (by its `filtered_commands()` index) within the
/// palette row list.
pub(crate) fn command_palette_display_index(
    rows: &[CommandPaletteListRow],
    command_index: usize,
) -> usize {
    rows.iter()
        .position(|row| row.command_index() == Some(command_index))
        .unwrap_or(0)
}

/// Simple fuzzy matching score, shared by the command palette and the help
/// overlay search so both surfaces rank identically.
pub(crate) fn fuzzy_score(query: &str, text: &str) -> i32 {
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

/// Fuzzy-score a single query `token` against an action's static search model:
/// its label, short label, description, category name, and search aliases. This
/// is the command palette's per-token index minus the per-user shortcut labels
/// (callers that have shortcut strings fold those in with their own
/// [`fuzzy_score`]). Shared with the help overlay so alias queries like
/// "pie menu" rank the same on both surfaces.
pub(crate) fn action_meta_token_score(meta: &ActionMeta, token: &str) -> i32 {
    let aliases = meta.search_aliases.join(" ");
    fuzzy_score(token, meta.label)
        .max(fuzzy_score(token, meta.description))
        .max(fuzzy_score(token, action_category_name(meta.category)))
        .max(fuzzy_score(token, &aliases))
        .max(
            meta.short_label
                .map_or(0, |label| fuzzy_score(token, label)),
        )
}

/// Full-query bonus over an action's static search model, mirroring the
/// command palette's query-level weighting minus shortcuts. Kept beside
/// [`action_meta_token_score`] so the two surfaces share one ranking model.
fn action_meta_query_bonus(meta: &ActionMeta, query: &str) -> i32 {
    let aliases = meta.search_aliases.join(" ");
    let mut bonus = fuzzy_score(query, meta.label) * 2;
    bonus += fuzzy_score(query, meta.description);
    bonus += fuzzy_score(query, &aliases) * 2;
    bonus += fuzzy_score(query, action_category_name(meta.category));
    if let Some(short_label) = meta.short_label {
        bonus += fuzzy_score(query, short_label);
    }
    bonus
}

fn normalize_query(query: &str) -> String {
    query.trim().to_lowercase()
}

pub(crate) fn query_tokens(query: &str) -> Vec<&str> {
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

/// Human-cased category names for palette group headers.
fn action_category_display_name(category: ActionCategory) -> &'static str {
    match category {
        ActionCategory::Core => "Core",
        ActionCategory::Drawing => "Drawing",
        ActionCategory::Tools => "Tools",
        ActionCategory::Colors => "Colors",
        ActionCategory::UI => "Interface",
        ActionCategory::Board => "Boards",
        ActionCategory::Zoom => "Zoom",
        ActionCategory::Capture => "Capture",
        ActionCategory::Selection => "Selection",
        ActionCategory::History => "History",
        ActionCategory::Presets => "Presets",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_query_trims_and_lowercases() {
        assert_eq!(normalize_query("  Ctrl+K / Zoom  "), "ctrl+k / zoom");
    }

    #[test]
    fn query_tokens_split_on_whitespace_plus_and_slash() {
        assert_eq!(
            query_tokens("ctrl+shift/file open"),
            vec!["ctrl", "shift", "file", "open"]
        );
    }

    #[test]
    fn fuzzy_score_prefers_prefix_matches_over_subsequence_matches() {
        assert!(fuzzy_score("cap", "capture to file") > fuzzy_score("cap", "clipboard action"));
    }

    #[test]
    fn fuzzy_score_prefers_word_boundary_matches_over_plain_substrings() {
        assert!(fuzzy_score("bar", "status bar") > fuzzy_score("bar", "crowbar"));
    }

    #[test]
    fn action_category_name_covers_palette_categories() {
        assert_eq!(action_category_name(ActionCategory::Capture), "capture");
        assert_eq!(action_category_name(ActionCategory::Presets), "presets");
    }

    #[test]
    fn action_meta_token_score_matches_search_aliases() {
        let radial = command_palette_entries()
            .find(|meta| meta.action == Action::ToggleRadialMenu)
            .expect("radial menu entry");
        assert!(!radial.search_aliases.is_empty(), "test relies on an alias");
        // "pie" only appears in the "pie menu" alias, never in the label
        // ("Radial Menu") or description, so a non-zero score proves the shared
        // model indexes aliases.
        assert!(action_meta_token_score(radial, "pie") > 0);
        assert_eq!(action_meta_token_score(radial, "zznomatch"), 0);
    }

    #[test]
    fn category_runs_are_unique_detects_split_runs() {
        fn entry(category: ActionCategory) -> &'static CommandEntry {
            command_palette_entries()
                .find(|meta| meta.category == category)
                .expect("category entry")
        }
        let zoom = entry(ActionCategory::Zoom);
        let ui = entry(ActionCategory::UI);
        assert!(category_runs_are_unique(&[zoom, zoom, ui]));
        assert!(!category_runs_are_unique(&[zoom, ui, zoom]));
    }
}
