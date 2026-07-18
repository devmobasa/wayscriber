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
