use super::*;

use crate::draw::StepMarkerLabel;
use crate::input::{BOARD_ID_BLACKBOARD, BOARD_ID_WHITEBOARD};
use crate::ui::toolbar::ToolbarEvent;

fn step_marker_with_label(value: u32, font_descriptor: &FontDescriptor) -> Shape {
    Shape::StepMarker {
        x: 10,
        y: 20,
        color: Color {
            r: 0.2,
            g: 0.4,
            b: 0.8,
            a: 1.0,
        },
        label: StepMarkerLabel {
            value,
            size: 14.0,
            font_descriptor: font_descriptor.clone(),
        },
    }
}

#[test]
fn sync_step_marker_counter_uses_max_across_boards() {
    let mut state = create_test_input_state();
    let font_descriptor = state.font_descriptor.clone();

    state
        .boards
        .active_frame_mut()
        .add_shape(step_marker_with_label(3, &font_descriptor));

    state.switch_board(BOARD_ID_WHITEBOARD);
    state
        .boards
        .active_frame_mut()
        .add_shape(step_marker_with_label(9, &font_descriptor));

    state.switch_board(BOARD_ID_BLACKBOARD);
    state
        .boards
        .active_frame_mut()
        .add_shape(step_marker_with_label(5, &font_descriptor));

    state.sync_step_marker_counter();
    assert_eq!(state.step_marker_counter, 10);
}

#[test]
fn next_step_marker_label_clamps_size() {
    let mut state = create_test_input_state();

    state.current_font_size = 10.0;
    let label = state.next_step_marker_label();
    assert_eq!(label.size, 12.0);

    state.current_font_size = 100.0;
    let label = state.next_step_marker_label();
    assert_eq!(label.size, 36.0);
}

#[test]
fn toolbar_reset_step_marker_counter_resets_to_one() {
    let mut state = create_test_input_state();
    state.step_marker_counter = 5;

    let changed = state.apply_toolbar_event(ToolbarEvent::ResetStepMarkerCounter);

    assert!(changed);
    assert_eq!(state.step_marker_counter, 1);
}

#[test]
fn drawing_step_marker_increments_counter() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::StepMarker));

    state.on_mouse_press(MouseButton::Left, 10, 10);
    state.on_mouse_release(MouseButton::Left, 10, 10);
    state.on_mouse_press(MouseButton::Left, 20, 20);
    state.on_mouse_release(MouseButton::Left, 20, 20);

    let shapes = &state.boards.active_frame().shapes;
    assert_eq!(shapes.len(), 2);

    let first_value = match &shapes[0].shape {
        Shape::StepMarker { label, .. } => label.value,
        _ => panic!("expected step marker for first shape"),
    };
    let second_value = match &shapes[1].shape {
        Shape::StepMarker { label, .. } => label.value,
        _ => panic!("expected step marker for second shape"),
    };

    assert_eq!(first_value, 1);
    assert_eq!(second_value, 2);
    assert_eq!(state.step_marker_counter, 3);
}
