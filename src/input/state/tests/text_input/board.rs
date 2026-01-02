use super::super::*;
use crate::input::{BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD};

#[test]
fn board_mode_toggle_restores_previous_color() {
    let mut state = create_test_input_state();
    let initial_color = state.current_color;
    assert_eq!(state.board_id(), BOARD_ID_TRANSPARENT);

    state.switch_board(BOARD_ID_WHITEBOARD);
    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);
    assert_eq!(state.board_previous_color, Some(initial_color));
    let expected_pen = state
        .boards
        .active_board()
        .spec
        .effective_pen_color()
        .expect("whiteboard should have default pen");
    assert_eq!(state.current_color, expected_pen);

    state.switch_board(BOARD_ID_WHITEBOARD);
    assert!(state.board_is_transparent());
    assert_eq!(state.current_color, initial_color);
    assert!(state.board_previous_color.is_none());
}
