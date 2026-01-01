use super::super::*;

#[test]
fn test_text_mode_plain_letters_not_triggering_actions() {
    let mut state = create_test_input_state();

    // Enter text mode
    state.state = DrawingState::TextInput {
        x: 100,
        y: 100,
        buffer: String::new(),
    };

    // Type 'r' - should add to buffer, not change color
    let original_color = state.current_color;
    state.on_key_press(Key::Char('r'));

    // Check that 'r' was added to buffer
    if let DrawingState::TextInput { buffer, .. } = &state.state {
        assert_eq!(buffer, "r");
    } else {
        panic!("Should still be in text input mode");
    }

    // Color should NOT have changed
    assert_eq!(state.current_color, original_color);

    // Type more color keys
    state.on_key_press(Key::Char('g'));
    state.on_key_press(Key::Char('b'));
    state.on_key_press(Key::Char('t'));

    if let DrawingState::TextInput { buffer, .. } = &state.state {
        assert_eq!(buffer, "rgbt");
    } else {
        panic!("Should still be in text input mode");
    }

    // Color should still not have changed
    assert_eq!(state.current_color, original_color);
}

#[test]
fn test_text_mode_allows_symbol_keys_without_modifiers() {
    let mut state = create_test_input_state();

    state.state = DrawingState::TextInput {
        x: 0,
        y: 0,
        buffer: String::new(),
    };

    for key in ['-', '+', '=', '_', '!', '@', '#', '$'] {
        state.on_key_press(Key::Char(key));
    }

    if let DrawingState::TextInput { buffer, .. } = &state.state {
        assert_eq!(buffer, "-+=_!@#$");
    } else {
        panic!("Expected to remain in text input mode");
    }
}

#[test]
fn test_text_mode_ctrl_keys_trigger_actions() {
    let mut state = create_test_input_state();

    // Enter text mode
    state.state = DrawingState::TextInput {
        x: 100,
        y: 100,
        buffer: String::from("test"),
    };

    // Press Ctrl (modifier)
    state.on_key_press(Key::Ctrl);

    // Verify Ctrl is held
    assert!(state.modifiers.ctrl);

    // Press 'Z' while Ctrl is held (Ctrl+Z should undo - a non-Exit action)
    state.on_key_press(Key::Char('Z'));

    // Should still be in text mode (undo works but doesn't exit text mode)
    assert!(matches!(state.state, DrawingState::TextInput { .. }));

    // Now test Ctrl+Q for exit
    state.on_key_press(Key::Char('Q'));

    // Exit action from text mode goes to Idle (cancels text mode)
    assert!(matches!(state.state, DrawingState::Idle));

    // Now that we're in Idle, pressing Ctrl+Q again should exit the app
    state.on_key_press(Key::Char('Q'));
    assert!(state.should_exit);
}

#[test]
fn test_text_mode_respects_length_cap() {
    let mut state = create_test_input_state();

    state.state = DrawingState::TextInput {
        x: 0,
        y: 0,
        buffer: "a".repeat(10_000),
    };

    state.on_key_press(Key::Char('b'));

    if let DrawingState::TextInput { buffer, .. } = &state.state {
        assert_eq!(buffer.len(), 10_000);
        assert!(buffer.ends_with('a'));
    } else {
        panic!("Expected to remain in text input mode");
    }

    // After trimming, adding should work again
    if let DrawingState::TextInput { buffer, .. } = &mut state.state {
        buffer.truncate(9_999);
    }

    state.on_key_press(Key::Char('c'));

    if let DrawingState::TextInput { buffer, .. } = &state.state {
        assert!(buffer.ends_with('c'));
        assert_eq!(buffer.len(), 10_000);
    }
}

#[test]
fn test_text_mode_escape_exits() {
    let mut state = create_test_input_state();

    // Enter text mode
    state.state = DrawingState::TextInput {
        x: 100,
        y: 100,
        buffer: String::from("test"),
    };

    // Press Escape (should cancel text input)
    state.on_key_press(Key::Escape);

    // Should have exited text mode without adding text
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(!state.should_exit); // Just cancel, don't exit app
}

#[test]
fn test_text_mode_f10_shows_help() {
    let mut state = create_test_input_state();

    // Enter text mode
    state.state = DrawingState::TextInput {
        x: 100,
        y: 100,
        buffer: String::new(),
    };

    assert!(!state.show_help);

    // Press F10 (should toggle help even in text mode)
    state.on_key_press(Key::F10);

    // Help should be visible
    assert!(state.show_help);

    // Should still be in text mode
    assert!(matches!(state.state, DrawingState::TextInput { .. }));
}
