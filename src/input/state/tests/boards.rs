use super::*;
use crate::input::{BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD};
use crate::input::state::core::board_picker::BoardPickerState;

#[test]
fn switch_board_force_does_not_toggle_back_to_transparent() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);

    state.switch_board_force(BOARD_ID_WHITEBOARD);
    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);
}

#[test]
fn switch_board_recent_skips_current_and_missing_entries() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    state.board_recent = vec![
        BOARD_ID_WHITEBOARD.to_string(),
        "missing".to_string(),
        "blackboard".to_string(),
    ];

    state.switch_board_recent();

    assert_eq!(state.board_id(), "blackboard");
}

#[test]
fn switch_board_recent_shows_toast_when_no_other_recent_board_exists() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    state.board_recent = vec![BOARD_ID_WHITEBOARD.to_string(), "missing".to_string()];

    state.switch_board_recent();

    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("No recent board to switch to.")
    );
}

#[test]
fn switch_board_updates_open_board_picker_selection_and_clears_hover() {
    let mut state = create_test_input_state();
    state.open_board_picker();

    if let BoardPickerState::Open { hover_index, .. } = &mut state.board_picker_state {
        *hover_index = Some(0);
    }

    state.switch_board("blackboard");

    assert_eq!(state.board_id(), "blackboard");
    assert_eq!(
        state.board_picker_selected_index(),
        state.board_picker_row_for_board(state.boards.active_index())
    );
    match &state.board_picker_state {
        BoardPickerState::Open { hover_index, .. } => assert!(hover_index.is_none()),
        BoardPickerState::Hidden => panic!("board picker should remain open"),
    }
}

#[test]
fn duplicate_board_from_transparent_shows_info_toast_without_creating_board() {
    let mut state = create_test_input_state();
    let initial_count = state.boards.board_count();
    assert_eq!(state.board_id(), BOARD_ID_TRANSPARENT);

    state.duplicate_board();

    assert_eq!(state.boards.board_count(), initial_count);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Overlay board cannot be duplicated.")
    );
}

#[test]
fn create_board_adds_board_queues_config_save_and_emits_toast() {
    let mut state = create_test_input_state();
    let initial_count = state.boards.board_count();

    assert!(state.create_board());

    assert_eq!(state.boards.board_count(), initial_count + 1);
    assert!(state.take_pending_board_config().is_some());
    assert!(state
        .ui_toast
        .as_ref()
        .is_some_and(|toast| toast.message.starts_with("Board created:")));
}
