use super::super::*;

#[test]
fn test_escape_cancels_active_drawing_only() {
    let mut state = create_test_input_state();
    state.state = DrawingState::Drawing {
        tool: Tool::Pen,
        start_x: 0,
        start_y: 0,
        points: vec![(0, 0), (5, 5)],
    };

    state.on_key_press(Key::Escape);

    assert!(matches!(state.state, DrawingState::Idle));
    assert!(!state.should_exit);
}

#[test]
fn test_escape_from_idle_requests_exit() {
    let mut state = create_test_input_state();
    assert!(matches!(state.state, DrawingState::Idle));

    state.on_key_press(Key::Escape);

    assert!(state.should_exit);
}
