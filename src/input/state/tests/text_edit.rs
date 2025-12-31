use super::*;

#[test]
fn edit_selected_text_commit_updates_and_undo() {
    let mut state = create_test_input_state();
    let shape_id = state.canvas_set.active_frame_mut().add_shape(Shape::Text {
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

    let frame = state.canvas_set.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::Text { text, .. } => assert_eq!(text, "Hello world"),
        _ => panic!("Expected text shape"),
    }
    assert_eq!(frame.undo_stack_len(), 1);

    if let Some(action) = state.canvas_set.active_frame_mut().undo_last() {
        state.apply_action_side_effects(&action);
    }

    let frame = state.canvas_set.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::Text { text, .. } => assert_eq!(text, "Hello"),
        _ => panic!("Expected text shape"),
    }
}

#[test]
fn edit_selected_text_cancel_restores_original() {
    let mut state = create_test_input_state();
    let shape_id = state.canvas_set.active_frame_mut().add_shape(Shape::Text {
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

    let frame = state.canvas_set.active_frame();
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
        .canvas_set
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

    let frame = state.canvas_set.active_frame();
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

    if let Some(action) = state.canvas_set.active_frame_mut().undo_last() {
        state.apply_action_side_effects(&action);
    }

    let frame = state.canvas_set.active_frame();
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
        .canvas_set
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

    let frame = state.canvas_set.active_frame();
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

#[test]
fn double_click_edit_enters_text_input() {
    let mut state = create_test_input_state();
    let shape_id = state.canvas_set.active_frame_mut().add_shape(Shape::Text {
        x: 120,
        y: 120,
        text: "Hello".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    });

    let bounds = state
        .canvas_set
        .active_frame()
        .shape(shape_id)
        .unwrap()
        .shape
        .bounding_box()
        .expect("text bounds");
    let click_x = bounds.x + 1;
    let click_y = bounds.y + 1;

    state.on_mouse_press(MouseButton::Left, click_x, click_y);
    state.on_mouse_release(MouseButton::Left, click_x, click_y);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_edit_target.is_none());

    state.on_mouse_press(MouseButton::Left, click_x, click_y);
    state.on_mouse_release(MouseButton::Left, click_x, click_y);

    match &state.state {
        DrawingState::TextInput { buffer, .. } => assert_eq!(buffer, "Hello"),
        _ => panic!("Expected text input state"),
    }
    assert!(state.text_edit_target.is_some());

    let frame = state.canvas_set.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::Text { text, .. } => assert!(text.is_empty()),
        _ => panic!("Expected text shape"),
    }
}

#[test]
fn right_click_clears_double_click_tracking() {
    let mut state = create_test_input_state();
    let shape_id = state.canvas_set.active_frame_mut().add_shape(Shape::Text {
        x: 80,
        y: 80,
        text: "Hello".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    });

    let bounds = state
        .canvas_set
        .active_frame()
        .shape(shape_id)
        .unwrap()
        .shape
        .bounding_box()
        .expect("text bounds");
    let click_x = bounds.x + 1;
    let click_y = bounds.y + 1;

    state.on_mouse_press(MouseButton::Left, click_x, click_y);
    state.on_mouse_release(MouseButton::Left, click_x, click_y);
    assert!(state.last_text_click.is_some());

    state.set_context_menu_enabled(false);
    state.on_mouse_press(MouseButton::Right, click_x, click_y);
    assert!(state.last_text_click.is_none());

    state.on_mouse_press(MouseButton::Left, click_x, click_y);
    state.on_mouse_release(MouseButton::Left, click_x, click_y);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_edit_target.is_none());

    let frame = state.canvas_set.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::Text { text, .. } => assert_eq!(text, "Hello"),
        _ => panic!("Expected text shape"),
    }
}

#[test]
fn dragging_text_resize_handle_updates_wrap_width_within_screen() {
    let mut state = create_test_input_state();
    state.update_screen_dimensions(300, 200);
    let shape_id = state.canvas_set.active_frame_mut().add_shape(Shape::Text {
        x: 250,
        y: 120,
        text: "Hello".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    });

    state.set_selection(vec![shape_id]);
    let (_, handle) = state
        .selected_text_resize_handle()
        .expect("expected resize handle");
    let handle_x = handle.x + handle.width / 2;
    let handle_y = handle.y + handle.height / 2;

    state.on_mouse_press(MouseButton::Left, handle_x, handle_y);
    let drag_x = 1000;
    state.on_mouse_motion(drag_x, handle_y);
    state.on_mouse_release(MouseButton::Left, drag_x, handle_y);
    assert!(matches!(state.state, DrawingState::Idle));

    let frame = state.canvas_set.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::Text { wrap_width, .. } => assert_eq!(*wrap_width, Some(50)),
        _ => panic!("Expected text shape"),
    }
}

#[test]
fn enter_key_starts_edit_for_selected_sticky_note() {
    let mut state = create_test_input_state();
    let shape_id = state
        .canvas_set
        .active_frame_mut()
        .add_shape(Shape::StickyNote {
            x: 120,
            y: 120,
            text: "Note".to_string(),
            background: state.current_color,
            size: state.current_font_size,
            font_descriptor: state.font_descriptor.clone(),
            wrap_width: None,
        });

    state.set_selection(vec![shape_id]);
    state.on_key_press(Key::Return);

    match &state.state {
        DrawingState::TextInput { buffer, .. } => assert_eq!(buffer, "Note"),
        _ => panic!("Expected text input state"),
    }
    assert!(matches!(state.text_input_mode, TextInputMode::StickyNote));
}

#[test]
fn enter_key_starts_edit_for_selected_text() {
    let mut state = create_test_input_state();
    let shape_id = state.canvas_set.active_frame_mut().add_shape(Shape::Text {
        x: 60,
        y: 70,
        text: "Hello".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    });

    state.set_selection(vec![shape_id]);
    state.on_key_press(Key::Return);

    match &state.state {
        DrawingState::TextInput { buffer, .. } => assert_eq!(buffer, "Hello"),
        _ => panic!("Expected text input state"),
    }
    assert!(matches!(state.text_input_mode, TextInputMode::Plain));
    let edit_id = state.text_edit_target.as_ref().map(|(id, _)| *id);
    assert_eq!(edit_id, Some(shape_id));
}
