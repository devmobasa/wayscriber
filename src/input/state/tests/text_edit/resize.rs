use super::*;

#[test]
fn dragging_text_resize_handle_updates_wrap_width_within_screen() {
    let mut state = create_test_input_state();
    state.update_screen_dimensions(300, 200);
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
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

    let frame = state.boards.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::Text { wrap_width, .. } => assert_eq!(*wrap_width, Some(50)),
        _ => panic!("Expected text shape"),
    }
}
