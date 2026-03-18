use super::*;

#[test]
fn duplicate_selection_via_action_creates_offset_shape() {
    let mut state = create_test_input_state();
    let original_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
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

    let frame = state.boards.active_frame();
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
    let original_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
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

    let frame = state.boards.active_frame();
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
    let unlocked_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let locked_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 20,
        y: 20,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    if let Some(index) = state.boards.active_frame().find_index(locked_id) {
        state.boards.active_frame_mut().shapes[index].locked = true;
    }

    state.set_selection(vec![unlocked_id, locked_id]);
    state.handle_action(Action::DuplicateSelection);

    let frame = state.boards.active_frame();
    assert_eq!(frame.shapes.len(), 3, "only one duplicate should be added");
    assert!(
        frame.shape(locked_id).unwrap().locked,
        "locked shape should remain locked"
    );
}

#[test]
fn copy_selection_of_only_locked_shapes_leaves_clipboard_empty() {
    let mut state = create_test_input_state();
    let locked_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 5,
        y: 5,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let locked_index = state
        .boards
        .active_frame()
        .find_index(locked_id)
        .expect("locked index");
    state.boards.active_frame_mut().shapes[locked_index].locked = true;
    state.set_selection(vec![locked_id]);

    assert_eq!(state.copy_selection(), 0);
    assert!(state.selection_clipboard_is_empty());
}

#[test]
fn repeated_paste_selection_uses_increasing_offsets() {
    let mut state = create_test_input_state();
    let original_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 30,
        h: 40,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![original_id]);
    assert_eq!(state.copy_selection(), 1);

    assert_eq!(state.paste_selection(), 1);
    assert_eq!(state.paste_selection(), 1);

    let frame = state.boards.active_frame();
    assert_eq!(frame.shapes.len(), 3);
    let coords = frame
        .shapes
        .iter()
        .map(|shape| match &shape.shape {
            Shape::Rect { x, y, .. } => (*x, *y),
            _ => panic!("expected rectangles"),
        })
        .collect::<Vec<_>>();
    assert_eq!(coords, vec![(10, 20), (22, 32), (34, 44)]);
}

#[test]
fn paste_selection_warns_when_shape_limit_prevents_any_paste() {
    let mut state = create_test_input_state();
    let original_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 20,
        w: 30,
        h: 40,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![original_id]);
    assert_eq!(state.copy_selection(), 1);
    state.max_shapes_per_frame = 1;

    assert_eq!(state.paste_selection(), 0);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Shape limit reached; nothing pasted.")
    );
}

#[test]
fn paste_selection_warns_when_shape_limit_allows_only_partial_paste() {
    let mut state = create_test_input_state();
    let first = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    let second = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 20,
        y: 20,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    state.set_selection(vec![first, second]);
    assert_eq!(state.copy_selection(), 2);
    state.max_shapes_per_frame = 3;

    assert_eq!(state.paste_selection(), 1);
    assert_eq!(state.boards.active_frame().shapes.len(), 3);
    assert_eq!(state.selected_shape_ids().len(), 1);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Shape limit reached; pasted 1 of 2.")
    );
}
