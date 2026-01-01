use super::super::*;

#[test]
fn test_idle_mode_plain_letters_trigger_color_actions() {
    let mut state = create_test_input_state();

    // Should be in Idle mode
    assert!(matches!(state.state, DrawingState::Idle));

    let original_color = state.current_color;

    // Press 'g' for green
    state.on_key_press(Key::Char('g'));

    // Color should have changed
    assert_ne!(state.current_color, original_color);
    assert_eq!(state.current_color, util::key_to_color('g').unwrap());
}
