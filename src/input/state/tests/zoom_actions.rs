use super::*;

#[test]
fn held_zoom_shortcut_modifiers_allow_repeated_zoom_steps() {
    let mut state = create_test_input_state();
    state.on_key_press(Key::Ctrl);
    state.on_key_press(Key::Alt);

    state.on_key_press(Key::Char('+'));
    state.on_key_release(Key::Char('+'));
    assert_eq!(state.take_pending_zoom_action(), Some(ZoomAction::In));

    state.on_key_press(Key::Char('+'));
    state.on_key_release(Key::Char('+'));
    assert_eq!(state.take_pending_zoom_action(), Some(ZoomAction::In));
}

#[test]
fn keyboard_zoom_marks_the_zoom_guidance_as_used() {
    let mut state = create_test_input_state();
    assert!(!state.pending_onboarding_usage.used_zoom_control);

    state.on_key_press(Key::Ctrl);
    state.on_key_press(Key::Alt);
    state.on_key_press(Key::Char('+'));
    state.on_key_release(Key::Char('+'));

    assert_eq!(state.take_pending_zoom_action(), Some(ZoomAction::In));
    assert!(state.pending_onboarding_usage.used_zoom_control);
}
