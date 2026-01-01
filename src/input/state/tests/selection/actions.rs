use super::*;

#[test]
fn select_all_action_selects_shapes() {
    let mut state = create_test_input_state();
    let first = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let second = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 30,
        y: 30,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.handle_action(Action::SelectAll);
    let selected = state.selected_shape_ids();
    assert_eq!(selected.len(), 2);
    assert!(selected.contains(&first));
    assert!(selected.contains(&second));
}

#[test]
fn escape_clears_selection_before_exit() {
    let mut state = create_test_input_state();
    let shape_id = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    state.on_key_press(Key::Escape);
    assert!(!state.has_selection());
    assert!(!state.should_exit);
}

#[test]
fn select_tool_drag_selects_shapes_in_rect() {
    let mut state = create_test_input_state();
    let inside = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 12,
        y: 12,
        w: 8,
        h: 8,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let outside = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 80,
        y: 80,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_tool_override(Some(Tool::Select));
    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_motion(40, 40);
    state.on_mouse_release(MouseButton::Left, 40, 40);

    let selected = state.selected_shape_ids();
    assert!(selected.contains(&inside));
    assert!(!selected.contains(&outside));
}
