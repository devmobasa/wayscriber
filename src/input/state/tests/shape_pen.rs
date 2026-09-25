use super::*;
use crate::input::tool::ProvisionalToolStroke;
use crate::ui::toolbar::{ToolContext, ToolbarEvent, ToolbarSnapshot, model};
use crate::ui::{ShapeExtent, ShapeReadout};

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

fn run_action(state: &mut InputState, action: Action) {
    let measurer = crate::draw::TextMeasurer::default();
    let ui_engine = crate::ui_text::UiTextEngine::default();
    state.handle_action_with_resources(
        crate::input::state::InputTextResources {
            measurer: &measurer,
            ui_engine: &ui_engine,
        },
        action,
    );
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

#[test]
fn readout_names_the_recognized_shape_and_stays_quiet_for_ink() {
    let mut state = shape_pen_state();

    // Two sides of the rectangle are neither a line nor a closed shape yet.
    draw_path(&mut state, &RECTANGLE[..8]);
    assert_eq!(state.provisional_shape_readout(110, 90), None);

    for &(x, y) in &RECTANGLE[8..] {
        state.on_mouse_motion(x, y);
    }
    assert_eq!(
        state.provisional_shape_readout(10, 10),
        Some(ShapeReadout {
            kind: Some("Rectangle"),
            extent: ShapeExtent::Size(104, 84),
        })
    );
    release_at_end(&mut state, &RECTANGLE);

    draw_path(&mut state, &[(0, 200), (40, 201), (80, 199), (120, 200)]);
    assert_eq!(
        state.provisional_shape_readout(120, 200),
        Some(ShapeReadout {
            kind: Some("Line"),
            extent: ShapeExtent::Length(120),
        })
    );
}

#[test]
fn sensitivity_steps_at_runtime_and_announces_the_level() {
    let mut state = shape_pen_state();
    let _ = state.apply_toolbar_event(ToolbarEvent::SetShapeRecognitionSensitivity(2));

    run_action(&mut state, Action::IncreaseShapeRecognitionSensitivity);
    assert_eq!(state.style.shape_recognition_sensitivity, 3);
    assert_eq!(
        state.active_toast().map(|toast| toast.message.as_str()),
        Some("Shape Pen sensitivity 3/4")
    );

    for _ in 0..3 {
        run_action(&mut state, Action::IncreaseShapeRecognitionSensitivity);
    }
    assert_eq!(state.style.shape_recognition_sensitivity, 4);

    assert!(state.apply_toolbar_event(ToolbarEvent::SetShapeRecognitionSensitivity(0)));
    run_action(&mut state, Action::DecreaseShapeRecognitionSensitivity);
    assert_eq!(state.style.shape_recognition_sensitivity, 0);
}

#[test]
fn toolbar_sensitivity_applies_to_the_next_stroke() {
    let rough_line = [(0, 0), (20, 5), (40, -4), (60, 6), (80, 1)];

    for (level, expected_kind) in [(0, "Freehand"), (4, "Line")] {
        let mut state = shape_pen_state();
        let _ = state.apply_toolbar_event(ToolbarEvent::SetShapeRecognitionSensitivity(level));

        draw_path(&mut state, &rough_line);
        release_at_end(&mut state, &rough_line);

        assert_eq!(
            state.boards.active_frame().shapes[0].shape.kind_name(),
            expected_kind,
            "sensitivity {level}"
        );
    }
}

#[test]
fn full_toolbar_shows_shape_pen_beside_pen_and_simple_keeps_it_in_the_picker() {
    let snapshot = ToolbarSnapshot::from_input(&create_test_input_state());

    let strip: Vec<_> = model::visible_top_tool_buttons(false, &snapshot).collect();
    let pen = strip
        .iter()
        .position(|&tool| tool == Tool::Pen)
        .expect("pen");
    assert_eq!(strip.get(pen + 1), Some(&Tool::LiveShape));
    assert!(
        !model::visible_shape_picker_rows(&snapshot, false)
            .concat()
            .contains(&Tool::LiveShape),
        "the full-mode picker lists only what the strip does not show"
    );

    assert!(!model::visible_top_tool_buttons(true, &snapshot).any(|tool| tool == Tool::LiveShape));
    assert!(
        model::visible_shape_picker_rows(&snapshot, true)
            .concat()
            .contains(&Tool::LiveShape)
    );
}

#[test]
fn first_undo_turns_a_recognized_shape_back_into_its_ink() {
    let mut state = shape_pen_state();
    draw_path(&mut state, &RECTANGLE);
    release_at_end(&mut state, &RECTANGLE);
    let kind = |state: &InputState| {
        let frame = state.boards.active_frame();
        frame.shapes.first().map(|drawn| drawn.shape.kind_name())
    };
    assert_eq!(kind(&state), Some("Rectangle"));

    run_action(&mut state, Action::Undo);
    assert_eq!(kind(&state), Some("Freehand"));
    let Shape::Freehand { points, .. } = &state.boards.active_frame().shapes[0].shape else {
        panic!("the ink comes back as a freehand stroke");
    };
    assert_eq!(points.first(), Some(&RECTANGLE[0]));

    run_action(&mut state, Action::Undo);
    assert_eq!(kind(&state), None);

    run_action(&mut state, Action::Redo);
    assert_eq!(kind(&state), Some("Freehand"));
    run_action(&mut state, Action::Redo);
    assert_eq!(kind(&state), Some("Rectangle"));
}

#[test]
fn ink_that_stays_ink_is_a_single_undo_step() {
    let mut state = shape_pen_state();
    let scribble = [(0, 0), (30, 30), (0, 30), (30, 0)];

    draw_path(&mut state, &scribble);
    release_at_end(&mut state, &scribble);

    assert_eq!(state.boards.active_frame().undo_stack_len(), 1);
}
