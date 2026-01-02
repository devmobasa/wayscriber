use super::*;

#[test]
fn edit_selected_text_commit_updates_and_undo() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 100,
        y: 100,
        text: "Hello".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    });

    state.set_selection(vec![shape_id]);
    assert!(state.edit_selected_text());

    if let DrawingState::TextInput { buffer, .. } = &mut state.state {
        buffer.push_str(" world");
    } else {
        panic!("Expected text input state");
    }

    state.on_key_press(Key::Return);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_edit_target.is_none());

    let frame = state.boards.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::Text { text, .. } => assert_eq!(text, "Hello world"),
        _ => panic!("Expected text shape"),
    }
    assert_eq!(frame.undo_stack_len(), 1);

    if let Some(action) = state.boards.active_frame_mut().undo_last() {
        state.apply_action_side_effects(&action);
    }

    let frame = state.boards.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::Text { text, .. } => assert_eq!(text, "Hello"),
        _ => panic!("Expected text shape"),
    }
}

#[test]
fn edit_selected_text_cancel_restores_original() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 40,
        y: 80,
        text: "Original".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    });

    state.set_selection(vec![shape_id]);
    assert!(state.edit_selected_text());

    if let DrawingState::TextInput { buffer, .. } = &mut state.state {
        buffer.push_str(" edit");
    } else {
        panic!("Expected text input state");
    }

    state.cancel_text_input();
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_edit_target.is_none());

    let frame = state.boards.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::Text { text, .. } => assert_eq!(text, "Original"),
        _ => panic!("Expected text shape"),
    }
    assert_eq!(frame.undo_stack_len(), 0);
}

#[test]
fn edit_selected_sticky_note_commit_updates_and_undo() {
    let mut state = create_test_input_state();
    let background = Color {
        r: 0.9,
        g: 0.8,
        b: 0.2,
        a: 1.0,
    };
    let shape_id = state
        .boards
        .active_frame_mut()
        .add_shape(Shape::StickyNote {
            x: 100,
            y: 100,
            text: "Note".to_string(),
            background,
            size: state.current_font_size,
            font_descriptor: state.font_descriptor.clone(),
            wrap_width: None,
        });

    state.set_selection(vec![shape_id]);
    assert!(state.edit_selected_text());

    if let DrawingState::TextInput { buffer, .. } = &mut state.state {
        buffer.push_str(" updated");
    } else {
        panic!("Expected text input state");
    }

    state.on_key_press(Key::Return);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_edit_target.is_none());

    let frame = state.boards.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::StickyNote {
            text,
            background: bg,
            ..
        } => {
            assert_eq!(text, "Note updated");
            assert_eq!(*bg, background);
        }
        _ => panic!("Expected sticky note shape"),
    }
    assert_eq!(frame.undo_stack_len(), 1);

    if let Some(action) = state.boards.active_frame_mut().undo_last() {
        state.apply_action_side_effects(&action);
    }

    let frame = state.boards.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::StickyNote {
            text,
            background: bg,
            ..
        } => {
            assert_eq!(text, "Note");
            assert_eq!(*bg, background);
        }
        _ => panic!("Expected sticky note shape"),
    }
}

#[test]
fn edit_selected_sticky_note_cancel_restores_original() {
    let mut state = create_test_input_state();
    let background = Color {
        r: 0.3,
        g: 0.4,
        b: 0.7,
        a: 1.0,
    };
    let shape_id = state
        .boards
        .active_frame_mut()
        .add_shape(Shape::StickyNote {
            x: 40,
            y: 80,
            text: "Original".to_string(),
            background,
            size: state.current_font_size,
            font_descriptor: state.font_descriptor.clone(),
            wrap_width: None,
        });

    state.set_selection(vec![shape_id]);
    assert!(state.edit_selected_text());

    if let DrawingState::TextInput { buffer, .. } = &mut state.state {
        buffer.push_str(" edit");
    } else {
        panic!("Expected text input state");
    }

    state.cancel_text_input();
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_edit_target.is_none());

    let frame = state.boards.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::StickyNote {
            text,
            background: bg,
            ..
        } => {
            assert_eq!(text, "Original");
            assert_eq!(*bg, background);
        }
        _ => panic!("Expected sticky note shape"),
    }
    assert_eq!(frame.undo_stack_len(), 0);
}
