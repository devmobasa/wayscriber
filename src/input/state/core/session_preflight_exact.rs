use super::base::InputState;
use super::session_preflight::board_should_persist_for_session;
use crate::draw::{BoardPages, Frame};
use crate::session::{self, BoardPagesSnapshot, BoardSnapshot, SessionOptions, SessionSnapshot};

#[derive(Debug, Clone, Copy)]
pub(super) enum ClonePreflightAction {
    PageDuplicate {
        board_index: usize,
        page_index: usize,
    },
    PageCopy {
        source_board_index: usize,
        page_index: usize,
        target_board_index: usize,
    },
    BoardDuplicate,
}

pub(super) fn exact_visible_save_allows(
    input: &InputState,
    options: &SessionOptions,
    action: ClonePreflightAction,
) -> Option<bool> {
    let mut snapshot =
        session::snapshot_from_input(input, options).unwrap_or_else(|| SessionSnapshot {
            active_board_id: input.board_id().to_string(),
            boards: Vec::new(),
            tool_state: None,
        });
    if !apply_action_to_snapshot(input, options, &mut snapshot, action) {
        return None;
    }

    let visible_can_clear = visible_without_history_is_empty(&snapshot);
    match session::estimate_snapshot_save(&snapshot, options) {
        Ok(estimate) => {
            Some(visible_can_clear || estimate.visible_without_history.limit_exceeded.is_none())
        }
        Err(err) => {
            log::warn!(
                "Session size preflight exact visible-save check failed after conservative estimate reached verification boundary: {err}"
            );
            None
        }
    }
}

fn apply_action_to_snapshot(
    input: &InputState,
    options: &SessionOptions,
    snapshot: &mut SessionSnapshot,
    action: ClonePreflightAction,
) -> bool {
    match action {
        ClonePreflightAction::PageDuplicate {
            board_index,
            page_index,
        } => duplicate_page_in_snapshot(input, options, snapshot, board_index, page_index),
        ClonePreflightAction::PageCopy {
            source_board_index,
            page_index,
            target_board_index,
        } => copy_page_between_boards_in_snapshot(
            input,
            options,
            snapshot,
            source_board_index,
            page_index,
            target_board_index,
        ),
        ClonePreflightAction::BoardDuplicate => duplicate_active_board_in_snapshot(input, snapshot),
    }
}

fn duplicate_page_in_snapshot(
    input: &InputState,
    options: &SessionOptions,
    snapshot: &mut SessionSnapshot,
    board_index: usize,
    page_index: usize,
) -> bool {
    let Some(source_board) = input.boards.board_states().get(board_index) else {
        return false;
    };
    if !board_should_persist_for_session(source_board, options) {
        return false;
    }
    let Some(cloned_page) = source_board
        .pages
        .pages()
        .get(page_index)
        .map(Frame::clone_without_history)
    else {
        return false;
    };

    if let Some(board) = snapshot
        .boards
        .iter_mut()
        .find(|board| board.id == source_board.spec.id)
    {
        let insert_at = (page_index + 1).min(board.pages.pages.len());
        board.pages.pages.insert(insert_at, cloned_page);
        board.pages.active = insert_at;
        return true;
    }

    let history_limit = options.effective_history_limit(input.undo_stack_limit);
    let mut pages = pages_for_snapshot(&source_board.pages, history_limit);
    let insert_at = (page_index + 1).min(pages.len());
    pages.insert(insert_at, cloned_page);
    snapshot.boards.push(BoardSnapshot {
        id: source_board.spec.id.clone(),
        pages: BoardPagesSnapshot {
            active: insert_at,
            pages,
        },
    });
    true
}

fn copy_page_between_boards_in_snapshot(
    input: &InputState,
    options: &SessionOptions,
    snapshot: &mut SessionSnapshot,
    source_board_index: usize,
    page_index: usize,
    target_board_index: usize,
) -> bool {
    if source_board_index == target_board_index {
        return false;
    }
    let Some(source_board) = input.boards.board_states().get(source_board_index) else {
        return false;
    };
    let Some(target_board) = input.boards.board_states().get(target_board_index) else {
        return false;
    };
    if !board_should_persist_for_session(target_board, options) {
        return false;
    }
    let Some(cloned_page) = source_board
        .pages
        .pages()
        .get(page_index)
        .map(Frame::clone_without_history)
    else {
        return false;
    };

    if let Some(target_snapshot) = snapshot
        .boards
        .iter_mut()
        .find(|board| board.id == target_board.spec.id)
    {
        let new_index = target_snapshot.pages.pages.len();
        target_snapshot.pages.pages.push(cloned_page);
        target_snapshot.pages.active = new_index;
        return true;
    }

    let history_limit = options.effective_history_limit(input.undo_stack_limit);
    let mut pages = pages_for_snapshot(&target_board.pages, history_limit);
    pages.push(cloned_page);
    snapshot.boards.push(BoardSnapshot {
        id: target_board.spec.id.clone(),
        pages: BoardPagesSnapshot {
            active: pages.len().saturating_sub(1),
            pages,
        },
    });
    true
}

fn duplicate_active_board_in_snapshot(input: &InputState, snapshot: &mut SessionSnapshot) -> bool {
    let source_board = input.boards.active_board();
    let Some(source_index) = snapshot
        .boards
        .iter()
        .position(|board| board.id == source_board.spec.id)
    else {
        return false;
    };

    let mut cloned = BoardSnapshot {
        id: duplicate_board_id_for_preflight(input, &source_board.spec.id),
        pages: snapshot.boards[source_index].pages.clone(),
    };
    cloned.pages.active = source_board.pages.active_index();
    let insert_at = (source_index + 1).min(snapshot.boards.len());
    snapshot.active_board_id = cloned.id.clone();
    snapshot.boards.insert(insert_at, cloned);
    true
}

fn pages_for_snapshot(pages: &BoardPages, history_limit: usize) -> Vec<Frame> {
    let mut cloned_pages = pages.pages().to_vec();
    for page in &mut cloned_pages {
        if history_limit == 0 {
            page.clamp_history_depth(0);
        } else if history_limit < usize::MAX {
            page.clamp_history_depth(history_limit);
        }
    }
    cloned_pages
}

fn visible_without_history_is_empty(snapshot: &SessionSnapshot) -> bool {
    snapshot.tool_state.is_none()
        && snapshot.boards.iter().all(|board| {
            let pages: Vec<_> = board
                .pages
                .pages
                .iter()
                .map(Frame::clone_without_history)
                .collect();
            board_pages_snapshot_is_empty(&pages, board.pages.active)
        })
}

fn board_pages_snapshot_is_empty(pages: &[Frame], active: usize) -> bool {
    pages.len() <= 1 && active == 0 && pages.iter().all(|page| !page.has_persistable_data())
}

pub(super) fn duplicate_board_id_for_preflight(input: &InputState, source_id: &str) -> String {
    let base = format!("{source_id}-copy");
    if !input
        .boards
        .board_states()
        .iter()
        .any(|board| board.spec.id == base)
    {
        return base;
    }

    let mut suffix = 2;
    loop {
        let candidate = format!("{base}-{suffix}");
        if !input
            .boards
            .board_states()
            .iter()
            .any(|board| board.spec.id == candidate)
        {
            return candidate;
        }
        suffix += 1;
    }
}
