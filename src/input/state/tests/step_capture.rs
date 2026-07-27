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

fn steps_board_index(state: &InputState) -> usize {
    state
        .boards
        .board_states()
        .iter()
        .position(|board| board.spec.id == STEP_CAPTURE_BOARD_ID)
        .expect("the append created the Steps board")
}

/// Marker numbers in page order, `None` for a page without one.
fn marker_numbers(state: &InputState) -> Vec<Option<u32>> {
    steps_board(state)
        .pages
        .pages()
        .iter()
        .map(|page| {
            page.shapes.iter().find_map(|drawn| match &drawn.shape {
                Shape::StepMarker { label, .. } => Some(label.value),
                _ => None,
            })
        })
        .collect()
}

fn capture_steps(state: &mut InputState, count: usize) {
    for index in 0..count {
        state
            .append_step_page(test_frame(Some((index as i32, index as i32))))
            .receipt()
            .expect("append succeeds");
    }
}

#[test]
fn first_step_creates_the_steps_board_and_claims_its_blank_page() {
    let mut state = create_test_input_state();

    let receipt = state
        .append_step_page(test_frame(Some((100, 200))))
        .receipt()
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
            .receipt()
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
        .receipt()
        .expect("append succeeds");
    assert_eq!(receipt.step, 1);
    let board = steps_board(&state);
    assert_eq!(board.pages.pages()[0].shapes.len(), 1, "backdrop only");
}

#[test]
fn a_step_that_exceeds_the_shape_limit_is_refused_before_creating_its_board() {
    let mut state = create_test_input_state();
    state.max_shapes_per_frame = 1;

    let outcome = state.append_step_page(test_frame(Some((10, 10))));

    assert_eq!(outcome, crate::input::state::StepPageOutcome::ShapeLimit);
    assert!(
        state
            .boards
            .board_states()
            .iter()
            .all(|board| board.spec.id != STEP_CAPTURE_BOARD_ID),
        "a refused first step must not leave an empty Steps board behind"
    );
}

#[test]
fn capturing_does_not_switch_the_active_board() {
    let mut state = create_test_input_state();
    let active_before = state.boards.active_index();

    state
        .append_step_page(test_frame(Some((5, 5))))
        .receipt()
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
        .receipt()
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
        .receipt()
        .expect("append succeeds");
    state
        .append_step_page(test_frame(Some((2, 2))))
        .receipt()
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
fn deleting_a_page_renumbers_the_markers_that_follow_it() {
    let mut state = create_test_input_state();
    capture_steps(&mut state, 3);
    assert_eq!(marker_numbers(&state), vec![Some(1), Some(2), Some(3)]);

    let board_index = steps_board_index(&state);
    // The first press asks for confirmation; the second deletes.
    state.delete_page_in_board(board_index, 1);
    state.delete_page_in_board(board_index, 1);

    assert_eq!(
        marker_numbers(&state),
        vec![Some(1), Some(2)],
        "the surviving steps renumber to match the exported guide's ordering"
    );
    assert_eq!(state.next_step_number(), 3);
}

#[test]
fn reordering_pages_renumbers_the_markers() {
    let mut state = create_test_input_state();
    capture_steps(&mut state, 3);

    let board_index = steps_board_index(&state);
    assert!(state.reorder_page_in_board(board_index, 2, 0));

    assert_eq!(marker_numbers(&state), vec![Some(1), Some(2), Some(3)]);
    // The page that used to be step 3 now leads the guide, marker and all.
    let first_marker_position =
        steps_board(&state).pages.pages()[0]
            .shapes
            .iter()
            .find_map(|drawn| match &drawn.shape {
                Shape::StepMarker { x, y, .. } => Some((*x, *y)),
                _ => None,
            });
    assert_eq!(first_marker_position, Some((2, 2)));
}

#[test]
fn duplicating_the_active_step_page_renumbers_every_marker() {
    let mut state = create_test_input_state();
    capture_steps(&mut state, 2);
    state.switch_board_force(STEP_CAPTURE_BOARD_ID);
    assert!(state.switch_to_page(0) || state.boards.active_page_index() == 0);

    state.page_duplicate();

    assert_eq!(
        marker_numbers(&state),
        vec![Some(1), Some(2), Some(3)],
        "the ordinary duplicate action keeps visible markers aligned with guide order"
    );
}

#[test]
fn hand_placed_markers_keep_their_numbers_when_pages_move() {
    let mut state = create_test_input_state();
    capture_steps(&mut state, 2);

    let board_index = steps_board_index(&state);
    let font_descriptor = state.font_descriptor.clone();
    state
        .boards
        .board_state_mut(board_index)
        .expect("steps board")
        .pages
        .pages_mut()[0]
        .add_shape(Shape::StepMarker {
            x: 40,
            y: 40,
            color: crate::draw::color::WHITE,
            label: crate::draw::StepMarkerLabel {
                value: 99,
                size: 14.0,
                font_descriptor,
                auto_numbered: false,
            },
        });

    assert!(state.reorder_page_in_board(board_index, 1, 0));

    let hand_placed = steps_board(&state)
        .pages
        .pages()
        .iter()
        .flat_map(|page| page.shapes.iter())
        .filter_map(|drawn| match &drawn.shape {
            Shape::StepMarker { label, .. } if !label.auto_numbered => Some(label.value),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        hand_placed,
        vec![99],
        "markers the user placed by hand are not renumbered"
    );
}

#[test]
fn disarming_without_steps_stays_on_the_current_board() {
    let mut state = create_test_input_state();
    let active_before = state.boards.active_index();
    assert!(state.toggle_step_capture());
    assert!(!state.toggle_step_capture());
    assert_eq!(state.boards.active_index(), active_before);
}
