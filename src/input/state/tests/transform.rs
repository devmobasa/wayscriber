use super::*;

#[test]
fn translate_selection_with_undo_moves_shape() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 50,
        y2: 50,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    assert!(state.translate_selection_with_undo(10, -5));

    {
        let frame = state.boards.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        match &shape.shape {
            Shape::Line { x1, y1, x2, y2, .. } => {
                assert_eq!((*x1, *y1, *x2, *y2), (10, -5, 60, 45));
            }
            _ => panic!("Expected line shape"),
        }
    }

    // Undo and ensure shape returns to original coordinates
    if let Some(action) = state.boards.active_frame_mut().undo_last() {
        state.apply_action_side_effects(&action);
    }

    {
        let frame = state.boards.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        match &shape.shape {
            Shape::Line { x1, y1, x2, y2, .. } => {
                assert_eq!((*x1, *y1, *x2, *y2), (0, 0, 50, 50));
            }
            _ => panic!("Expected line shape"),
        }
    }
}

#[test]
fn move_selection_to_horizontal_edges_uses_screen_bounds() {
    let mut state = create_test_input_state();
    state.update_screen_dimensions(200, 100);
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 50,
        y: 20,
        w: 20,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    state.handle_action(Action::MoveSelectionToStart);

    {
        let frame = state.boards.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        let bounds = shape.shape.bounding_box().expect("rect should have bounds");
        assert_eq!(bounds.x, 0);
    }

    state.handle_action(Action::MoveSelectionToEnd);

    {
        let frame = state.boards.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        let bounds = shape.shape.bounding_box().expect("rect should have bounds");
        assert_eq!(bounds.x + bounds.width, 200);
    }
}

#[test]
fn move_selection_to_horizontal_edges_ignores_last_axis() {
    let mut state = create_test_input_state();
    state.update_screen_dimensions(200, 100);
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 50,
        y: 20,
        w: 20,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    state.handle_action(Action::NudgeSelectionUp);
    state.handle_action(Action::MoveSelectionToStart);

    {
        let frame = state.boards.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        let bounds = shape.shape.bounding_box().expect("rect should have bounds");
        assert_eq!(bounds.x, 0);
    }

    state.handle_action(Action::MoveSelectionToEnd);

    {
        let frame = state.boards.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        let bounds = shape.shape.bounding_box().expect("rect should have bounds");
        assert_eq!(bounds.x + bounds.width, 200);
    }
}

#[test]
fn move_selection_to_vertical_edges_explicit_actions() {
    let mut state = create_test_input_state();
    state.update_screen_dimensions(200, 100);
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 50,
        y: 20,
        w: 20,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    state.handle_action(Action::MoveSelectionToTop);

    {
        let frame = state.boards.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        let bounds = shape.shape.bounding_box().expect("rect should have bounds");
        assert_eq!(bounds.y, 0);
    }

    state.handle_action(Action::MoveSelectionToBottom);

    {
        let frame = state.boards.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        let bounds = shape.shape.bounding_box().expect("rect should have bounds");
        assert_eq!(bounds.y + bounds.height, 100);
    }
}

#[test]
fn nudge_selection_large_uses_large_step() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    state.handle_action(Action::NudgeSelectionDownLarge);

    let frame = state.boards.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::Rect { y, .. } => assert_eq!(*y, 42),
        _ => panic!("Expected rect shape"),
    }
}

#[test]
fn nudge_selection_clamps_left_and_top_edges() {
    let mut state = create_test_input_state();
    state.update_screen_dimensions(100, 100);
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 4,
        y: 3,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    state.handle_action(Action::NudgeSelectionLeft);
    state.handle_action(Action::NudgeSelectionUp);

    let frame = state.boards.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    let bounds = shape.shape.bounding_box().expect("rect should have bounds");
    assert_eq!((bounds.x, bounds.y), (0, 0));
}

#[test]
fn nudge_selection_clamps_right_and_bottom_edges() {
    let mut state = create_test_input_state();
    state.update_screen_dimensions(100, 100);
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 90,
        y: 90,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    state.handle_action(Action::NudgeSelectionRight);
    state.handle_action(Action::NudgeSelectionDown);

    let frame = state.boards.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    let bounds = shape.shape.bounding_box().expect("rect should have bounds");
    assert_eq!(
        (bounds.x + bounds.width, bounds.y + bounds.height),
        (100, 100)
    );
}

#[test]
fn restore_selection_snapshots_reverts_translation() {
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
    let snapshots = state.capture_movable_selection_snapshots();
    assert_eq!(snapshots.len(), 1);

    assert!(state.apply_translation_to_selection(20, 30));
    state.restore_selection_from_snapshots(snapshots);

    let frame = state.boards.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    match &shape.shape {
        Shape::Text { x, y, .. } => {
            assert_eq!((*x, *y), (100, 100));
        }
        _ => panic!("Expected text shape"),
    }
}
