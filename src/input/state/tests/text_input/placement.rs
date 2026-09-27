use super::super::*;
use crate::draw::TextMeasurer;
use crate::draw::shape::{bounding_box_for_sticky_note_preview_with, bounding_box_for_text_with};
use crate::util::Rect;

const OUTPUT: (u32, u32) = (800, 600);

fn output_state() -> InputState {
    let mut state = create_test_input_state();
    state.update_screen_dimensions(OUTPUT.0, OUTPUT.1);
    state
}

fn enter_mode(state: &mut InputState, action: Action) {
    let measurer = TextMeasurer::default();
    let ui_engine = crate::ui_text::UiTextEngine::default();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    state.handle_action_with_resources(resources, action);
    assert!(matches!(state.state, DrawingState::TextInput { .. }));
}

fn type_text(state: &mut InputState, text: &str) {
    for c in text.chars() {
        state.on_key_press(Key::Char(c));
    }
}

fn draft_origin(state: &InputState) -> (i32, i32) {
    match &state.state {
        DrawingState::TextInput { x, y, .. } => (*x, *y),
        other => panic!("expected an active text draft, found {other:?}"),
    }
}

fn draft_bounds(state: &InputState) -> Rect {
    let measurer = TextMeasurer::default();
    let DrawingState::TextInput { x, y, buffer, .. } = &state.state else {
        panic!("expected an active text draft");
    };
    let bounds = match state.text_editing.mode() {
        TextInputMode::Plain => bounding_box_for_text_with(
            &measurer,
            *x,
            *y,
            buffer,
            state.style.current_font_size,
            &state.style.font_descriptor,
            state.style.text_background_enabled,
            state.style.text_wrap_width,
        ),
        TextInputMode::StickyNote => bounding_box_for_sticky_note_preview_with(
            &measurer,
            *x,
            *y,
            buffer,
            state.style.current_font_size,
            &state.style.font_descriptor,
            state.style.text_wrap_width,
        ),
    };
    bounds.expect("a typed draft has bounds")
}

fn committed_bounds(state: &InputState) -> Rect {
    let measurer = TextMeasurer::default();
    state
        .boards
        .active_frame()
        .shapes
        .last()
        .and_then(|shape| shape.shape.bounding_box_with(&measurer))
        .expect("a committed text shape has bounds")
}

fn assert_inside(bounds: Rect, area: Rect, what: &str) {
    assert!(
        bounds.x >= area.x
            && bounds.y >= area.y
            && bounds.x + bounds.width <= area.x + area.width
            && bounds.y + bounds.height <= area.y + area.height,
        "{what} {bounds:?} must stay inside {area:?}"
    );
}

fn output_rect() -> Rect {
    Rect::new(0, 0, OUTPUT.0 as i32, OUTPUT.1 as i32).unwrap()
}

#[test]
fn sticky_note_placed_near_the_right_edge_stays_inside_the_output() {
    let mut state = output_state();
    enter_mode(&mut state, Action::EnterStickyNoteMode);

    state.on_mouse_press(MouseButton::Left, 780, 300);
    state.on_mouse_release(MouseButton::Left, 780, 300);
    type_text(&mut state, "Check module order");

    assert!(draft_origin(&state).0 < 780, "the anchor shifts left");
    assert_inside(draft_bounds(&state), output_rect(), "the live note");

    state.on_key_press(Key::Return);

    assert!(matches!(state.state, DrawingState::Idle));
    assert_inside(committed_bounds(&state), output_rect(), "the placed note");
}

#[test]
fn text_placed_near_the_bottom_edge_moves_up_inside_the_output() {
    let mut state = output_state();
    enter_mode(&mut state, Action::EnterTextMode);

    state.on_mouse_press(MouseButton::Left, 100, 598);
    state.on_mouse_release(MouseButton::Left, 100, 598);
    type_text(&mut state, "Bottom");

    assert!(draft_origin(&state).1 < 598, "the anchor shifts up");
    assert_eq!(draft_origin(&state).0, 100, "a fitting axis stays put");
    state.on_key_press(Key::Return);

    assert_inside(committed_bounds(&state), output_rect(), "the placed text");
}

#[test]
fn text_placed_away_from_the_edges_keeps_its_anchor() {
    let mut state = output_state();
    enter_mode(&mut state, Action::EnterTextMode);

    state.on_mouse_press(MouseButton::Left, 200, 200);
    state.on_mouse_release(MouseButton::Left, 200, 200);
    type_text(&mut state, "Middle");

    assert_eq!(draft_origin(&state), (200, 200));
}

#[test]
fn a_zoomed_view_keeps_the_draft_inside_the_visible_canvas() {
    let mut state = output_state();
    state.set_zoom_status(true, false, 2.0, (100.0, 200.0));
    let visible = state.visible_canvas_rect();
    assert_ne!(visible, output_rect(), "the zoom must narrow the view");
    state.text_editing.set_mode(TextInputMode::StickyNote);
    state.state = DrawingState::text_input(
        visible.x + visible.width - 10,
        visible.y + visible.height / 2,
        String::new(),
    );

    type_text(&mut state, "Zoomed note");

    assert_inside(draft_bounds(&state), visible, "the zoomed note");
}

#[test]
fn editing_a_note_that_already_straddles_the_edge_does_not_move_it() {
    let measurer = TextMeasurer::default();
    let mut state = output_state();
    let note = state
        .boards
        .active_frame_mut()
        .add_shape(Shape::StickyNote {
            x: 760,
            y: 300,
            text: "Past the edge".to_string(),
            background: state.style.current_color,
            size: state.style.current_font_size,
            font_descriptor: state.style.font_descriptor.clone(),
            wrap_width: None,
        });
    state.set_selection(vec![note]);

    assert!(state.edit_selected_text_with(&measurer));
    assert_eq!(draft_origin(&state), (760, 300));

    state.on_key_press(Key::Return);

    match &state.boards.active_frame().shape(note).expect("note").shape {
        Shape::StickyNote { x, y, .. } => assert_eq!((*x, *y), (760, 300)),
        other => panic!("expected the note, found {other:?}"),
    }
}
