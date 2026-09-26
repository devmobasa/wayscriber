use super::*;
use std::time::{Duration, Instant};

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

/// A scribble no fit accepts, so it stays ink.
const SCRIBBLE: [(i32, i32); 8] = [
    (10, 10),
    (60, 40),
    (20, 70),
    (90, 20),
    (40, 100),
    (100, 90),
    (15, 45),
    (70, 5),
];

fn shape_pen_state(state: InputState) -> InputState {
    let mut state = state;
    assert!(state.set_tool_override(Some(Tool::LiveShape)));
    state
}

fn draw(state: &mut InputState, path: &[(i32, i32)]) {
    let (first, rest) = path.split_first().expect("path has points");
    state.on_mouse_press(MouseButton::Left, first.0, first.1);
    for &(x, y) in rest {
        state.on_mouse_motion(x, y);
    }
    let last = path.last().expect("path has points");
    state.on_mouse_release(MouseButton::Left, last.0, last.1);
}

fn chip_label(state: &InputState) -> Option<String> {
    state
        .recognition_chip()
        .map(|chip| chip.label().to_string())
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
fn a_recognized_stroke_names_the_shape_and_the_undo_that_keeps_its_ink() {
    let measurer = crate::draw::TextMeasurer::default();
    let mut state = shape_pen_state(create_test_input_state());
    state.needs_redraw = false;

    draw(&mut state, &RECTANGLE);

    let shape = &state.boards.active_frame().shapes[0].shape;
    assert!(matches!(shape, Shape::Rect { .. }));
    let chip = state
        .recognition_chip()
        .expect("a recognized stroke shows a chip");
    assert_eq!(chip.label(), "Rectangle · Ctrl+Z keeps ink");
    assert_eq!(Some(chip.anchor()), shape.bounding_box_with(&measurer));
    assert!(state.needs_redraw);
}

#[test]
fn a_stroke_that_stays_ink_shows_no_chip() {
    let mut state = shape_pen_state(create_test_input_state());

    draw(&mut state, &SCRIBBLE);

    assert!(matches!(
        state.boards.active_frame().shapes[0].shape,
        Shape::Freehand { .. } | Shape::FreehandPressure { .. }
    ));
    assert_eq!(chip_label(&state), None);
}

#[test]
fn the_undo_the_chip_advertises_takes_it_away() {
    let mut state = shape_pen_state(create_test_input_state());
    draw(&mut state, &RECTANGLE);
    assert!(state.recognition_chip().is_some());

    run_action(&mut state, Action::Undo);

    assert!(matches!(
        state.boards.active_frame().shapes[0].shape,
        Shape::Freehand { .. } | Shape::FreehandPressure { .. }
    ));
    assert_eq!(chip_label(&state), None);
}

#[test]
fn the_chip_names_the_configured_undo_shortcut() {
    let mut keybindings = crate::config::KeybindingsConfig::default();
    keybindings.core.undo = vec!["Ctrl+Alt+U".to_string()];
    let mut state = shape_pen_state(create_test_input_state_with_keybindings(keybindings));

    draw(&mut state, &RECTANGLE);

    assert_eq!(
        chip_label(&state).as_deref(),
        Some("Rectangle · Ctrl+Alt+U keeps ink")
    );
}

#[test]
fn an_unbound_undo_is_named_in_words() {
    let mut keybindings = crate::config::KeybindingsConfig::default();
    keybindings.core.undo = Vec::new();
    let mut state = shape_pen_state(create_test_input_state_with_keybindings(keybindings));

    draw(&mut state, &RECTANGLE);

    assert_eq!(
        chip_label(&state).as_deref(),
        Some("Rectangle · Undo keeps ink")
    );
}

#[test]
fn turning_recognition_feedback_off_shows_no_chip() {
    let mut state = shape_pen_state(create_test_input_state());
    state.set_shape_recognition_feedback(false);

    draw(&mut state, &RECTANGLE);

    assert!(matches!(
        state.boards.active_frame().shapes[0].shape,
        Shape::Rect { .. }
    ));
    assert_eq!(chip_label(&state), None);
}

#[test]
fn the_chip_fades_and_then_expires() {
    let _motion = crate::ui::anim::override_motion_for_test(true);
    let mut state = shape_pen_state(create_test_input_state());
    draw(&mut state, &RECTANGLE);
    let shown = Instant::now();

    let chip = state.recognition_chip().expect("chip");
    assert_eq!(chip.opacity(shown + Duration::from_millis(500)), 1.0);
    let fading = chip.opacity(shown + Duration::from_millis(1400));
    assert!(fading > 0.0 && fading < 1.0, "fading, found {fading}");
    assert!(state.advance_recognition_chip(shown + Duration::from_millis(500)));

    assert!(!state.advance_recognition_chip(shown + Duration::from_millis(1600)));
    assert_eq!(chip_label(&state), None);
}

#[test]
fn reduced_motion_keeps_the_chip_opaque_for_its_lifetime() {
    let _motion = crate::ui::anim::override_motion_for_test(false);
    let mut state = shape_pen_state(create_test_input_state());
    draw(&mut state, &RECTANGLE);
    let shown = Instant::now();

    let chip = state.recognition_chip().expect("chip");

    assert_eq!(chip.opacity(shown + Duration::from_millis(1400)), 1.0);
}

#[test]
fn switching_boards_takes_the_chip_away() {
    let mut state = shape_pen_state(create_test_input_state());
    draw(&mut state, &RECTANGLE);
    assert!(state.recognition_chip().is_some());

    state.switch_board(crate::input::BOARD_ID_WHITEBOARD);

    assert_eq!(chip_label(&state), None);
}
