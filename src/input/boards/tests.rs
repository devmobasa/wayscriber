use super::*;
use crate::config::BoardsConfig;
use crate::draw::{BoardPages, Frame};

fn manager() -> BoardManager {
    BoardManager::from_config(BoardsConfig::default())
}

fn board_index(boards: &BoardManager, id: &str) -> usize {
    boards
        .board_states()
        .iter()
        .position(|board| board.spec.id == id)
        .expect("board index")
}

fn two_named_pages(first: &str, second: &str) -> BoardPages {
    let mut a = Frame::new();
    a.set_page_name(Some(first.to_string()));
    let mut b = Frame::new();
    b.set_page_name(Some(second.to_string()));
    BoardPages::from_pages(vec![a, b], 0)
}

fn board_delete_confirmation(boards: &mut BoardManager, id: &str) -> BoardDeleteConfirmation {
    let BoardDeleteOutcome::RequiresConfirmation { confirmation } = boards.delete_board(
        BoardDeleteRequest::Request(BoardDeleteTarget::BoardId(id.to_string())),
    ) else {
        panic!("expected board delete confirmation");
    };
    confirmation
}

fn page_delete_confirmation(
    boards: &mut BoardManager,
    board_id: &str,
    page_index: usize,
) -> PageDeleteConfirmation {
    let PageDeleteOutcome::RequiresConfirmation { confirmation } =
        boards.delete_page(PageDeleteRequest::Request(PageDeleteTarget {
            board: PageDeleteBoardTarget::BoardId(board_id.to_string()),
            page_index,
        }))
    else {
        panic!("expected page delete confirmation");
    };
    confirmation
}

#[test]
fn board_identity_generation_changes_only_for_identity_mutations() {
    let mut boards = manager();
    let initial = boards.board_identity_generation();
    assert!(boards.switch_to_id(BOARD_ID_WHITEBOARD));
    assert_eq!(boards.board_identity_generation(), initial);

    let renamed = board_index(&boards, BOARD_ID_WHITEBOARD);
    boards.board_states_mut()[renamed].spec.name = "Renamed".to_string();
    assert_eq!(boards.board_identity_generation(), initial);

    boards.new_page();
    assert_eq!(boards.board_identity_generation(), initial);

    assert!(boards.move_board(1, 2));
    assert_eq!(boards.board_identity_generation(), initial);

    assert!(boards.create_board());
    let after_create = boards.board_identity_generation();
    assert_ne!(after_create, initial);

    assert!(boards.duplicate_active_board().is_some());
    let after_duplicate = boards.board_identity_generation();
    assert_ne!(after_duplicate, after_create);
}

#[test]
fn confirmed_board_delete_uses_id_after_active_drift_and_reorder() {
    let mut boards = manager();
    let confirmation = board_delete_confirmation(&mut boards, BOARD_ID_BLACKBOARD);

    assert!(boards.switch_to_id(BOARD_ID_WHITEBOARD));
    let blackboard = board_index(&boards, BOARD_ID_BLACKBOARD);
    assert!(boards.move_board(blackboard, 1));

    let BoardDeleteOutcome::Deleted { deleted_id, .. } =
        boards.delete_board(BoardDeleteRequest::Confirm(confirmation))
    else {
        panic!("expected delete");
    };
    assert_eq!(deleted_id, BOARD_ID_BLACKBOARD);
    assert!(!boards.has_board(BOARD_ID_BLACKBOARD));
    assert_eq!(boards.active_board_id(), BOARD_ID_WHITEBOARD);
}

#[test]
fn board_rename_does_not_stale_board_delete_confirmation() {
    let mut boards = manager();
    let confirmation = board_delete_confirmation(&mut boards, BOARD_ID_BLACKBOARD);
    let index = board_index(&boards, BOARD_ID_BLACKBOARD);
    boards.board_states_mut()[index].spec.name = "New display name".to_string();

    assert!(matches!(
        boards.delete_board(BoardDeleteRequest::Confirm(confirmation)),
        BoardDeleteOutcome::Deleted { .. }
    ));
}

#[test]
fn board_identity_change_rejects_stale_board_delete_confirmation() {
    let mut boards = manager();
    let confirmation = board_delete_confirmation(&mut boards, BOARD_ID_BLACKBOARD);

    assert!(boards.create_board());

    assert!(matches!(
        boards.delete_board(BoardDeleteRequest::Confirm(confirmation)),
        BoardDeleteOutcome::Rejected(BoardDeleteRejection::StaleConfirmation)
    ));
}

#[test]
fn cloned_board_manager_gets_fresh_identity_generation() {
    let boards = manager();
    let clone = boards.clone();

    assert_ne!(
        clone.board_identity_generation(),
        boards.board_identity_generation()
    );
}

#[test]
fn deleted_and_reused_board_id_rejects_stale_board_delete_confirmation() {
    let mut boards = manager();
    let stale = board_delete_confirmation(&mut boards, BOARD_ID_BLACKBOARD);
    let current = board_delete_confirmation(&mut boards, BOARD_ID_BLACKBOARD);

    let BoardDeleteOutcome::Deleted { deleted_board, .. } =
        boards.delete_board(BoardDeleteRequest::Confirm(current))
    else {
        panic!("expected delete");
    };
    assert!(matches!(
        boards.restore_board(BoardRestoreRequest {
            board: deleted_board,
            preferred_index: None,
        }),
        BoardRestoreOutcome::Restored {
            id_changed: false,
            ..
        }
    ));
    assert!(boards.has_board(BOARD_ID_BLACKBOARD));

    assert!(matches!(
        boards.delete_board(BoardDeleteRequest::Confirm(stale)),
        BoardDeleteOutcome::Rejected(BoardDeleteRejection::StaleConfirmation)
    ));
}

#[test]
fn full_board_manager_replacement_rejects_same_id_confirmation() {
    let mut boards = manager();
    let confirmation = board_delete_confirmation(&mut boards, BOARD_ID_BLACKBOARD);

    let mut replacement = manager();

    assert!(matches!(
        replacement.delete_board(BoardDeleteRequest::Confirm(confirmation)),
        BoardDeleteOutcome::Rejected(BoardDeleteRejection::StaleConfirmation)
    ));
}

#[test]
fn confirmation_identity_ignores_display_board_name() {
    let mut boards = manager();
    let board_confirmation = board_delete_confirmation(&mut boards, BOARD_ID_BLACKBOARD);
    let mut renamed_board_confirmation = board_confirmation.clone();
    renamed_board_confirmation.board_name = "Different prompt text".to_string();
    assert!(renamed_board_confirmation.matches_identity(
        &board_confirmation.board_id,
        board_confirmation.board_identity_generation
    ));

    let index = board_index(&boards, BOARD_ID_BLACKBOARD);
    boards.board_states_mut()[index].pages = two_named_pages("one", "two");
    let page_confirmation = page_delete_confirmation(&mut boards, BOARD_ID_BLACKBOARD, 0);
    let mut renamed_page_confirmation = page_confirmation.clone();
    renamed_page_confirmation.board_name = "Different prompt text".to_string();
    assert!(renamed_page_confirmation.matches_identity(
        &page_confirmation.board_id,
        page_confirmation.board_identity_generation,
        page_confirmation.page_index,
        page_confirmation.page_count,
        page_confirmation.page_generation
    ));
}

#[test]
fn page_delete_confirmation_validates_board_generation_page_count_and_page_generation() {
    let mut boards = manager();
    let index = board_index(&boards, BOARD_ID_BLACKBOARD);
    boards.board_states_mut()[index].pages = two_named_pages("one", "two");
    let confirmation = page_delete_confirmation(&mut boards, BOARD_ID_BLACKBOARD, 0);

    boards.board_states_mut()[index].pages.new_page();

    assert!(matches!(
        boards.delete_page(PageDeleteRequest::Confirm(confirmation)),
        PageDeleteOutcome::Rejected(PageOperationRejection::StaleConfirmation)
    ));
}

#[test]
fn board_identity_change_rejects_stale_page_delete_confirmation() {
    let mut boards = manager();
    let index = board_index(&boards, BOARD_ID_BLACKBOARD);
    boards.board_states_mut()[index].pages = two_named_pages("one", "two");
    let confirmation = page_delete_confirmation(&mut boards, BOARD_ID_BLACKBOARD, 0);

    assert!(boards.create_board());

    assert!(matches!(
        boards.delete_page(PageDeleteRequest::Confirm(confirmation)),
        PageDeleteOutcome::Rejected(PageOperationRejection::StaleConfirmation)
    ));
}

#[test]
fn page_rename_does_not_stale_page_delete_confirmation() {
    let mut boards = manager();
    let index = board_index(&boards, BOARD_ID_BLACKBOARD);
    boards.board_states_mut()[index].pages = two_named_pages("one", "two");
    let confirmation = page_delete_confirmation(&mut boards, BOARD_ID_BLACKBOARD, 0);

    assert!(
        boards.board_states_mut()[index]
            .pages
            .set_page_name(0, Some("renamed".to_string()))
    );

    assert!(matches!(
        boards.delete_page(PageDeleteRequest::Confirm(confirmation)),
        PageDeleteOutcome::Removed { .. }
    ));
}

#[test]
fn restore_rejections_return_original_requests() {
    let mut boards = manager();
    let request = BoardRestoreRequest {
        board: boards.active_board().clone(),
        preferred_index: Some(3),
    };
    boards.max_count = boards.board_count();
    let BoardRestoreOutcome::Rejected(BoardRestoreRejection::MaxCountReached { request }) =
        boards.restore_board(request)
    else {
        panic!("expected max-count rejection");
    };
    assert_eq!(request.board.spec.id, BOARD_ID_TRANSPARENT);
    assert_eq!(request.preferred_index, Some(3));

    let page_request = PageRestoreRequest {
        board_id: "missing".to_string(),
        page: Frame::new(),
        placement: PageRestorePlacement::AtIndex(99),
    };
    let PageRestoreOutcome::Rejected(PageRestoreRejection::MissingBoard { request }) =
        boards.restore_page(page_request)
    else {
        panic!("expected missing-board rejection");
    };
    assert_eq!(request.board_id, "missing");
    assert_eq!(request.placement, PageRestorePlacement::AtIndex(99));
}

#[test]
fn page_restore_at_index_clamps_to_append_and_returns_active_index() {
    let mut boards = manager();
    let PageRestoreOutcome::Restored {
        page_index,
        active_page_index,
        page_count,
        ..
    } = boards.restore_page(PageRestoreRequest {
        board_id: BOARD_ID_BLACKBOARD.to_string(),
        page: Frame::new(),
        placement: PageRestorePlacement::AtIndex(99),
    })
    else {
        panic!("expected page restore");
    };

    assert_eq!(page_index, 1);
    assert_eq!(active_page_index, 1);
    assert_eq!(page_count, 2);
}

#[test]
fn last_page_clear_returns_cleared_page() {
    let mut boards = manager();
    let index = board_index(&boards, BOARD_ID_BLACKBOARD);
    boards.board_states_mut()[index]
        .pages
        .active_frame_mut()
        .set_page_name(Some("saved".to_string()));

    let PageDeleteOutcome::ClearedLastPage { cleared_page, .. } =
        boards.delete_page(PageDeleteRequest::Request(PageDeleteTarget {
            board: PageDeleteBoardTarget::BoardId(BOARD_ID_BLACKBOARD.to_string()),
            page_index: 0,
        }))
    else {
        panic!("expected last-page clear");
    };

    assert_eq!(cleared_page.page_name(), Some("saved"));
}
