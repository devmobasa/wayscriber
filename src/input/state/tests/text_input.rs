use super::*;

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
fn test_redo_restores_shape_after_undo() {
    let mut state = create_test_input_state();

    {
        let frame = state.canvas_set.active_frame_mut();
        let shape_id = frame.add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 10,
            y2: 10,
            color: state.current_color,
            thick: state.current_thickness,
        });

        let index = frame.find_index(shape_id).unwrap();
        let snapshot = frame.shape(shape_id).unwrap().clone();
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(index, snapshot)],
            },
            state.undo_stack_limit,
        );
    }

    assert_eq!(state.canvas_set.active_frame().shapes.len(), 1);

    state.handle_action(Action::Undo);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 0);

    state.handle_action(Action::Redo);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 1);
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

#[test]
fn capture_action_sets_pending_and_clears_modifiers() {
    let mut state = create_test_input_state();
    state.modifiers.ctrl = true;
    state.modifiers.shift = true;
    state.modifiers.alt = true;

    state.handle_action(Action::CaptureClipboardFull);

    assert!(!state.modifiers.ctrl);
    assert!(!state.modifiers.shift);
    assert!(!state.modifiers.alt);

    assert_eq!(
        state.take_pending_capture_action(),
        Some(Action::CaptureClipboardFull)
    );
    assert!(state.take_pending_capture_action().is_none());
}

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
