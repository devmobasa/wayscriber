use super::*;

#[test]
fn translate_selection_with_undo_moves_shape() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 50,
        y2: 50,
        color: state.style.current_color,
        thick: state.style.current_thickness,
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
fn resizing_selection_marks_previous_live_bounds_dirty() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 20,
        h: 20,
        fill: false,
        color: state.style.current_color,
        thick: state.style.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    let original_bounds = state
        .selection_bounds()
        .expect("selection should have bounds");
    let snapshots = state.capture_resize_selection_snapshots();

    state.apply_selection_resize(
        SelectionHandle::BottomRight,
        &original_bounds,
        80,
        80,
        &snapshots,
    );
    let expanded_bounds = state.selection_bounds().expect("selection should resize");
    let _ = state.take_dirty_regions();

    state.apply_selection_resize(
        SelectionHandle::BottomRight,
        &original_bounds,
        10,
        10,
        &snapshots,
    );
    let current_bounds = state.selection_bounds().expect("selection should resize");
    assert!(
        expanded_bounds.x + expanded_bounds.width > current_bounds.x + current_bounds.width,
        "test setup should resize inward after an expanded live resize"
    );

    let dirty = state.take_dirty_regions();
    let expanded_bottom_right = (
        expanded_bounds.x + expanded_bounds.width - 1,
        expanded_bounds.y + expanded_bounds.height - 1,
    );
    assert!(
        dirty
            .iter()
            .any(|rect| rect.contains(expanded_bottom_right.0, expanded_bottom_right.1)),
        "dirty regions should include the previous live resize bounds; dirty={dirty:?}, previous={expanded_bounds:?}"
    );
}

#[test]
fn resizing_selection_back_to_start_restores_original_geometry() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 10,
        y: 10,
        w: 20,
        h: 20,
        fill: false,
        color: state.style.current_color,
        thick: state.style.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    let original_bounds = state
        .selection_bounds()
        .expect("selection should have bounds");
    let snapshots = state.capture_resize_selection_snapshots();

    state.apply_selection_resize(
        SelectionHandle::BottomRight,
        &original_bounds,
        80,
        80,
        &snapshots,
    );
    let expanded_bounds = state.selection_bounds().expect("selection should resize");
    let _ = state.take_dirty_regions();

    state.apply_selection_resize(
        SelectionHandle::BottomRight,
        &original_bounds,
        0,
        0,
        &snapshots,
    );

    {
        let frame = state.boards.active_frame();
        let shape = frame.shape(shape_id).expect("shape should exist");
        match &shape.shape {
            Shape::Rect { x, y, w, h, .. } => assert_eq!((*x, *y, *w, *h), (10, 10, 20, 20)),
            _ => panic!("Expected rect shape"),
        }
    }

    assert_eq!(state.selection_bounds(), Some(original_bounds));
    let dirty = state.take_dirty_regions();
    let expanded_bottom_right = (
        expanded_bounds.x + expanded_bounds.width - 1,
        expanded_bounds.y + expanded_bounds.height - 1,
    );
    assert!(
        dirty
            .iter()
            .any(|rect| rect.contains(expanded_bottom_right.0, expanded_bottom_right.1)),
        "dirty regions should include the previous expanded bounds; dirty={dirty:?}, previous={expanded_bounds:?}"
    );
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
        color: state.style.current_color,
        thick: state.style.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    state.handle_action(Action::MoveSelectionToStart);

    {
        let frame = state.boards.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        let bounds = shape.bounding_box().expect("rect should have bounds");
        assert_eq!(bounds.x, 0);
    }

    state.handle_action(Action::MoveSelectionToEnd);

    {
        let frame = state.boards.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        let bounds = shape.bounding_box().expect("rect should have bounds");
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
        color: state.style.current_color,
        thick: state.style.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    state.handle_action(Action::NudgeSelectionUp);
    state.handle_action(Action::MoveSelectionToStart);

    {
        let frame = state.boards.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        let bounds = shape.bounding_box().expect("rect should have bounds");
        assert_eq!(bounds.x, 0);
    }

    state.handle_action(Action::MoveSelectionToEnd);

    {
        let frame = state.boards.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        let bounds = shape.bounding_box().expect("rect should have bounds");
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
        color: state.style.current_color,
        thick: state.style.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    state.handle_action(Action::MoveSelectionToTop);

    {
        let frame = state.boards.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        let bounds = shape.bounding_box().expect("rect should have bounds");
        assert_eq!(bounds.y, 0);
    }

    state.handle_action(Action::MoveSelectionToBottom);

    {
        let frame = state.boards.active_frame();
        let shape = frame.shape(shape_id).unwrap();
        let bounds = shape.bounding_box().expect("rect should have bounds");
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
        color: state.style.current_color,
        thick: state.style.current_thickness,
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
        color: state.style.current_color,
        thick: state.style.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    state.handle_action(Action::NudgeSelectionLeft);
    state.handle_action(Action::NudgeSelectionUp);

    let frame = state.boards.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    let bounds = shape.bounding_box().expect("rect should have bounds");
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
        color: state.style.current_color,
        thick: state.style.current_thickness,
    });

    state.set_selection(vec![shape_id]);
    state.handle_action(Action::NudgeSelectionRight);
    state.handle_action(Action::NudgeSelectionDown);

    let frame = state.boards.active_frame();
    let shape = frame.shape(shape_id).unwrap();
    let bounds = shape.bounding_box().expect("rect should have bounds");
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
        color: state.style.current_color,
        size: state.style.current_font_size,
        font_descriptor: state.style.font_descriptor.clone(),
        background_enabled: state.style.text_background_enabled,
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

#[test]
fn resizing_a_curved_arrow_keeps_its_style_and_curvature() {
    // `scale_shape` rebuilds `Shape::Arrow` field by field, so a field left out
    // there silently resets on every resize. `style` has to survive untouched;
    // `bend` has to survive as an *arc*, which a non-uniform scale means is not
    // the same as surviving as a number.
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Arrow {
        x1: 0,
        y1: 0,
        x2: 100,
        y2: 0,
        color: state.style.current_color,
        thick: 4.0,
        arrow_length: 20.0,
        arrow_angle: 30.0,
        head_at_end: true,
        style: crate::draw::ArrowStyle::Curved,
        bend: 0.4,
        label: None,
    });

    state.set_selection(vec![shape_id]);
    let original_bounds = state
        .selection_bounds()
        .expect("selection should have bounds");
    let snapshots = state.capture_resize_selection_snapshots();

    state.apply_selection_resize(
        SelectionHandle::BottomRight,
        &original_bounds,
        100,
        60,
        &snapshots,
    );

    let frame = state.boards.active_frame();
    match &frame.shape(shape_id).expect("arrow").shape {
        Shape::Arrow {
            style,
            bend,
            x1,
            x2,
            ..
        } => {
            assert_eq!(
                *style,
                crate::draw::ArrowStyle::Curved,
                "resize reset the style"
            );
            assert!(*bend > 0.0, "resize reset the bend");
            assert!(x2 - x1 > 100, "test setup should have widened the arrow");
        }
        other => panic!("expected arrow, got {other:?}"),
    }
}

#[test]
fn stretching_a_flat_curved_arrow_downward_grows_its_arc() {
    // A horizontal curved arrow's height is almost entirely its arc. Dragging
    // the bottom handle does not lengthen the chord, so a bend copied through
    // unchanged keeps exactly the bulge it had and the selection refuses to
    // follow the pointer — the arrow is the one shape a vertical resize cannot
    // move. Scaling the arc itself is what makes the handle mean something.
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Arrow {
        x1: 0,
        y1: 200,
        x2: 300,
        y2: 200,
        color: state.style.current_color,
        thick: 4.0,
        arrow_length: 20.0,
        arrow_angle: 30.0,
        head_at_end: true,
        style: crate::draw::ArrowStyle::Curved,
        bend: 0.3,
        label: None,
    });

    state.set_selection(vec![shape_id]);
    let original_bounds = state
        .selection_bounds()
        .expect("selection should have bounds");
    let snapshots = state.capture_resize_selection_snapshots();

    state.apply_selection_resize(
        SelectionHandle::Bottom,
        &original_bounds,
        0,
        original_bounds.height,
        &snapshots,
    );

    let resized = state
        .selection_bounds()
        .expect("selection should still have bounds");
    // Doubling the drag height should roughly double the box. Exactness is not
    // the claim — the endpoints round to whole pixels and the arrowhead adds a
    // few of its own — but "grew by most of the drag" separates a working
    // handle from one that does nothing.
    assert!(
        resized.height >= original_bounds.height * 2 - 4,
        "vertical resize did not carry the arc: {} -> {}",
        original_bounds.height,
        resized.height
    );

    match &state
        .boards
        .active_frame()
        .shape(shape_id)
        .expect("arrow")
        .shape
    {
        Shape::Arrow { bend, x1, x2, .. } => {
            assert!(
                *bend > 0.3,
                "bend should have grown with the height, got {bend}"
            );
            assert_eq!(
                x2 - x1,
                300,
                "a vertical resize must not have moved the endpoints"
            );
        }
        other => panic!("expected arrow, got {other:?}"),
    }
}
