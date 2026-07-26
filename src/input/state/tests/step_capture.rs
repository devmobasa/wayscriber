use super::*;

use crate::draw::EmbeddedImage;
use crate::input::state::{STEP_CAPTURE_BOARD_ID, StepCaptureFrame, StepPageReceipt};

fn test_frame(marker: Option<(i32, i32)>) -> StepCaptureFrame {
    StepCaptureFrame {
        image: EmbeddedImage {
            mime_type: "image/png".to_string(),
            width: 4,
            height: 4,
            bytes: vec![1, 2, 3, 4],
        },
        logical_width: 1280,
        logical_height: 720,
        marker,
    }
}

fn steps_board(state: &InputState) -> &crate::input::boards::BoardState {
    state
        .boards
        .board_states()
        .iter()
        .find(|board| board.spec.id == STEP_CAPTURE_BOARD_ID)
        .expect("the append created the Steps board")
}

#[test]
fn first_step_creates_the_steps_board_and_claims_its_blank_page() {
    let mut state = create_test_input_state();

    let receipt = state
        .append_step_page(test_frame(Some((100, 200))))
        .expect("append succeeds");
    assert_eq!(
        receipt,
        StepPageReceipt {
            step: 1,
            page_count: 1,
        }
    );

    let board = steps_board(&state);
    assert_eq!(board.spec.name, "Steps");
    assert_eq!(board.pages.page_count(), 1);
    let page = &board.pages.pages()[0];
    assert_eq!(page.shapes.len(), 2, "backdrop image plus marker");
    let backdrop = &page.shapes[0];
    assert!(backdrop.locked, "the captured frame must not be draggable");
    assert!(matches!(
        backdrop.shape,
        Shape::Image {
            x: 0,
            y: 0,
            w: 1280,
            h: 720,
            ..
        }
    ));
    match &page.shapes[1].shape {
        Shape::StepMarker { x, y, label, .. } => {
            assert_eq!((*x, *y), (100, 200));
            assert_eq!(label.value, 1);
        }
        other => panic!("expected a step marker, got {other:?}"),
    }
}

#[test]
fn later_steps_append_numbered_pages() {
    let mut state = create_test_input_state();

    for expected in 1..=3u32 {
        let receipt = state
            .append_step_page(test_frame(Some((10, 10))))
            .expect("append succeeds");
        assert_eq!(receipt.step, expected);
    }

    let board = steps_board(&state);
    assert_eq!(board.pages.page_count(), 3);
    for (index, page) in board.pages.pages().iter().enumerate() {
        match &page.shapes[1].shape {
            Shape::StepMarker { label, .. } => assert_eq!(label.value, index as u32 + 1),
            other => panic!("expected a step marker, got {other:?}"),
        }
    }
}

#[test]
fn a_markerless_frame_still_becomes_a_page() {
    let mut state = create_test_input_state();

    let receipt = state
        .append_step_page(test_frame(None))
        .expect("append succeeds");
    assert_eq!(receipt.step, 1);
    let board = steps_board(&state);
    assert_eq!(board.pages.pages()[0].shapes.len(), 1, "backdrop only");
}

#[test]
fn capturing_does_not_switch_the_active_board() {
    let mut state = create_test_input_state();
    let active_before = state.boards.active_index();

    state
        .append_step_page(test_frame(Some((5, 5))))
        .expect("append succeeds");

    assert_eq!(
        state.boards.active_index(),
        active_before,
        "captures must not yank the user away from their board"
    );
}

#[test]
fn next_step_number_tracks_the_steps_board() {
    let mut state = create_test_input_state();
    assert_eq!(state.next_step_number(), 1);

    state
        .append_step_page(test_frame(None))
        .expect("append succeeds");
    assert_eq!(state.next_step_number(), 2);
}

#[test]
fn disarming_jumps_to_the_steps_board_for_review() {
    let mut state = create_test_input_state();
    assert!(state.toggle_step_capture(), "first toggle arms");
    assert!(state.step_capture_armed());

    state
        .append_step_page(test_frame(Some((1, 1))))
        .expect("append succeeds");
    state
        .append_step_page(test_frame(Some((2, 2))))
        .expect("append succeeds");

    assert!(!state.toggle_step_capture(), "second toggle disarms");
    assert!(!state.step_capture_armed());
    let active = state.boards.active_index();
    assert_eq!(
        state.boards.board_states()[active].spec.id,
        STEP_CAPTURE_BOARD_ID,
        "review starts on the Steps board"
    );
    assert_eq!(steps_board(&state).pages.active_index(), 0);
}

#[test]
fn disarming_without_steps_stays_on_the_current_board() {
    let mut state = create_test_input_state();
    let active_before = state.boards.active_index();
    assert!(state.toggle_step_capture());
    assert!(!state.toggle_step_capture());
    assert_eq!(state.boards.active_index(), active_before);
}
