use super::*;
use crate::draw::ShapeId;
use crate::input::state::SelectionHandle;
use std::sync::Arc;

fn select_state() -> InputState {
    let mut state = create_test_input_state();
    state.update_screen_dimensions(800, 600);
    state.set_tool_override(Some(Tool::Select));
    state
}

fn add_filled_rect(state: &mut InputState, x: i32, y: i32) -> ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x,
        y,
        w: 40,
        h: 40,
        fill: true,
        fill_color: None,
        color: state.style.current_color,
        thick: state.style.current_thickness,
    })
}

fn rect_geometry(state: &InputState, id: ShapeId) -> (i32, i32, i32, i32) {
    match &state.boards.active_frame().shape(id).expect("rect").shape {
        Shape::Rect { x, y, w, h, .. } => (*x, *y, *w, *h),
        other => panic!("expected a rect, got {other:?}"),
    }
}

fn undo(state: &mut InputState) {
    let measurer = crate::draw::TextMeasurer::default();
    let ui_engine = crate::ui_text::UiTextEngine::default();
    state.handle_action_with_resources(
        crate::input::state::InputTextResources {
            measurer: &measurer,
            ui_engine: &ui_engine,
        },
        Action::Undo,
    );
}

#[test]
fn a_nudge_key_mid_move_lands_the_move_before_recording_the_nudge() {
    let mut state = select_state();
    let rect = add_filled_rect(&mut state, 100, 100);
    let original = rect_geometry(&state, rect);

    state.on_mouse_press(MouseButton::Left, 120, 120);
    state.on_mouse_motion(170, 150);
    let dragged = rect_geometry(&state, rect);
    assert_eq!(dragged, (150, 130, 40, 40));

    state.on_key_press(Key::Right);

    assert!(
        matches!(state.state, DrawingState::Idle),
        "the nudge left the move running"
    );
    let nudged = rect_geometry(&state, rect);
    assert_ne!(nudged, dragged, "the nudge did not move the selection");
    assert_eq!(state.boards.active_frame().undo_stack_len(), 2);

    state.on_mouse_release(MouseButton::Left, 170, 150);

    assert_eq!(rect_geometry(&state, rect), nudged);
    assert_eq!(
        state.boards.active_frame().undo_stack_len(),
        2,
        "the release recorded the move a second time"
    );

    undo(&mut state);
    assert_eq!(rect_geometry(&state, rect), dragged);
    undo(&mut state);
    assert_eq!(rect_geometry(&state, rect), original);
}

#[test]
fn a_nudge_key_mid_resize_lands_the_resize_before_recording_the_nudge() {
    let measurer = crate::draw::TextMeasurer::default();
    let mut state = select_state();
    let rect = add_filled_rect(&mut state, 100, 100);
    let original = rect_geometry(&state, rect);
    state.set_selection(vec![rect]);
    let original_bounds = state.selection_bounds_with(&measurer).expect("bounds");
    let snapshots = state.capture_resize_selection_snapshots();
    state.begin_pointer_drag(MouseButton::Left, None);
    state.state = DrawingState::ResizingSelection {
        handle: SelectionHandle::BottomRight,
        original_bounds,
        start_x: 140,
        start_y: 140,
        snapshots: Arc::new(snapshots),
    };

    state.on_mouse_motion(180, 180);
    let resized = rect_geometry(&state, rect);
    assert_ne!(resized, original, "the drag did not resize the rect");

    state.on_key_press(Key::Right);

    assert!(
        matches!(state.state, DrawingState::Idle),
        "the nudge left the resize running"
    );
    let nudged = rect_geometry(&state, rect);
    assert_ne!(nudged, resized, "the nudge did not move the selection");

    state.on_mouse_release(MouseButton::Left, 180, 180);

    assert_eq!(rect_geometry(&state, rect), nudged);
    assert_eq!(state.boards.active_frame().undo_stack_len(), 2);

    undo(&mut state);
    assert_eq!(rect_geometry(&state, rect), resized);
    undo(&mut state);
    assert_eq!(rect_geometry(&state, rect), original);
}

#[test]
fn a_held_press_interrupted_by_a_key_is_not_a_click() {
    let mut state = select_state();
    let rect = add_filled_rect(&mut state, 100, 100);
    state.on_mouse_press(MouseButton::Left, 120, 120);
    state.on_mouse_release(MouseButton::Left, 120, 120);
    assert_eq!(state.selected_shape_ids(), &[rect]);

    // The second press of what would be a double-click is still held when a
    // key lands. Settling the press for that key must not open the shape.
    state.on_mouse_press(MouseButton::Left, 120, 120);
    state.on_key_press(Key::Right);
    state.on_mouse_release(MouseButton::Left, 120, 120);

    assert!(!state.is_properties_panel_open());
    assert!(matches!(state.state, DrawingState::Idle));
}

#[test]
fn escape_mid_move_still_cancels_the_move() {
    let mut state = select_state();
    let rect = add_filled_rect(&mut state, 100, 100);
    let original = rect_geometry(&state, rect);
    state.on_mouse_press(MouseButton::Left, 120, 120);
    state.on_mouse_motion(170, 150);

    state.on_key_press(Key::Escape);

    assert!(matches!(state.state, DrawingState::Idle));
    assert_eq!(rect_geometry(&state, rect), original);
    assert_eq!(state.boards.active_frame().undo_stack_len(), 0);
    assert!(!state.should_exit, "Escape cancelled the move, not the app");
}

#[test]
fn an_action_during_a_box_selection_leaves_the_rubber_band_running() {
    let mut state = select_state();
    state.on_mouse_press(MouseButton::Left, 300, 300);
    state.on_mouse_motion(340, 340);
    assert!(matches!(state.state, DrawingState::Selecting { .. }));

    state.on_key_press(Key::Right);

    assert!(
        matches!(state.state, DrawingState::Selecting { .. }),
        "an interaction without snapshots was ended, got {:?}",
        state.state
    );
}
