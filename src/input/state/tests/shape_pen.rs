use super::*;
use crate::input::tool::ProvisionalToolStroke;
use crate::ui::toolbar::{ToolContext, ToolbarSnapshot};

/// A hand-drawn rectangle from the top-left corner, clockwise.
const RECTANGLE: [(i32, i32); 16] = [
    (10, 10),
    (30, 8),
    (55, 11),
    (80, 9),
    (110, 10),
    (112, 30),
    (109, 55),
    (110, 90),
    (85, 92),
    (60, 89),
    (35, 91),
    (10, 90),
    (8, 70),
    (11, 45),
    (9, 25),
    (10, 10),
];

fn shape_pen_state() -> InputState {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::LiveShape)));
    state
}

fn draw_path(state: &mut InputState, path: &[(i32, i32)]) {
    let (first, rest) = path.split_first().expect("path has points");
    state.on_mouse_press(MouseButton::Left, first.0, first.1);
    for &(x, y) in rest {
        state.on_mouse_motion(x, y);
    }
}

fn release_at_end(state: &mut InputState, path: &[(i32, i32)]) {
    let last = path.last().expect("path has points");
    state.on_mouse_release(MouseButton::Left, last.0, last.1);
}

#[test]
fn recognized_closed_shapes_follow_the_fill_toggle() {
    for fill_enabled in [false, true] {
        let mut state = shape_pen_state();
        state.style.fill_enabled = fill_enabled;

        draw_path(&mut state, &RECTANGLE);
        assert!(matches!(
            state.provisional_tool_stroke(10, 10),
            ProvisionalToolStroke::Shape(Shape::Rect { fill, .. }) if fill == fill_enabled
        ));
        release_at_end(&mut state, &RECTANGLE);

        assert!(
            matches!(
                state.boards.active_frame().shapes[0].shape,
                Shape::Rect { fill, .. } if fill == fill_enabled
            ),
            "fill {fill_enabled}"
        );
    }
}

#[test]
fn shape_pen_style_pill_offers_fill_and_smoothing() {
    let state = shape_pen_state();

    let context = ToolContext::from_snapshot(&ToolbarSnapshot::from_input(&state));

    assert!(context.show_fill_toggle);
    assert!(context.show_pen_smoothing);
    assert_eq!(context.thickness_label, "Thickness");
}
