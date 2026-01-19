use super::*;

#[test]
fn double_click_edit_enters_text_input() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
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
        .boards
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

    let frame = state.boards.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::Text { text, .. } => assert!(text.is_empty()),
        _ => panic!("Expected text shape"),
    }
}

#[test]
fn right_click_clears_double_click_tracking() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
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
        .boards
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

    let frame = state.boards.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::Text { text, .. } => assert_eq!(text, "Hello"),
        _ => panic!("Expected text shape"),
    }
}

#[test]
fn enter_key_starts_edit_for_selected_sticky_note() {
    let mut state = create_test_input_state();
    let shape_id = state
        .boards
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
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
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
