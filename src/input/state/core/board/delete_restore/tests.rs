use crate::domain::Action;
use crate::draw::Frame;
use crate::input::BOARD_ID_BLACKBOARD;
use crate::input::state::core::base::{
    BOARD_DELETE_CONFIRM_MS, BOARD_UNDO_EXPIRE_MS, InputState, PAGE_DELETE_CONFIRM_MS,
    PAGE_UNDO_EXPIRE_MS,
};
use crate::input::state::test_support::make_test_input_state;
use crate::input::state::{Toast, ToastPriority};
use std::time::{Duration, Instant};

fn board_index(state: &InputState, id: &str) -> usize {
    state
        .boards
        .board_states()
        .iter()
        .position(|board| board.spec.id == id)
        .expect("board index")
}

fn set_page_count(state: &mut InputState, board_index: usize, count: usize) {
    let pages = state.boards.board_states_mut()[board_index]
        .pages
        .pages_mut();
    pages.clear();
    pages.extend((0..count.max(1)).map(|_| Frame::new()));
}

#[test]
fn confirmed_board_delete_uses_supplied_now_for_undo_timestamp() {
    let mut state = make_test_input_state();
    state.switch_board(BOARD_ID_BLACKBOARD);
    let requested_at = Instant::now();
    let confirmed_at = requested_at + Duration::from_millis(1);

    state.delete_active_board_at(requested_at);
    state.delete_active_board_at(confirmed_at);

    let (_, deleted_at) = state.deleted_boards.last().expect("deleted board undo");
    assert_eq!(*deleted_at, confirmed_at);
}

#[test]
fn expired_board_delete_confirmation_is_replaced_with_supplied_now() {
    let mut state = make_test_input_state();
    state.switch_board(BOARD_ID_BLACKBOARD);
    let requested_at = Instant::now();
    let expired_at = requested_at + Duration::from_millis(BOARD_DELETE_CONFIRM_MS + 1);
    let board_count = state.boards.board_count();

    state.delete_active_board_at(requested_at);
    state.delete_active_board_at(expired_at);

    assert_eq!(state.boards.board_count(), board_count);
    let pending = state
        .pending_board_delete
        .as_ref()
        .expect("replacement confirmation");
    assert_eq!(
        pending.expires_at,
        expired_at + Duration::from_millis(BOARD_DELETE_CONFIRM_MS)
    );
}

#[test]
fn restore_deleted_board_expires_old_entries_with_supplied_now() {
    let mut state = make_test_input_state();
    state.switch_board(BOARD_ID_BLACKBOARD);
    let requested_at = Instant::now();
    let confirmed_at = requested_at + Duration::from_millis(1);

    state.delete_active_board_at(requested_at);
    state.delete_active_board_at(confirmed_at);
    let board_count_after_delete = state.boards.board_count();

    state.restore_deleted_board_at(confirmed_at + Duration::from_millis(BOARD_UNDO_EXPIRE_MS + 1));

    assert!(state.deleted_boards.is_empty());
    assert_eq!(state.boards.board_count(), board_count_after_delete);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("No deleted board to restore.")
    );
}

#[test]
fn confirmed_active_page_delete_uses_supplied_now_for_undo_timestamp() {
    let mut state = make_test_input_state();
    let board = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    set_page_count(&mut state, board, 2);
    let requested_at = Instant::now();
    let confirmed_at = requested_at + Duration::from_millis(1);

    assert_eq!(
        state.delete_active_page_at(requested_at),
        crate::draw::PageDeleteOutcome::Pending
    );
    assert_eq!(
        state.delete_active_page_at(confirmed_at),
        crate::draw::PageDeleteOutcome::Removed
    );

    let (_, deleted_at) = state.deleted_pages.last().expect("deleted page undo");
    assert_eq!(*deleted_at, confirmed_at);
}

#[test]
fn expired_active_page_delete_confirmation_is_replaced_with_supplied_now() {
    let mut state = make_test_input_state();
    let board = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    set_page_count(&mut state, board, 2);
    let requested_at = Instant::now();
    let expired_at = requested_at + Duration::from_millis(PAGE_DELETE_CONFIRM_MS + 1);
    let page_count = state.boards.page_count();

    assert_eq!(
        state.delete_active_page_at(requested_at),
        crate::draw::PageDeleteOutcome::Pending
    );
    assert_eq!(
        state.delete_active_page_at(expired_at),
        crate::draw::PageDeleteOutcome::Pending
    );

    assert_eq!(state.boards.page_count(), page_count);
    let pending = state
        .pending_page_delete
        .as_ref()
        .expect("replacement confirmation");
    assert_eq!(
        pending.expires_at,
        expired_at + Duration::from_millis(PAGE_DELETE_CONFIRM_MS)
    );
}

#[test]
fn expired_page_in_board_delete_confirmation_is_replaced_with_supplied_now() {
    let mut state = make_test_input_state();
    let board = board_index(&state, BOARD_ID_BLACKBOARD);
    set_page_count(&mut state, board, 2);
    let requested_at = Instant::now();
    let expired_at = requested_at + Duration::from_millis(PAGE_DELETE_CONFIRM_MS + 1);
    let page_count = state.boards.board_states()[board].pages.page_count();

    assert_eq!(
        state.delete_page_in_board_at(board, 1, requested_at),
        crate::draw::PageDeleteOutcome::Pending
    );
    assert_eq!(
        state.delete_page_in_board_at(board, 1, expired_at),
        crate::draw::PageDeleteOutcome::Pending
    );

    assert_eq!(
        state.boards.board_states()[board].pages.page_count(),
        page_count
    );
    let pending = state
        .pending_page_delete
        .as_ref()
        .expect("replacement confirmation");
    assert_eq!(
        pending.expires_at,
        expired_at + Duration::from_millis(PAGE_DELETE_CONFIRM_MS)
    );
}

/// A Critical toast can hold a delete/restore action toast queued behind it.
/// Session replacement must retract the *queued* action too, not just an active
/// one, so it cannot surface later against a session that no longer backs it.
#[test]
fn session_replacement_drops_queued_delete_undo_toast() {
    let mut state = make_test_input_state();
    state.switch_board(BOARD_ID_BLACKBOARD);

    // (2 pre): delete a board so an "Undo" action toast is active and a
    // deleted-board undo entry exists.
    let requested_at = Instant::now();
    let confirmed_at = requested_at + Duration::from_millis(1);
    state.delete_active_board_at(requested_at);
    state.delete_active_board_at(confirmed_at);
    assert!(
        !state.deleted_boards.is_empty(),
        "board delete recorded undo"
    );
    assert_eq!(
        state
            .ui_toast
            .as_ref()
            .and_then(|toast| toast.action.as_ref())
            .map(|action| action.action),
        Some(Action::BoardRestoreDeleted),
        "active toast carries the restore action"
    );

    // (1): a Critical toast preempts the undo toast, pushing it into the
    // pending queue. (2): the delete/restore action is now queued.
    state.push_toast(
        ToastPriority::Critical,
        "capability.limitations",
        Toast::warning("Freeze unavailable"),
    );
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.key),
        Some("capability.limitations"),
        "critical toast is active"
    );
    assert!(
        !state.toast_queue.is_empty(),
        "undo toast queued behind the critical toast"
    );

    // (3): session replacement clears delete/restore state and must retract the
    // queued undo toast alongside it.
    state.clear_session_delete_restore_state();
    assert!(state.deleted_boards.is_empty(), "undo state discarded");
    assert!(
        state.toast_queue.is_empty(),
        "queued delete/restore toast retracted, not left behind"
    );

    // (4): the Critical toast expires. (5): nothing stale surfaces.
    let critical = state.ui_toast.as_ref().expect("critical toast");
    let after = critical.started + Duration::from_millis(critical.duration_ms);
    state.advance_ui_toast(after);
    assert!(
        state.ui_toast.is_none(),
        "no stale delete/restore toast surfaced after the critical toast expired"
    );
}

#[test]
fn restore_deleted_page_expires_old_entries_with_supplied_now() {
    let mut state = make_test_input_state();
    let board = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    set_page_count(&mut state, board, 2);
    let requested_at = Instant::now();
    let confirmed_at = requested_at + Duration::from_millis(1);

    assert_eq!(
        state.delete_active_page_at(requested_at),
        crate::draw::PageDeleteOutcome::Pending
    );
    assert_eq!(
        state.delete_active_page_at(confirmed_at),
        crate::draw::PageDeleteOutcome::Removed
    );
    let page_count_after_delete = state.boards.page_count();

    state.restore_deleted_page_at(confirmed_at + Duration::from_millis(PAGE_UNDO_EXPIRE_MS + 1));

    assert!(state.deleted_pages.is_empty());
    assert_eq!(state.boards.page_count(), page_count_after_delete);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("No deleted page to restore.")
    );
}
