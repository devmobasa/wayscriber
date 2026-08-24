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

#[test]
fn explicit_hit_testing_rejects_invalid_tolerances() {
    let mut state = create_test_input_state();
    add_test_line(&mut state);

    for tolerance in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, f64::MAX] {
        assert!(
            state
                .hit_test_all_for_points(&[(10, 0)], tolerance)
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
        assert_eq!(state.hit_test_tolerance, 1.0);
    }

    state.set_hit_test_tolerance(25.0);
    assert_eq!(state.hit_test_tolerance, 25.0);
}

#[test]
fn extreme_persisted_rectangle_is_selectable_through_the_spatial_index() {
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
        state.hit_test_all_for_points(&[(i32::MIN, 10)], 1.0),
        vec![extreme_id]
    );
}
