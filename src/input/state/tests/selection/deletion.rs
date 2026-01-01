use super::*;

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
