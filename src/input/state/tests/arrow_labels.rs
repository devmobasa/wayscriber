use super::*;

use crate::draw::{ArrowLabel, ArrowStyle};
use crate::input::{BOARD_ID_BLACKBOARD, BOARD_ID_WHITEBOARD};

fn arrow_with_label(value: u32, font_descriptor: &FontDescriptor) -> Shape {
    Shape::Arrow {
        x1: 0,
        y1: 0,
        x2: 20,
        y2: 10,
        color: Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
        thick: 2.0,
        arrow_length: 10.0,
        arrow_angle: 30.0,
        head_at_end: true,
        style: ArrowStyle::Standard,
        bend: 0.0,
        label: Some(ArrowLabel {
            value,
            size: 12.0,
            font_descriptor: font_descriptor.clone(),
        }),
    }
}

#[test]
fn sync_arrow_label_counter_uses_max_across_boards() {
    let mut state = create_test_input_state();
    let font_descriptor = state.style.font_descriptor.clone();

    state
        .boards
        .active_frame_mut()
        .add_shape(arrow_with_label(2, &font_descriptor));

    state.switch_board(BOARD_ID_WHITEBOARD);
    state
        .boards
        .active_frame_mut()
        .add_shape(arrow_with_label(7, &font_descriptor));

    state.switch_board(BOARD_ID_BLACKBOARD);
    state
        .boards
        .active_frame_mut()
        .add_shape(arrow_with_label(4, &font_descriptor));

    state.sync_arrow_label_counter();
    assert_eq!(state.style.arrow_label_counter, 8);
}

#[test]
fn next_arrow_label_returns_none_when_disabled() {
    let state = create_test_input_state();
    assert!(state.next_arrow_label().is_none());
}

#[test]
fn enabling_arrow_labels_syncs_counter_and_marks_session_dirty() {
    let mut state = create_test_input_state();
    let font_descriptor = state.style.font_descriptor.clone();
    state
        .boards
        .active_frame_mut()
        .add_shape(arrow_with_label(5, &font_descriptor));
    state.needs_redraw = false;
    state.clear_session_dirty();

    assert!(state.set_arrow_label_enabled(true));
    assert!(state.style.arrow_label_enabled);
    assert_eq!(state.style.arrow_label_counter, 6);
    assert!(state.needs_redraw);
    assert!(state.is_session_dirty());
}

#[test]
fn enabling_arrow_labels_is_noop_when_already_enabled() {
    let mut state = create_test_input_state();
    state.style.arrow_label_enabled = true;
    state.needs_redraw = false;
    state.clear_session_dirty();

    assert!(!state.set_arrow_label_enabled(true));
    assert!(!state.needs_redraw);
    assert!(!state.is_session_dirty());
}

#[test]
fn reset_arrow_label_counter_reports_no_change_at_default() {
    let mut state = create_test_input_state();
    state.needs_redraw = false;

    assert!(!state.reset_arrow_label_counter());
    assert_eq!(state.style.arrow_label_counter, 1);
    assert!(!state.needs_redraw);
}
