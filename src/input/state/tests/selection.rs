use super::*;

#[test]
fn duplicate_selection_via_action_creates_offset_shape() {
    let mut state = create_test_input_state();
    let original_id = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![original_id]);
    state.handle_action(Action::DuplicateSelection);

    let frame = state.canvas_set.active_frame();
    assert_eq!(frame.shapes.len(), 2);

    let new_id = frame
        .shapes
        .iter()
        .map(|shape| shape.id)
        .find(|id| *id != original_id)
        .expect("duplicate shape id");
    let original = frame.shape(original_id).unwrap();
    let duplicate = frame.shape(new_id).unwrap();

    match (&original.shape, &duplicate.shape) {
        (Shape::Rect { x: ox, y: oy, .. }, Shape::Rect { x: dx, y: dy, .. }) => {
            assert_eq!(*dx, ox + 12);
            assert_eq!(*dy, oy + 12);
        }
        _ => panic!("Expected rectangles"),
    }
}

#[test]
fn copy_paste_selection_creates_offset_shape() {
    let mut state = create_test_input_state();
    let original_id = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 100,
        h: 80,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![original_id]);
    state.handle_action(Action::CopySelection);
    state.handle_action(Action::PasteSelection);

    let frame = state.canvas_set.active_frame();
    assert_eq!(frame.shapes.len(), 2);

    let new_id = frame
        .shapes
        .iter()
        .map(|shape| shape.id)
        .find(|id| *id != original_id)
        .expect("pasted shape id");
    let original = frame.shape(original_id).unwrap();
    let pasted = frame.shape(new_id).unwrap();

    match (&original.shape, &pasted.shape) {
        (Shape::Rect { x: ox, y: oy, .. }, Shape::Rect { x: px, y: py, .. }) => {
            assert_eq!(*px, ox + 12);
            assert_eq!(*py, oy + 12);
        }
        _ => panic!("Expected rectangles"),
    }
}

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
fn duplicate_selection_skips_locked_shapes() {
    let mut state = create_test_input_state();
    let unlocked_id = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let locked_id = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 20,
        y: 20,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    if let Some(index) = state.canvas_set.active_frame().find_index(locked_id) {
        state.canvas_set.active_frame_mut().shapes[index].locked = true;
    }

    state.set_selection(vec![unlocked_id, locked_id]);
    state.handle_action(Action::DuplicateSelection);

    let frame = state.canvas_set.active_frame();
    assert_eq!(frame.shapes.len(), 3, "only one duplicate should be added");
    assert!(
        frame.shape(locked_id).unwrap().locked,
        "locked shape should remain locked"
    );
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

#[test]
fn delete_shapes_by_ids_ignores_missing_ids() {
    let mut state = create_test_input_state();
    state.canvas_set.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 5,
        y2: 5,
        color: state.current_color,
        thick: state.current_thickness,
    });

    let removed = state.delete_shapes_by_ids(&[9999]);
    assert!(!removed);
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 1);
}

#[test]
fn locked_shape_blocks_edit_and_delete() {
    let mut state = create_test_input_state();
    let shape_id = state.canvas_set.active_frame_mut().add_shape(Shape::Text {
        x: 40,
        y: 50,
        text: "Locked".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    });

    if let Some(index) = state.canvas_set.active_frame().find_index(shape_id) {
        state.canvas_set.active_frame_mut().shapes[index].locked = true;
    }

    state.set_selection(vec![shape_id]);
    assert!(!state.edit_selected_text());
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_edit_target.is_none());

    assert!(!state.delete_selection());
    assert!(state.canvas_set.active_frame().shape(shape_id).is_some());
}

#[test]
fn clear_all_removes_shapes_even_when_marked_frozen() {
    let mut state = create_test_input_state();
    state.canvas_set.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 10,
        y2: 10,
        color: state.current_color,
        thick: state.current_thickness,
    });

    // Simulate frozen flag being on
    state.set_frozen_active(true);
    assert!(state.frozen_active());

    assert!(state.clear_all());
    assert_eq!(state.canvas_set.active_frame().shapes.len(), 0);
    assert!(state.needs_redraw);
}

#[test]
fn clear_all_skips_locked_shapes() {
    let mut state = create_test_input_state();
    let locked_id = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let unlocked_id = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 40,
        y: 40,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    if let Some(index) = state.canvas_set.active_frame().find_index(locked_id) {
        state.canvas_set.active_frame_mut().shapes[index].locked = true;
    }

    assert!(state.clear_all());
    let frame = state.canvas_set.active_frame();
    assert!(frame.shape(unlocked_id).is_none());
    assert!(
        frame
            .shape(locked_id)
            .map(|shape| shape.locked)
            .unwrap_or(false),
        "locked shape should remain after clear_all"
    );
}

#[test]
fn clear_all_returns_false_when_all_locked() {
    let mut state = create_test_input_state();
    let locked_id = state.canvas_set.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 20,
        h: 20,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    if let Some(index) = state.canvas_set.active_frame().find_index(locked_id) {
        state.canvas_set.active_frame_mut().shapes[index].locked = true;
    }

    assert!(!state.clear_all());
    let frame = state.canvas_set.active_frame();
    assert!(frame.shape(locked_id).is_some());
}
