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
    fn alias_query_matches_radial_menu_command() {
        let mut state = make_state();
        state.command_palette_query = "pie menu".to_string();

        let results = state.filtered_commands();
        assert!(!results.is_empty());
        assert_eq!(
            results[0].action,
            crate::config::keybindings::Action::ToggleRadialMenu
        );
    }

    #[test]
    fn short_label_query_matches_configurator_command() {
        let mut state = make_state();
        state.command_palette_query = "config ui".to_string();

        let results = state.filtered_commands();
        assert!(!results.is_empty());
        assert_eq!(
            results[0].action,
            crate::config::keybindings::Action::OpenConfigurator
        );
    }

    #[test]
    fn slash_separated_tokens_match_capture_file_command() {
        let mut state = make_state();
        state.command_palette_query = "capture/file".to_string();

        let results = state.filtered_commands();
        assert!(!results.is_empty());
        assert_eq!(
            results[0].action,
            crate::config::keybindings::Action::CaptureFileFull
        );
    }

    #[test]
    fn toggle_command_palette_opens_and_tracks_usage() {
        let mut state = make_state();
        assert!(!state.command_palette_open);
        assert!(!state.pending_onboarding_usage.used_command_palette);

        state.toggle_command_palette();

        assert!(state.command_palette_open);
        assert!(state.pending_onboarding_usage.used_command_palette);
        assert_eq!(state.command_palette_selected, 0);
        assert_eq!(state.command_palette_scroll, 0);
    }

    #[test]
    fn backspace_resets_selection_and_scroll_when_query_changes() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "zoom".to_string();
        state.command_palette_selected = 4;
        state.command_palette_scroll = 3;

        assert!(state.handle_command_palette_key(crate::input::Key::Backspace));
        assert_eq!(state.command_palette_query, "zoo");
        assert_eq!(state.command_palette_selected, 0);
        assert_eq!(state.command_palette_scroll, 0);
    }

    #[test]
    fn down_key_scrolls_once_selection_moves_past_visible_window() {
        let mut state = make_state();
        state.toggle_command_palette();
        assert!(state.filtered_commands().len() > COMMAND_PALETTE_MAX_VISIBLE);

        for _ in 0..COMMAND_PALETTE_MAX_VISIBLE {
            assert!(state.handle_command_palette_key(crate::input::Key::Down));
        }

        assert_eq!(state.command_palette_selected, COMMAND_PALETTE_MAX_VISIBLE);
        assert_eq!(state.command_palette_scroll, 1);
    }

    #[test]
    fn repeated_recent_action_moves_to_front_without_duplication() {
        let mut state = make_state();
        state.record_command_palette_action(crate::config::keybindings::Action::CaptureFileFull);
        state.record_command_palette_action(crate::config::keybindings::Action::ToggleHelp);
        state.record_command_palette_action(crate::config::keybindings::Action::CaptureFileFull);

        assert_eq!(
            state.command_palette_recent,
            vec![
                crate::config::keybindings::Action::CaptureFileFull,
                crate::config::keybindings::Action::ToggleHelp,
            ]
        );
    }

    #[test]
    fn escape_key_closes_command_palette() {
        let mut state = make_state();
        state.toggle_command_palette();
        assert!(state.command_palette_open);

        assert!(state.handle_command_palette_key(crate::input::Key::Escape));
        assert!(!state.command_palette_open);
    }

    #[test]
    fn return_key_executes_selected_command_and_records_it() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "status bar".to_string();
        let selected = state.selected_command().expect("selected command");
        assert_eq!(selected.action, crate::config::keybindings::Action::ToggleStatusBar);
        assert!(state.show_status_bar);

        assert!(state.handle_command_palette_key(crate::input::Key::Return));
        assert!(!state.command_palette_open);
        assert!(!state.show_status_bar);
        assert_eq!(
            state.command_palette_recent.first().copied(),
            Some(crate::config::keybindings::Action::ToggleStatusBar)
        );
    }

    #[test]
    fn clicking_outside_palette_closes_it() {
        let mut state = make_state();
        state.toggle_command_palette();

        assert!(state.handle_command_palette_click(0, 0, 1920, 1000));
        assert!(!state.command_palette_open);
    }

    #[test]
    fn char_key_appends_query_and_resets_selection_and_scroll() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_selected = 3;
        state.command_palette_scroll = 2;

        assert!(state.handle_command_palette_key(crate::input::Key::Char('z')));
        assert_eq!(state.command_palette_query, "z");
        assert_eq!(state.command_palette_selected, 0);
        assert_eq!(state.command_palette_scroll, 0);
    }

    #[test]
    fn clicking_input_region_keeps_palette_open_without_executing() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "status".to_string();
        state.command_palette_selected = 1;
        let filtered = state.filtered_commands();
        let geometry = state.command_palette_geometry(1920, 1000, filtered.len());
        let x = (geometry.x + geometry.inner_x + 4.0) as i32;
        let y = (geometry.y + geometry.input_top + 4.0) as i32;

        assert!(state.handle_command_palette_click(x, y, 1920, 1000));
        assert!(state.command_palette_open);
        assert_eq!(state.command_palette_selected, 1);
        assert!(state.ui_toast.is_none());
    }

    #[test]
    fn clicking_visible_item_executes_selected_command_and_sets_toast() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "status bar".to_string();
        state.command_palette_selected = 0;
        let filtered = state.filtered_commands();
        let selected = filtered.first().expect("selected command");
        assert_eq!(
            selected.action,
            crate::config::keybindings::Action::ToggleStatusBar
        );
        let geometry = state.command_palette_geometry(1920, 1000, filtered.len());
        let x = (geometry.x + geometry.inner_x + 4.0) as i32;
        let y = (geometry.y + geometry.items_top + COMMAND_PALETTE_ITEM_HEIGHT * 0.5) as i32;

        assert!(state.show_status_bar);
        assert!(state.handle_command_palette_click(x, y, 1920, 1000));
        assert!(!state.command_palette_open);
        assert!(!state.show_status_bar);
        let toast = state.ui_toast.as_ref().expect("command toast");
        assert_eq!(toast.kind, crate::input::state::core::base::UiToastKind::Info);
        assert_eq!(toast.message, selected.label);
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
