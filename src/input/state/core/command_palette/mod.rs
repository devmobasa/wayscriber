//! Command palette for fuzzy action search.

mod input;
mod layout;
mod registry;
mod search;

pub use layout::COMMAND_PALETTE_MAX_VISIBLE;
pub(crate) use layout::{
    COMMAND_PALETTE_INPUT_HEIGHT, COMMAND_PALETTE_ITEM_HEIGHT, COMMAND_PALETTE_LIST_GAP,
    COMMAND_PALETTE_PADDING, COMMAND_PALETTE_QUERY_PLACEHOLDER,
};
pub use registry::{CommandEntry, command_palette_entries};

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
        assert_eq!(
            results[0].action,
            crate::config::keybindings::Action::ToggleFrozenMode
        );
    }

    #[test]
    fn multi_token_query_returns_file_capture_first() {
        let mut state = make_state();
        state.command_palette_query = "capture file".to_string();

        let results = state.filtered_commands();
        assert!(!results.is_empty());
        assert_eq!(
            results[0].action,
            crate::config::keybindings::Action::CaptureFileFull
        );
    }

    #[test]
    fn recent_commands_rank_first_for_empty_query() {
        let mut state = make_state();
        state.record_command_palette_action(crate::config::keybindings::Action::CaptureFileFull);
        state
            .record_command_palette_action(crate::config::keybindings::Action::TogglePresenterMode);
        state.command_palette_query.clear();

        let results = state.filtered_commands();
        assert!(results.len() >= 2);
        assert_eq!(
            results[0].action,
            crate::config::keybindings::Action::TogglePresenterMode
        );
        assert_eq!(
            results[1].action,
            crate::config::keybindings::Action::CaptureFileFull
        );
    }

    #[test]
    fn monitor_query_matches_output_focus_actions() {
        let mut state = make_state();
        state.command_palette_query = "monitor".to_string();

        let results = state.filtered_commands();
        let actions: HashSet<crate::config::keybindings::Action> =
            results.iter().map(|cmd| cmd.action).collect();
        assert!(actions.contains(&crate::config::keybindings::Action::FocusNextOutput));
        assert!(actions.contains(&crate::config::keybindings::Action::FocusPrevOutput));
    }

    #[test]
    fn display_query_matches_output_focus_actions() {
        let mut state = make_state();
        state.command_palette_query = "display".to_string();

        let results = state.filtered_commands();
        let actions: HashSet<crate::config::keybindings::Action> =
            results.iter().map(|cmd| cmd.action).collect();
        assert!(actions.contains(&crate::config::keybindings::Action::FocusNextOutput));
        assert!(actions.contains(&crate::config::keybindings::Action::FocusPrevOutput));
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
