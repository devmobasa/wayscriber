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
