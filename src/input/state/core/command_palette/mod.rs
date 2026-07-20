//! Command palette for fuzzy action search.

mod input;
mod layout;
mod registry;
mod search;

pub use layout::COMMAND_PALETTE_MAX_VISIBLE;
pub(crate) use layout::{
    COMMAND_PALETTE_INPUT_HEIGHT, COMMAND_PALETTE_ITEM_HEIGHT, COMMAND_PALETTE_LIST_GAP,
    COMMAND_PALETTE_PADDING, COMMAND_PALETTE_QUERY_PLACEHOLDER, COMMAND_PALETTE_ROW_ACTION_COUNT,
    COMMAND_PALETTE_ROW_ACTION_GAP, COMMAND_PALETTE_ROW_ACTION_SIZE, COMMAND_PALETTE_ROW_ICON_GAP,
    COMMAND_PALETTE_ROW_ICON_SIZE, COMMAND_PALETTE_TOP_RATIO,
};
pub use registry::{CommandEntry, command_palette_entries};
pub use search::CommandPaletteListRow;
pub(crate) use search::{action_meta_token_score, fuzzy_score, query_tokens};

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
    use crate::config::keybindings::Action;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode, InputState};
    use search::command_palette_display_index;
    use std::collections::HashSet;
    use std::time::{Duration, Instant};

    /// Screen-space centre of the row that renders `command_index`, accounting
    /// for the group headers interleaved into the display list. Callers click
    /// here to exercise a specific command through the real hit geometry.
    fn command_row_click_point(
        state: &InputState,
        command_index: usize,
        screen_width: u32,
        screen_height: u32,
    ) -> (i32, i32) {
        let rows = state.command_palette_rows();
        let display_index = command_palette_display_index(&rows, command_index);
        let geometry = state.command_palette_geometry(screen_width, screen_height, rows.len());
        let x = (geometry.x + geometry.inner_x + 4.0) as i32;
        let y = (geometry.y
            + geometry.items_top
            + (display_index - state.command_palette_scroll) as f64 * COMMAND_PALETTE_ITEM_HEIGHT
            + COMMAND_PALETTE_ITEM_HEIGHT * 0.5) as i32;
        (x, y)
    }

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
    fn shortcut_capture_emits_a_replace_request() {
        let mut state = make_state();
        assert!(state.begin_keybinding_capture(Action::SelectPenTool));
        assert!(state.handle_command_palette_key(crate::input::Key::Ctrl));
        assert!(state.handle_command_palette_key(crate::input::Key::Char('p')));

        assert_eq!(
            state.take_pending_backend_action(),
            Some(crate::input::state::PendingBackendAction::EditKeybinding(
                crate::input::state::KeybindingEditRequest {
                    action: Action::SelectPenTool,
                    operation: crate::input::state::KeybindingEditOperation::Replace(vec![
                        "Ctrl+P".to_string()
                    ]),
                }
            ))
        );
        assert_eq!(state.keybinding_capture_action, None);
    }

    #[test]
    fn palette_shortcut_controls_request_delete_and_reset() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "pen tool".to_string();
        let action = state.selected_command().expect("selected command").action;

        state.modifiers.ctrl = true;
        assert!(state.handle_command_palette_key(crate::input::Key::Delete));
        assert_eq!(
            state.take_pending_backend_action(),
            Some(crate::input::state::PendingBackendAction::EditKeybinding(
                crate::input::state::KeybindingEditRequest {
                    action,
                    operation: crate::input::state::KeybindingEditOperation::Delete,
                }
            ))
        );

        assert!(state.handle_command_palette_key(crate::input::Key::Char('r')));
        assert_eq!(
            state.take_pending_backend_action(),
            Some(crate::input::state::PendingBackendAction::EditKeybinding(
                crate::input::state::KeybindingEditRequest {
                    action,
                    operation: crate::input::state::KeybindingEditOperation::Reset,
                }
            ))
        );
    }

    #[test]
    fn palette_edit_icon_starts_capture_without_running_command() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "pen tool".to_string();
        let filtered = state.filtered_commands();
        let action = filtered.first().expect("matching command").action;
        let geometry = state.command_palette_geometry(1920, 1000, filtered.len());
        let stride =
            layout::COMMAND_PALETTE_ROW_ACTION_SIZE + layout::COMMAND_PALETTE_ROW_ACTION_GAP;
        let actions_left = geometry.inner_x + geometry.inner_width
            - stride * layout::COMMAND_PALETTE_ROW_ACTION_COUNT as f64;
        let x = (geometry.x + actions_left + 2.0).round() as i32;
        let y = (geometry.y + geometry.items_top + 4.0).round() as i32;

        assert!(state.handle_command_palette_click(x, y, 1920, 1000));
        assert_eq!(state.keybinding_capture_action, Some(action));
        assert!(state.command_palette_open);
        assert!(state.take_pending_backend_action().is_none());
    }

    #[test]
    fn palette_shortcut_controls_expose_specific_tooltips() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "pen tool".to_string();
        let filtered = state.filtered_commands();
        let geometry = state.command_palette_geometry(1920, 1000, filtered.len());
        let stride =
            layout::COMMAND_PALETTE_ROW_ACTION_SIZE + layout::COMMAND_PALETTE_ROW_ACTION_GAP;
        let actions_left = geometry.inner_x + geometry.inner_width
            - stride * layout::COMMAND_PALETTE_ROW_ACTION_COUNT as f64;
        let y = (geometry.y + geometry.items_top + 4.0).round() as i32;

        for (slot, expected) in [
            "Edit shortcut",
            "Unbind shortcut",
            "Reset shortcut to default",
        ]
        .into_iter()
        .enumerate()
        {
            let x = (geometry.x + actions_left + stride * slot as f64 + 2.0).round() as i32;
            state.update_pointer_position(x, y);
            assert_eq!(
                state
                    .command_palette_action_tooltip(1920, 1000)
                    .map(|(tooltip, _, _)| tooltip),
                Some(expected)
            );
        }

        let (_, _, visual_width, _) =
            crate::ui::command_palette_visual_geometry(&state, 1920, 1000)
                .expect("palette plus tooltip geometry");
        assert!(visual_width > geometry.width + 4.0);
    }

    fn assert_palette_finds(query: &str, action: Action) {
        let mut state = make_state();
        state.command_palette_query = query.to_string();

        let results = state.filtered_commands();
        assert!(
            results.iter().any(|cmd| cmd.action == action),
            "expected query {query:?} to find {action:?}, got {:?}",
            results.iter().map(|cmd| cmd.action).collect::<Vec<_>>()
        );
    }

    #[test]
    fn board_and_page_lifecycle_commands_are_searchable() {
        assert_palette_finds("new board", Action::BoardNew);
        assert_palette_finds("delete board", Action::BoardDelete);
        assert_palette_finds("duplicate page", Action::PageDuplicate);
        assert_palette_finds("delete page", Action::PageDelete);
        assert_palette_finds("restore page", Action::PageRestoreDeleted);
    }

    #[test]
    fn hidden_utility_commands_are_searchable() {
        assert_palette_finds("increase marker opacity", Action::IncreaseMarkerOpacity);
        assert_palette_finds("decrease font size", Action::DecreaseFontSize);
        assert_palette_finds("reset arrow labels", Action::ResetArrowLabelCounter);
        assert_palette_finds("reset step markers", Action::ResetStepMarkerCounter);
        assert_palette_finds("selection properties", Action::ToggleSelectionProperties);
        assert_palette_finds("context menu", Action::OpenContextMenu);
        assert_palette_finds("refresh zoom", Action::RefreshZoomCapture);
    }

    #[test]
    fn save_preset_commands_are_searchable_by_label_and_shortcut() {
        for (slot, action) in [
            Action::SavePreset1,
            Action::SavePreset2,
            Action::SavePreset3,
            Action::SavePreset4,
            Action::SavePreset5,
        ]
        .into_iter()
        .enumerate()
        {
            let slot = slot + 1;
            assert_palette_finds(&format!("save preset {slot}"), action);
            assert_palette_finds(&format!("shift+{slot}"), action);
        }
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
    fn ctrl_backspace_deletes_previous_query_word_and_resets_position() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "export canvas clipboard  ".to_string();
        state.command_palette_selected = 4;
        state.command_palette_scroll = 3;

        assert!(state.handle_command_palette_key(crate::input::Key::Ctrl));
        assert!(state.handle_command_palette_key(crate::input::Key::Backspace));

        assert_eq!(state.command_palette_query, "export canvas ");
        assert_eq!(state.command_palette_selected, 0);
        assert_eq!(state.command_palette_scroll, 0);
    }

    #[test]
    fn ctrl_backspace_stops_at_shortcut_token_separator() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "ctrl+shift+f".to_string();

        assert!(state.handle_command_palette_key(crate::input::Key::Ctrl));
        assert!(state.handle_command_palette_key(crate::input::Key::Backspace));

        assert_eq!(state.command_palette_query, "ctrl+shift+");
    }

    #[test]
    fn ctrl_backspace_stops_at_slash_token_separator() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "capture/file".to_string();

        assert!(state.handle_command_palette_key(crate::input::Key::Ctrl));
        assert!(state.handle_command_palette_key(crate::input::Key::Backspace));

        assert_eq!(state.command_palette_query, "capture/");
    }

    #[test]
    fn ctrl_u_clears_query_and_resets_position() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "status bar".to_string();
        state.command_palette_selected = 4;
        state.command_palette_scroll = 3;

        assert!(state.handle_command_palette_key(crate::input::Key::Ctrl));
        assert!(state.handle_command_palette_key(crate::input::Key::Char('u')));

        assert!(state.command_palette_query.is_empty());
        assert_eq!(state.command_palette_selected, 0);
        assert_eq!(state.command_palette_scroll, 0);
    }

    #[test]
    fn down_key_keeps_selection_visible_while_scrolling_past_headers() {
        let mut state = make_state();
        state.toggle_command_palette();
        // Enough commands that walking twice the window is always valid.
        assert!(state.filtered_commands().len() > COMMAND_PALETTE_MAX_VISIBLE * 2);
        assert!(state.command_palette_rows().len() > COMMAND_PALETTE_MAX_VISIBLE);

        let mut scrolled = false;
        for step in 0..COMMAND_PALETTE_MAX_VISIBLE * 2 {
            assert!(state.handle_command_palette_key(crate::input::Key::Down));
            // Selection advances one command per press.
            assert_eq!(state.command_palette_selected, step + 1);
            // The selected command's display row stays inside the visible window
            // even though interleaved group headers consume display rows.
            let rows = state.command_palette_rows();
            let display = command_palette_display_index(&rows, state.command_palette_selected);
            assert!(display >= state.command_palette_scroll);
            assert!(display < state.command_palette_scroll + COMMAND_PALETTE_MAX_VISIBLE);
            if state.command_palette_scroll > 0 {
                scrolled = true;
            }
        }
        assert!(
            scrolled,
            "list must scroll once the selection leaves the first window"
        );
    }

    #[test]
    fn home_key_jumps_to_first_command() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_selected = 5;
        state.command_palette_scroll = 3;

        assert!(state.handle_command_palette_key(crate::input::Key::Home));

        assert_eq!(state.command_palette_selected, 0);
        assert_eq!(state.command_palette_scroll, 0);
    }

    #[test]
    fn end_key_jumps_to_last_command_and_scrolls_into_view() {
        let mut state = make_state();
        state.toggle_command_palette();
        let filtered_len = state.filtered_commands().len();
        assert!(filtered_len > COMMAND_PALETTE_MAX_VISIBLE);

        assert!(state.handle_command_palette_key(crate::input::Key::End));

        assert_eq!(state.command_palette_selected, filtered_len - 1);
        // Scroll is measured in display rows (headers included), so the bottom of
        // the list aligns to the display length, not the command count.
        let rows = state.command_palette_rows();
        assert_eq!(
            state.command_palette_scroll,
            rows.len() - COMMAND_PALETTE_MAX_VISIBLE
        );
        // The last command sits inside the visible window.
        let display = command_palette_display_index(&rows, state.command_palette_selected);
        assert!(display >= state.command_palette_scroll);
        assert!(display < state.command_palette_scroll + COMMAND_PALETTE_MAX_VISIBLE);
    }

    #[test]
    fn held_down_key_repeats_after_delay_until_release() {
        let mut state = make_state();
        state.toggle_command_palette();

        assert!(state.handle_command_palette_key(crate::input::Key::Down));
        assert_eq!(state.command_palette_selected, 1);
        assert!(
            state
                .command_palette_repeat_timeout(Instant::now())
                .is_some()
        );

        assert!(state.tick_command_palette_repeat(Instant::now() + Duration::from_secs(1)));
        assert_eq!(state.command_palette_selected, 2);

        state.on_key_release(crate::input::Key::Down);
        assert!(
            state
                .command_palette_repeat_timeout(Instant::now())
                .is_none()
        );
        assert!(!state.tick_command_palette_repeat(Instant::now() + Duration::from_secs(1)));
        assert_eq!(state.command_palette_selected, 2);
    }

    #[test]
    fn record_command_palette_action_notes_shortcut_coach_slow_path() {
        let mut state = make_state();
        let action = crate::config::keybindings::Action::Undo;
        assert!(
            state.shortcut_for_action(action).is_some(),
            "test relies on Undo having a default shortcut"
        );

        state.record_command_palette_action(action);
        assert_eq!(
            state.pending_onboarding_usage.shortcut_slow_path_action,
            Some(action),
            "palette run of a shortcut-bound action feeds the coach slow path"
        );
        assert_eq!(state.pending_onboarding_usage.shortcut_slow_path_repeats, 1);

        // Repeated slow-path runs accumulate the streak.
        state.record_command_palette_action(action);
        assert_eq!(state.pending_onboarding_usage.shortcut_slow_path_repeats, 2);
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
        assert_eq!(
            selected.action,
            crate::config::keybindings::Action::ToggleStatusBar
        );
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
    fn return_key_sets_pending_canvas_export_backend_action() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "export canvas clipboard".to_string();
        let selected = state.selected_command().expect("selected command");
        assert_eq!(
            selected.action,
            crate::config::keybindings::Action::ExportCanvasClipboard
        );

        assert!(state.handle_command_palette_key(crate::input::Key::Return));

        assert_eq!(
            state.take_pending_backend_action(),
            Some(crate::input::state::PendingBackendAction::CanvasExport(
                crate::config::keybindings::Action::ExportCanvasClipboard
            ))
        );
    }

    #[test]
    fn return_key_sets_pending_board_pdf_export_backend_action() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "export pdf".to_string();
        let selected = state.selected_command().expect("selected command");
        assert_eq!(
            selected.action,
            crate::config::keybindings::Action::ExportBoardPdfFile
        );

        assert!(state.handle_command_palette_key(crate::input::Key::Return));

        assert_eq!(
            state.take_pending_backend_action(),
            Some(crate::input::state::PendingBackendAction::BoardPdfExport(
                crate::config::keybindings::Action::ExportBoardPdfFile
            ))
        );
    }

    #[test]
    fn return_key_sets_pending_all_boards_pdf_export_backend_action() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "all boards pdf".to_string();
        let selected = state.selected_command().expect("selected command");
        assert_eq!(
            selected.action,
            crate::config::keybindings::Action::ExportAllBoardsPdfFile
        );

        assert!(state.handle_command_palette_key(crate::input::Key::Return));

        assert_eq!(
            state.take_pending_backend_action(),
            Some(crate::input::state::PendingBackendAction::BoardPdfExport(
                crate::config::keybindings::Action::ExportAllBoardsPdfFile
            ))
        );
    }

    #[test]
    fn return_key_sets_pending_clear_saved_tool_state_backend_action() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "clear saved tool state".to_string();
        let selected = state.selected_command().expect("selected command");
        assert_eq!(
            selected.action,
            crate::config::keybindings::Action::ClearSavedToolState
        );

        assert!(state.handle_command_palette_key(crate::input::Key::Return));

        assert_eq!(
            state.take_pending_backend_action(),
            Some(crate::input::state::PendingBackendAction::ClearSavedToolState)
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
        let (x, y) = command_row_click_point(&state, 0, 1920, 1000);

        assert!(state.show_status_bar);
        assert!(state.handle_command_palette_click(x, y, 1920, 1000));
        assert!(!state.command_palette_open);
        assert!(!state.show_status_bar);
        let toast = state.ui_toast.as_ref().expect("command toast");
        assert_eq!(
            toast.kind,
            crate::input::state::core::base::UiToastKind::Info
        );
        assert_eq!(toast.message, selected.label);
    }

    #[test]
    fn clicking_visible_canvas_export_item_sets_pending_backend_action() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "save board".to_string();
        state.command_palette_selected = 0;
        let filtered = state.filtered_commands();
        assert_eq!(
            filtered.first().expect("selected command").action,
            crate::config::keybindings::Action::ExportCanvasFile
        );
        let (x, y) = command_row_click_point(&state, 0, 1920, 1000);

        assert!(state.handle_command_palette_click(x, y, 1920, 1000));

        assert_eq!(
            state.take_pending_backend_action(),
            Some(crate::input::state::PendingBackendAction::CanvasExport(
                crate::config::keybindings::Action::ExportCanvasFile
            ))
        );
    }

    #[test]
    fn clicking_visible_board_pdf_export_item_sets_pending_backend_action() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "export pdf".to_string();
        state.command_palette_selected = 0;
        let filtered = state.filtered_commands();
        assert_eq!(
            filtered.first().expect("selected command").action,
            crate::config::keybindings::Action::ExportBoardPdfFile
        );
        let (x, y) = command_row_click_point(&state, 0, 1920, 1000);

        assert!(state.handle_command_palette_click(x, y, 1920, 1000));

        assert_eq!(
            state.take_pending_backend_action(),
            Some(crate::input::state::PendingBackendAction::BoardPdfExport(
                crate::config::keybindings::Action::ExportBoardPdfFile
            ))
        );
    }

    #[test]
    fn clicking_visible_all_boards_pdf_export_item_sets_pending_backend_action() {
        let mut state = make_state();
        state.toggle_command_palette();
        state.command_palette_query = "all boards pdf".to_string();
        state.command_palette_selected = 0;
        let filtered = state.filtered_commands();
        assert_eq!(
            filtered.first().expect("selected command").action,
            crate::config::keybindings::Action::ExportAllBoardsPdfFile
        );
        let (x, y) = command_row_click_point(&state, 0, 1920, 1000);

        assert!(state.handle_command_palette_click(x, y, 1920, 1000));

        assert_eq!(
            state.take_pending_backend_action(),
            Some(crate::input::state::PendingBackendAction::BoardPdfExport(
                crate::config::keybindings::Action::ExportAllBoardsPdfFile
            ))
        );
    }

    #[test]
    fn cursor_hint_rejects_strip_below_clamped_panel_height() {
        let mut state = make_state();
        state.toggle_command_palette();
        let rows = state.command_palette_rows();
        assert!(rows.len() > COMMAND_PALETTE_MAX_VISIBLE);

        // A point just below the clamped panel bottom is outside the overlay.
        let geometry = state.command_palette_geometry(1920, 1000, rows.len());
        let inside_x = (geometry.x + geometry.width / 2.0) as i32;
        let below_y = (geometry.y + geometry.height + 3.0) as i32;
        assert!(
            state
                .command_palette_cursor_hint_at(inside_x, below_y, 1920, 1000)
                .is_none()
        );

        // A point on the first visible row still reports a hint, so the
        // rejection above is about the height clamp, not a blanket None.
        let within_y = (geometry.y + geometry.items_top + COMMAND_PALETTE_ITEM_HEIGHT * 0.5) as i32;
        assert!(
            state
                .command_palette_cursor_hint_at(inside_x, within_y, 1920, 1000)
                .is_some()
        );
    }

    #[test]
    fn visual_damage_bounds_cover_palette_without_covering_the_screen() {
        let mut state = make_state();
        state.toggle_command_palette();

        let (x, y, width, height) = crate::ui::command_palette_visual_geometry(&state, 3840, 2160)
            .expect("open palette geometry");

        assert!(x > 0.0 && y > 0.0);
        assert!(width > 0.0 && width < 3840.0);
        assert!(height > 0.0 && height < 2160.0);
        assert!(x + width <= 3840.0);
        assert!(y + height <= 2160.0);
    }
}
