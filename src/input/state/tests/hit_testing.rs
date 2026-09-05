use super::*;

fn add_test_line(state: &mut InputState) -> crate::draw::ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Line {
        x1: 0,
        y1: 0,
        x2: 20,
        y2: 0,
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 2.0,
    })
}

fn indexed_state_with_two_overlapping_rects() -> InputState {
    let mut state = create_test_input_state();
    state.set_hit_test_threshold(1);
    let color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    for _ in 0..2 {
        state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 20,
            h: 20,
            fill: true,
            color,
            thick: 1.0,
        });
    }
    state
}

#[test]
fn explicit_hit_testing_rejects_invalid_tolerances() {
    let measurer = crate::draw::TextMeasurer::default();
    let mut state = create_test_input_state();
    add_test_line(&mut state);

    for tolerance in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, f64::MAX] {
        assert!(
            state
                .hit_test_all_for_points_with(&measurer, &[(10, 0)], tolerance)
                .is_empty(),
            "invalid tolerance {tolerance:?} must fail closed"
        );
    }
}

#[test]
fn stored_hit_test_tolerance_is_always_valid() {
    let mut state = create_test_input_state();

    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, f64::MAX] {
        state.set_hit_test_tolerance(invalid);
        assert_eq!(state.hit_test_tolerance(), 1.0);
    }

    state.set_hit_test_tolerance(25.0);
    assert_eq!(state.hit_test_tolerance(), 25.0);
}

#[test]
fn extreme_persisted_rectangle_is_selectable_through_the_spatial_index() {
    let measurer = crate::draw::TextMeasurer::default();
    let mut state = create_test_input_state();
    state.set_hit_test_threshold(1);
    let color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let extreme_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: i32::MIN,
        y: 0,
        w: i32::MIN,
        h: 20,
        fill: true,
        color,
        thick: 1.0,
    });
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 100,
        y: 100,
        w: 20,
        h: 20,
        fill: true,
        color,
        thick: 1.0,
    });

    assert_eq!(state.hit_test_at(i32::MIN, 10), Some(extreme_id));
    assert!(state.has_spatial_index());
    assert_eq!(
        state.hit_test_all_for_points_with(&measurer, &[(i32::MIN, 10)], 1.0),
        vec![extreme_id]
    );
}

#[test]
fn spatial_hit_testing_reuses_resolved_candidate_indices_without_id_rescans() {
    let mut state = create_test_input_state();
    state.set_hit_test_threshold(1);
    let color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 20,
        h: 20,
        fill: true,
        color,
        thick: 1.0,
    });
    let topmost = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 20,
        h: 20,
        fill: true,
        color,
        thick: 1.0,
    });

    crate::draw::Frame::reset_linear_id_lookup_count();
    assert_eq!(state.hit_test_at(10, 10), Some(topmost));
    assert!(state.has_spatial_index());
    assert_eq!(
        crate::draw::Frame::linear_id_lookup_count(),
        0,
        "spatial candidates already carry resolved frame indices"
    );
}

#[test]
fn unchanged_spatial_hit_tests_build_shape_indices_once() {
    let mut state = indexed_state_with_two_overlapping_rects();

    InputState::reset_spatial_shape_index_build_count();
    for _ in 0..100 {
        assert!(state.hit_test_at(10, 10).is_some());
    }

    assert_eq!(InputState::spatial_shape_index_build_count(), 1);
}

#[test]
fn unchanged_spatial_multi_point_hit_tests_build_shape_indices_once() {
    let measurer = crate::draw::TextMeasurer::default();
    let mut state = indexed_state_with_two_overlapping_rects();

    InputState::reset_spatial_shape_index_build_count();
    for _ in 0..100 {
        assert_eq!(
            state
                .hit_test_all_for_points_with(&measurer, &[(0, 10)], 1.0)
                .len(),
            2
        );
    }

    assert_eq!(InputState::spatial_shape_index_build_count(), 1);
}

#[test]
fn spatial_hit_testing_falls_back_when_public_shape_storage_bypasses_generation() {
    let mut state = indexed_state_with_two_overlapping_rects();
    let bottom = state.boards.active_frame().shapes[0].id;
    let top = state.boards.active_frame().shapes[1].id;

    InputState::reset_spatial_shape_index_build_count();
    assert_eq!(state.hit_test_at(10, 10), Some(top));

    state.boards.active_frame_mut().shapes.swap(0, 1);

    assert_eq!(state.hit_test_at(10, 10), Some(bottom));
    assert_eq!(InputState::spatial_shape_index_build_count(), 1);
}

#[test]
fn spatial_hit_testing_rebuilds_after_public_shape_id_replacement() {
    let mut state = indexed_state_with_two_overlapping_rects();
    let bottom = state.boards.active_frame().shapes[0].id;
    let old_top = state.boards.active_frame().shapes[1].id;
    state.boards.active_frame_mut().shapes[1].set_shape(Shape::Rect {
        x: 100,
        y: 100,
        w: 20,
        h: 20,
        fill: true,
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 1.0,
    });

    InputState::reset_spatial_shape_index_build_count();
    assert_eq!(state.hit_test_at(10, 10), Some(bottom));

    let replacement = old_top.saturating_add(10_000);
    let top = &mut state.boards.active_frame_mut().shapes[1];
    top.id = replacement;
    top.set_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 20,
        h: 20,
        fill: true,
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 1.0,
    });
    state.invalidate_hit_cache_for(replacement);

    assert_eq!(state.hit_test_at(10, 10), Some(replacement));
    assert_eq!(InputState::spatial_shape_index_build_count(), 2);
}

#[test]
fn spatial_shape_indices_follow_insert_delete_reorder_undo_and_redo() {
    let mut state = create_test_input_state();
    state.set_hit_test_threshold(1);
    let color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let add_rect = |state: &mut InputState| {
        state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 20,
            h: 20,
            fill: true,
            color,
            thick: 1.0,
        })
    };
    let bottom = add_rect(&mut state);
    let middle = add_rect(&mut state);
    let top = add_rect(&mut state);

    InputState::reset_spatial_shape_index_build_count();
    assert_eq!(state.hit_test_at(10, 10), Some(top));

    state
        .boards
        .active_frame_mut()
        .move_shape(2, 0)
        .expect("valid reorder");
    assert_eq!(state.hit_test_at(10, 10), Some(middle));

    state
        .boards
        .active_frame_mut()
        .remove_shape_by_id(middle)
        .expect("middle shape exists");
    assert_eq!(state.hit_test_at(10, 10), Some(bottom));

    let inserted = add_rect(&mut state);
    let inserted_shape = state
        .boards
        .active_frame()
        .shape(inserted)
        .expect("inserted shape exists")
        .clone();
    state.boards.active_frame_mut().push_undo_action(
        UndoAction::Create {
            shapes: vec![(2, inserted_shape)],
        },
        usize::MAX,
    );
    assert_eq!(state.hit_test_at(10, 10), Some(inserted));

    state
        .boards
        .active_frame_mut()
        .undo_last()
        .expect("create can be undone");
    assert_eq!(state.hit_test_at(10, 10), Some(bottom));

    state
        .boards
        .active_frame_mut()
        .redo_last()
        .expect("create can be redone");
    assert_eq!(state.hit_test_at(10, 10), Some(inserted));
    assert_eq!(InputState::spatial_shape_index_build_count(), 6);
}

#[test]
fn spatial_shape_indices_are_scoped_to_the_active_board_and_page() {
    let mut state = create_test_input_state();
    state.set_hit_test_threshold(1);
    add_test_line(&mut state);
    let first_page_top = add_test_line(&mut state);

    InputState::reset_spatial_shape_index_build_count();
    assert!(state.hit_test_at(10, 0).is_some());

    state.page_new();
    add_test_line(&mut state);
    add_test_line(&mut state);
    let second_page_top = add_test_line(&mut state);
    assert_eq!(state.hit_test_at(10, 0), Some(second_page_top));

    assert!(state.page_prev());
    assert_ne!(state.hit_test_at(10, 0), Some(second_page_top));

    state.switch_board_force(crate::input::BOARD_ID_BLACKBOARD);
    add_test_line(&mut state);
    add_test_line(&mut state);
    let blackboard_top = add_test_line(&mut state);
    assert_eq!(state.hit_test_at(10, 0), Some(blackboard_top));

    state.switch_board_force(crate::input::BOARD_ID_TRANSPARENT);
    assert_eq!(state.hit_test_at(10, 0), Some(first_page_top));
    assert_eq!(InputState::spatial_shape_index_build_count(), 5);
}
