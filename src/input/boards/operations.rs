use super::{BoardIdentityGeneration, BoardManager, BoardState};
use crate::draw::Frame;

#[derive(Clone, Debug)]
pub enum BoardDeleteRequest {
    Request(BoardDeleteTarget),
    Confirm(BoardDeleteConfirmation),
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum BoardDeleteTarget {
    Active,
    BoardIndex(usize),
    BoardId(String),
}

#[derive(Clone, Debug)]
pub struct BoardDeleteConfirmation {
    pub board_id: String,
    pub board_name: String,
    pub board_identity_generation: BoardIdentityGeneration,
}

impl BoardDeleteConfirmation {
    pub fn matches_identity(&self, board_id: &str, generation: BoardIdentityGeneration) -> bool {
        self.board_id == board_id && self.board_identity_generation == generation
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
#[allow(
    clippy::large_enum_variant,
    reason = "workspace contract returns the deleted board without extra indirection"
)]
pub enum BoardDeleteOutcome {
    RequiresConfirmation {
        confirmation: BoardDeleteConfirmation,
    },
    Deleted {
        deleted_board: BoardState,
        deleted_id: String,
        deleted_name: String,
        active_id: String,
        active_index: usize,
        board_identity_generation: BoardIdentityGeneration,
    },
    Rejected(BoardDeleteRejection),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoardDeleteRejection {
    MissingBoard,
    StaleConfirmation,
    TransparentBoard,
    LastBoard,
}

#[derive(Clone, Debug)]
pub enum PageDeleteRequest {
    Request(PageDeleteTarget),
    Confirm(PageDeleteConfirmation),
}

#[derive(Clone, Debug)]
pub struct PageDeleteTarget {
    pub board: PageDeleteBoardTarget,
    pub page_index: usize,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum PageDeleteBoardTarget {
    ActiveBoard,
    BoardIndex(usize),
    BoardId(String),
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct PageDeleteConfirmation {
    pub board_id: String,
    pub board_name: String,
    pub board_identity_generation: BoardIdentityGeneration,
    pub page_index: usize,
    pub page_count: usize,
    pub page_generation: u64,
}

impl PageDeleteConfirmation {
    pub fn matches_identity(
        &self,
        board_id: &str,
        board_identity_generation: BoardIdentityGeneration,
        page_index: usize,
        page_count: usize,
        page_generation: u64,
    ) -> bool {
        self.board_id == board_id
            && self.board_identity_generation == board_identity_generation
            && self.page_index == page_index
            && self.page_count == page_count
            && self.page_generation == page_generation
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum PageDeleteOutcome {
    RequiresConfirmation {
        confirmation: PageDeleteConfirmation,
    },
    ClearedLastPage {
        board_id: String,
        board_name: String,
        page_index: usize,
        cleared_page: Frame,
        new_page_count: usize,
        new_page_generation: u64,
    },
    Removed {
        board_id: String,
        board_name: String,
        page_index: usize,
        deleted_page: Frame,
        new_page_index: usize,
        new_page_count: usize,
        new_page_generation: u64,
    },
    Rejected(PageOperationRejection),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageOperationRejection {
    MissingBoard,
    MissingPage,
    StaleConfirmation,
}

#[derive(Clone, Debug)]
pub struct BoardRestoreRequest {
    pub board: BoardState,
    pub preferred_index: Option<usize>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum BoardRestoreOutcome {
    Restored {
        restored_id: String,
        restored_name: String,
        restored_index: usize,
        active_id: String,
        active_index: usize,
        id_changed: bool,
        board_identity_generation: BoardIdentityGeneration,
    },
    Rejected(BoardRestoreRejection),
}

#[derive(Clone, Debug)]
pub enum BoardRestoreRejection {
    MaxCountReached { request: BoardRestoreRequest },
}

#[derive(Clone, Debug)]
pub struct PageRestoreRequest {
    pub board_id: String,
    pub page: Frame,
    pub placement: PageRestorePlacement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PageRestorePlacement {
    AfterActivePage,
    AtIndex(usize),
    Append,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum PageRestoreOutcome {
    Restored {
        board_id: String,
        board_name: String,
        page_index: usize,
        active_page_index: usize,
        page_count: usize,
        page_generation: u64,
    },
    Rejected(PageRestoreRejection),
}

#[derive(Clone, Debug)]
pub enum PageRestoreRejection {
    MissingBoard { request: PageRestoreRequest },
}

impl BoardManager {
    pub fn delete_board(&mut self, request: BoardDeleteRequest) -> BoardDeleteOutcome {
        match request {
            BoardDeleteRequest::Request(target) => self.request_board_delete(target),
            BoardDeleteRequest::Confirm(confirmation) => self.confirm_board_delete(confirmation),
        }
    }

    pub fn delete_page(&mut self, request: PageDeleteRequest) -> PageDeleteOutcome {
        match request {
            PageDeleteRequest::Request(target) => self.request_page_delete(target),
            PageDeleteRequest::Confirm(confirmation) => self.confirm_page_delete(confirmation),
        }
    }

    pub fn restore_board(&mut self, request: BoardRestoreRequest) -> BoardRestoreOutcome {
        if self.boards.len() >= self.max_count {
            return BoardRestoreOutcome::Rejected(BoardRestoreRejection::MaxCountReached {
                request,
            });
        }

        let original_id = request.board.spec.id.clone();
        let mut board = request.board;
        board.spec.id = self.unique_board_id(board.spec.id.clone());
        let restored_id = board.spec.id.clone();
        let restored_name = board.spec.name.clone();
        let restored_index = request
            .preferred_index
            .filter(|index| *index <= self.boards.len())
            .unwrap_or(self.boards.len());

        self.boards.insert(restored_index, board);
        self.active_index = restored_index;
        let board_identity_generation = self.bump_board_identity_generation();

        BoardRestoreOutcome::Restored {
            restored_id: restored_id.clone(),
            restored_name,
            restored_index,
            active_id: restored_id,
            active_index: restored_index,
            id_changed: original_id != self.boards[restored_index].spec.id,
            board_identity_generation,
        }
    }

    pub fn restore_page(&mut self, request: PageRestoreRequest) -> PageRestoreOutcome {
        let Some(board_index) = self.board_index_by_id(&request.board_id) else {
            return PageRestoreOutcome::Rejected(PageRestoreRejection::MissingBoard { request });
        };

        let board = &mut self.boards[board_index];
        let board_id = board.spec.id.clone();
        let board_name = board.spec.name.clone();
        let page_count = board.pages.page_count();
        let insert_at = match request.placement {
            PageRestorePlacement::AfterActivePage => {
                (board.pages.active_index() + 1).min(page_count)
            }
            PageRestorePlacement::AtIndex(index) => index.min(page_count),
            PageRestorePlacement::Append => page_count,
        };
        let page_index = board.pages.insert_page_at(insert_at, request.page);

        PageRestoreOutcome::Restored {
            board_id,
            board_name,
            page_index,
            active_page_index: board.pages.active_index(),
            page_count: board.pages.page_count(),
            page_generation: board.pages.generation(),
        }
    }

    fn request_board_delete(&self, target: BoardDeleteTarget) -> BoardDeleteOutcome {
        let Some(index) = self.board_index_for_delete_target(target) else {
            return BoardDeleteOutcome::Rejected(BoardDeleteRejection::MissingBoard);
        };
        let board = &self.boards[index];
        if board.spec.background.is_transparent() {
            return BoardDeleteOutcome::Rejected(BoardDeleteRejection::TransparentBoard);
        }
        if self.boards.len() <= 1 {
            return BoardDeleteOutcome::Rejected(BoardDeleteRejection::LastBoard);
        }
        BoardDeleteOutcome::RequiresConfirmation {
            confirmation: BoardDeleteConfirmation {
                board_id: board.spec.id.clone(),
                board_name: board.spec.name.clone(),
                board_identity_generation: self.identity_generation,
            },
        }
    }

    fn confirm_board_delete(
        &mut self,
        confirmation: BoardDeleteConfirmation,
    ) -> BoardDeleteOutcome {
        if confirmation.board_identity_generation != self.identity_generation {
            return BoardDeleteOutcome::Rejected(BoardDeleteRejection::StaleConfirmation);
        }
        let Some(index) = self.board_index_by_id(&confirmation.board_id) else {
            return BoardDeleteOutcome::Rejected(BoardDeleteRejection::MissingBoard);
        };
        let board = &self.boards[index];
        if !confirmation.matches_identity(&board.spec.id, self.identity_generation) {
            return BoardDeleteOutcome::Rejected(BoardDeleteRejection::StaleConfirmation);
        }
        if board.spec.background.is_transparent() {
            return BoardDeleteOutcome::Rejected(BoardDeleteRejection::TransparentBoard);
        }
        if self.boards.len() <= 1 {
            return BoardDeleteOutcome::Rejected(BoardDeleteRejection::LastBoard);
        }

        let deleted_board = self.boards.remove(index);
        let deleted_id = deleted_board.spec.id.clone();
        let deleted_name = deleted_board.spec.name.clone();
        if self.active_index == index {
            self.active_index = self.active_index.min(self.boards.len().saturating_sub(1));
        } else if self.active_index > index {
            self.active_index -= 1;
        }
        self.repair_default_board_after_removal(&deleted_id);
        let board_identity_generation = self.bump_board_identity_generation();
        let active_id = self.active_board_id().to_string();
        let active_index = self.active_index;

        BoardDeleteOutcome::Deleted {
            deleted_board,
            deleted_id,
            deleted_name,
            active_id,
            active_index,
            board_identity_generation,
        }
    }

    fn request_page_delete(&mut self, target: PageDeleteTarget) -> PageDeleteOutcome {
        let Some(board_index) = self.board_index_for_page_delete_target(&target.board) else {
            return PageDeleteOutcome::Rejected(PageOperationRejection::MissingBoard);
        };
        let board = &mut self.boards[board_index];
        if target.page_index >= board.pages.page_count() {
            return PageDeleteOutcome::Rejected(PageOperationRejection::MissingPage);
        }
        let board_id = board.spec.id.clone();
        let board_name = board.spec.name.clone();
        let page_count = board.pages.page_count();
        let page_generation = board.pages.generation();

        if page_count <= 1 {
            let cleared_page = board.pages.pages()[target.page_index].clone();
            let _ = board.pages.delete_page_at(target.page_index);
            return PageDeleteOutcome::ClearedLastPage {
                board_id,
                board_name,
                page_index: target.page_index,
                cleared_page,
                new_page_count: board.pages.page_count(),
                new_page_generation: board.pages.generation(),
            };
        }

        PageDeleteOutcome::RequiresConfirmation {
            confirmation: PageDeleteConfirmation {
                board_id,
                board_name,
                board_identity_generation: self.identity_generation,
                page_index: target.page_index,
                page_count,
                page_generation,
            },
        }
    }

    fn confirm_page_delete(&mut self, confirmation: PageDeleteConfirmation) -> PageDeleteOutcome {
        if confirmation.board_identity_generation != self.identity_generation {
            return PageDeleteOutcome::Rejected(PageOperationRejection::StaleConfirmation);
        }
        let Some(board_index) = self.board_index_by_id(&confirmation.board_id) else {
            return PageDeleteOutcome::Rejected(PageOperationRejection::MissingBoard);
        };
        let board = &mut self.boards[board_index];
        let page_count = board.pages.page_count();
        let page_generation = board.pages.generation();
        if !confirmation.matches_identity(
            &board.spec.id,
            self.identity_generation,
            confirmation.page_index,
            page_count,
            page_generation,
        ) {
            return PageDeleteOutcome::Rejected(PageOperationRejection::StaleConfirmation);
        }
        if confirmation.page_index >= page_count {
            return PageDeleteOutcome::Rejected(PageOperationRejection::MissingPage);
        }

        let board_id = board.spec.id.clone();
        let board_name = board.spec.name.clone();
        if page_count <= 1 {
            let cleared_page = board.pages.pages()[confirmation.page_index].clone();
            let _ = board.pages.delete_page_at(confirmation.page_index);
            return PageDeleteOutcome::ClearedLastPage {
                board_id,
                board_name,
                page_index: confirmation.page_index,
                cleared_page,
                new_page_count: board.pages.page_count(),
                new_page_generation: board.pages.generation(),
            };
        }

        let deleted_page = board.pages.pages()[confirmation.page_index].clone();
        let _ = board.pages.delete_page_at(confirmation.page_index);
        PageDeleteOutcome::Removed {
            board_id,
            board_name,
            page_index: confirmation.page_index,
            deleted_page,
            new_page_index: board.pages.active_index(),
            new_page_count: board.pages.page_count(),
            new_page_generation: board.pages.generation(),
        }
    }

    fn board_index_for_delete_target(&self, target: BoardDeleteTarget) -> Option<usize> {
        match target {
            BoardDeleteTarget::Active => Some(self.active_index),
            BoardDeleteTarget::BoardIndex(index) => self.boards.get(index).map(|_| index),
            BoardDeleteTarget::BoardId(id) => self.board_index_by_id(&id),
        }
    }

    fn board_index_for_page_delete_target(&self, target: &PageDeleteBoardTarget) -> Option<usize> {
        match target {
            PageDeleteBoardTarget::ActiveBoard => Some(self.active_index),
            PageDeleteBoardTarget::BoardIndex(index) => self.boards.get(*index).map(|_| *index),
            PageDeleteBoardTarget::BoardId(id) => self.board_index_by_id(id),
        }
    }

    fn board_index_by_id(&self, id: &str) -> Option<usize> {
        self.boards.iter().position(|board| board.spec.id == id)
    }

    fn repair_default_board_after_removal(&mut self, removed_id: &str) {
        if self.default_board_id == removed_id {
            self.default_board_id = self.boards.get(self.active_index).map_or_else(
                || super::BOARD_ID_TRANSPARENT.to_string(),
                |board| board.spec.id.clone(),
            );
        }
    }
}
