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
