use super::*;

use crate::draw::StepMarkerLabel;
use crate::input::{BOARD_ID_BLACKBOARD, BOARD_ID_WHITEBOARD};

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
