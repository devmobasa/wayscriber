use super::super::*;

#[test]
fn board_mode_toggle_restores_previous_color() {
    let mut state = create_test_input_state();
    let initial_color = state.current_color;
    assert_eq!(state.board_mode(), BoardMode::Transparent);

    state.switch_board_mode(BoardMode::Whiteboard);
    assert_eq!(state.board_mode(), BoardMode::Whiteboard);
    assert_eq!(state.board_previous_color, Some(initial_color));
    let expected_pen = BoardMode::Whiteboard
        .default_pen_color(&state.board_config)
        .expect("whiteboard should have default pen");
    assert_eq!(state.current_color, expected_pen);

    state.switch_board_mode(BoardMode::Whiteboard);
    assert_eq!(state.board_mode(), BoardMode::Transparent);
    assert_eq!(state.current_color, initial_color);
    assert!(state.board_previous_color.is_none());
}
