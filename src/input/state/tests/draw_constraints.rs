//! Modifier constraints applied to an in-flight drawing drag.
//!
//! The governing rule: a modifier keeps one job per drag. If it selected the
//! tool at press, it does not also constrain the shape, so every existing
//! modifier-drag binding behaves exactly as it did before.

use super::*;
use crate::input::tool::ProvisionalToolStroke;

fn only_shape(state: &InputState) -> &Shape {
    &state.boards.active_frame().shapes[0].shape
}

#[test]
fn shift_squares_an_explicitly_selected_rect_drag() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Rect));

    state.on_mouse_press(MouseButton::Left, 10, 10);
    state.on_mouse_motion(110, 40);
    state.modifiers.shift = true;
    state.on_mouse_release(MouseButton::Left, 110, 40);

    match only_shape(&state) {
        Shape::Rect { x, y, w, h, .. } => {
            assert_eq!((*x, *y), (10, 10));
            assert_eq!(w, h, "shift should square the rect, got {w}x{h}");
            assert_eq!(*w, 100, "square should follow the longer drag axis");
        }
        other => panic!("expected a rect, got {other:?}"),
    }
}

#[test]
fn alt_centers_a_rect_drag_on_its_origin() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Rect));

    state.on_mouse_press(MouseButton::Left, 100, 100);
    state.on_mouse_motion(140, 120);
    state.modifiers.alt = true;
    state.on_mouse_release(MouseButton::Left, 140, 120);

    match only_shape(&state) {
        Shape::Rect { x, y, w, h, .. } => {
            assert_eq!(x + w / 2, 100, "press point should be the rect center x");
            assert_eq!(y + h / 2, 100, "press point should be the rect center y");
            assert_eq!((*w, *h), (80, 40));
        }
        other => panic!("expected a rect, got {other:?}"),
    }
}

#[test]
fn shift_snaps_an_explicitly_selected_line_to_fifteen_degree_steps() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Line));

    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.modifiers.shift = true;
    state.on_mouse_release(MouseButton::Left, 100, 6);

    match only_shape(&state) {
        Shape::Line { x1, y1, x2, y2, .. } => {
            assert_eq!((*x1, *y1), (0, 0));
            assert_eq!(*y2, 0, "a near-horizontal drag should snap flat");
            assert!(*x2 > 90, "length should be preserved, got x2 = {x2}");
        }
        other => panic!("expected a line, got {other:?}"),
    }
}

#[test]
fn shift_snaps_an_explicitly_selected_arrow() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Arrow));

    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.modifiers.shift = true;
    state.on_mouse_release(MouseButton::Left, 6, 100);

    match only_shape(&state) {
        Shape::Arrow { x2, .. } => {
            assert_eq!(*x2, 0, "a near-vertical arrow should snap upright");
        }
        other => panic!("expected an arrow, got {other:?}"),
    }
}

#[test]
fn legacy_shift_drag_line_is_not_also_snapped() {
    // Shift selected the Line tool here, so it keeps that job for the drag.
    let mut state = create_test_input_state();
    state.modifiers.shift = true;

    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_release(MouseButton::Left, 100, 6);

    match only_shape(&state) {
        Shape::Line { x2, y2, .. } => {
            assert_eq!(
                (*x2, *y2),
                (100, 6),
                "a modifier that chose the tool must not also constrain it"
            );
        }
        other => panic!("expected a line, got {other:?}"),
    }
}

#[test]
fn legacy_ctrl_shift_drag_arrow_is_not_snapped() {
    let mut state = create_test_input_state();
    state.modifiers.ctrl = true;
    state.modifiers.shift = true;

    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_release(MouseButton::Left, 100, 6);

    match only_shape(&state) {
        Shape::Arrow { x2, y2, .. } => {
            assert_eq!((*x2, *y2), (100, 6));
        }
        other => panic!("expected an arrow, got {other:?}"),
    }
}

#[test]
fn legacy_ctrl_drag_rect_still_accepts_shift_squaring() {
    // Ctrl chose the tool, so Shift is still free to constrain.
    let mut state = create_test_input_state();
    state.modifiers.ctrl = true;

    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.modifiers.shift = true;
    state.on_mouse_release(MouseButton::Left, 100, 30);

    match only_shape(&state) {
        Shape::Rect { w, h, .. } => {
            assert_eq!(w, h, "shift should still square a ctrl-selected rect");
        }
        other => panic!("expected a rect, got {other:?}"),
    }
}

#[test]
fn legacy_tab_drag_ellipse_still_accepts_shift_circling() {
    let mut state = create_test_input_state();
    state.modifiers.tab = true;

    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.modifiers.shift = true;
    state.on_mouse_release(MouseButton::Left, 60, 20);

    match only_shape(&state) {
        Shape::Ellipse { rx, ry, .. } => {
            assert_eq!(rx, ry, "shift should turn the ellipse into a circle");
        }
        other => panic!("expected an ellipse, got {other:?}"),
    }
}

#[test]
fn preview_matches_the_committed_shape_under_constraints() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Rect));

    state.on_mouse_press(MouseButton::Left, 10, 10);
    state.modifiers.shift = true;
    state.on_mouse_motion(110, 40);

    let previewed = match state.provisional_tool_stroke(110, 40) {
        ProvisionalToolStroke::Shape(Shape::Rect { x, y, w, h, .. }) => (x, y, w, h),
        _ => panic!("expected a rect preview"),
    };

    state.on_mouse_release(MouseButton::Left, 110, 40);
    let committed = match only_shape(&state) {
        Shape::Rect { x, y, w, h, .. } => (*x, *y, *w, *h),
        other => panic!("expected a rect, got {other:?}"),
    };

    assert_eq!(
        previewed, committed,
        "the preview must show exactly what a release commits"
    );
}

#[test]
fn freehand_drags_ignore_constraint_modifiers() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Pen));

    // Alt at press would start a selection drag, so the modifiers go down
    // mid-stroke, exactly as a user would reach for them.
    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.modifiers.shift = true;
    state.modifiers.alt = true;
    state.on_mouse_motion(40, 7);
    state.on_mouse_release(MouseButton::Left, 40, 7);

    match only_shape(&state) {
        Shape::Freehand { points, .. } => {
            assert!(
                points.contains(&(40, 7)),
                "freehand path must keep its raw samples, got {points:?}"
            );
        }
        other => panic!("expected a freehand stroke, got {other:?}"),
    }
}

#[test]
fn the_drag_tool_modifier_latch_clears_after_release() {
    let mut state = create_test_input_state();
    state.modifiers.shift = true;

    state.on_mouse_press(MouseButton::Left, 0, 0);
    assert!(
        !state.active_drag_constraints().proportional,
        "Shift chose the tool, so it must not constrain this drag"
    );

    state.on_mouse_release(MouseButton::Left, 40, 40);
    // With the drag over, Shift is no longer claimed by a tool binding.
    assert!(
        state.active_drag_constraints().proportional,
        "latch should clear so the next drag re-evaluates Shift"
    );
}
