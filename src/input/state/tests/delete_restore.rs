use super::*;
use crate::draw::{Frame, PageDeleteOutcome};
use crate::input::{BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT};

fn board_index(state: &InputState, id: &str) -> usize {
    state
        .boards
        .board_states()
        .iter()
        .position(|board| board.spec.id == id)
        .expect("board index")
}

fn set_page_count(state: &mut InputState, board_index: usize, count: usize) {
    let pages = state.boards.board_states_mut()[board_index].pages.pages_mut();
    pages.clear();
    pages.extend((0..count.max(1)).map(|_| Frame::new()));
}

#[test]
fn delete_active_board_requires_confirmation_then_restore_recovers_board() {
    let mut state = create_test_input_state();
    let initial_count = state.boards.board_count();
    state.switch_board(BOARD_ID_BLACKBOARD);

    state.delete_active_board();
    assert!(state.has_pending_board_delete());
    assert_eq!(state.boards.board_count(), initial_count);
    assert!(state
        .ui_toast
        .as_ref()
        .is_some_and(|toast| toast.message.contains("Click to confirm.")));

    state.delete_active_board();
    assert!(!state.has_pending_board_delete());
    assert_eq!(state.boards.board_count(), initial_count - 1);
    assert_ne!(state.board_id(), BOARD_ID_BLACKBOARD);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Board deleted: Blackboard")
    );

    state.restore_deleted_board();
    assert_eq!(state.boards.board_count(), initial_count);
    assert_eq!(state.board_id(), BOARD_ID_BLACKBOARD);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Board restored: Blackboard")
    );
}

#[test]
fn cancel_pending_board_delete_clears_confirmation_state() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_BLACKBOARD);
    state.delete_active_board();
    assert!(state.has_pending_board_delete());

    state.cancel_pending_board_delete();

    assert!(!state.has_pending_board_delete());
    assert_eq!(state.board_id(), BOARD_ID_BLACKBOARD);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Board deletion cancelled.")
    );
}

#[test]
fn page_delete_requires_confirmation_and_restore_recovers_deleted_page() {
    let mut state = create_test_input_state();
    let board = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    set_page_count(&mut state, board, 2);

    assert_eq!(state.page_delete(), PageDeleteOutcome::Pending);
    assert!(state.has_pending_page_delete());
    assert_eq!(state.boards.page_count(), 2);

    assert_eq!(state.page_delete(), PageDeleteOutcome::Removed);
    assert!(!state.has_pending_page_delete());
    assert_eq!(state.boards.page_count(), 1);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Page deleted (1/1)")
    );

    state.restore_deleted_page();
    assert_eq!(state.boards.page_count(), 2);
    assert_eq!(state.boards.active_page_index(), 1);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Page restored (2/2)")
    );
}

#[test]
fn cancel_pending_page_delete_clears_confirmation_state() {
    let mut state = create_test_input_state();
    let board = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    set_page_count(&mut state, board, 2);
    state.page_delete();
    assert!(state.has_pending_page_delete());

    state.cancel_pending_page_delete();

    assert!(!state.has_pending_page_delete());
    assert_eq!(state.boards.page_count(), 2);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Page deletion cancelled.")
    );
}

#[test]
fn page_delete_on_last_page_clears_shapes_without_removing_page() {
    let mut state = create_test_input_state();
    assert_eq!(state.board_id(), BOARD_ID_TRANSPARENT);
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    assert_eq!(state.page_delete(), PageDeleteOutcome::Cleared);

    assert_eq!(state.boards.page_count(), 1);
    assert!(state.boards.active_frame().shapes.is_empty());
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Page cleared (last page)")
    );
}
