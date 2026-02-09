use super::{BOARD_ID_TRANSPARENT, BoardBackground, BoardManager, BoardSpec, BoardState};
use crate::draw::{BoardPages, Frame, PageDeleteOutcome};

impl BoardManager {
    pub fn board_count(&self) -> usize {
        self.boards.len()
    }

    pub fn active_index(&self) -> usize {
        self.active_index
    }

    pub fn active_board(&self) -> &BoardState {
        &self.boards[self.active_index]
    }

    pub fn active_board_mut(&mut self) -> &mut BoardState {
        &mut self.boards[self.active_index]
    }

    pub fn active_board_id(&self) -> &str {
        &self.active_board().spec.id
    }

    pub fn has_board(&self, id: &str) -> bool {
        self.boards.iter().any(|board| board.spec.id == id)
    }

    pub fn active_board_name(&self) -> &str {
        &self.active_board().spec.name
    }

    pub fn active_background(&self) -> &BoardBackground {
        &self.active_board().spec.background
    }

    pub fn active_pages(&self) -> &BoardPages {
        &self.active_board().pages
    }

    pub fn active_pages_mut(&mut self) -> &mut BoardPages {
        &mut self.active_board_mut().pages
    }

    pub fn active_frame(&self) -> &Frame {
        self.active_pages().active_frame()
    }

    pub fn active_frame_mut(&mut self) -> &mut Frame {
        self.active_pages_mut().active_frame_mut()
    }

    pub fn show_badge(&self) -> bool {
        self.show_badge
    }

    pub fn max_count(&self) -> usize {
        self.max_count
    }

    pub fn persist_customizations(&self) -> bool {
        self.persist_customizations
    }

    #[allow(dead_code)]
    pub fn default_board_id(&self) -> &str {
        &self.default_board_id
    }

    pub fn page_count(&self) -> usize {
        self.active_pages().page_count()
    }

    pub fn active_page_index(&self) -> usize {
        self.active_pages().active_index()
    }

    pub fn next_page(&mut self) -> bool {
        self.active_pages_mut().next_page()
    }

    pub fn prev_page(&mut self) -> bool {
        self.active_pages_mut().prev_page()
    }

    pub fn new_page(&mut self) {
        self.active_pages_mut().new_page();
    }

    pub fn duplicate_page(&mut self) {
        self.active_pages_mut().duplicate_page();
    }

    pub fn insert_page(&mut self, page: Frame) {
        self.active_pages_mut().insert_page(page);
    }

    pub fn delete_page(&mut self) -> PageDeleteOutcome {
        self.active_pages_mut().delete_page()
    }

    pub fn next_board(&mut self) -> bool {
        if self.boards.is_empty() {
            return false;
        }
        let next = (self.active_index + 1) % self.boards.len();
        self.active_index = next;
        true
    }

    pub fn prev_board(&mut self) -> bool {
        if self.boards.is_empty() {
            return false;
        }
        let prev = if self.active_index == 0 {
            self.boards.len() - 1
        } else {
            self.active_index - 1
        };
        self.active_index = prev;
        true
    }

    #[allow(dead_code)]
    pub fn board_specs(&self) -> impl Iterator<Item = &BoardSpec> {
        self.boards.iter().map(|board| &board.spec)
    }

    pub fn board_states_mut(&mut self) -> &mut [BoardState] {
        &mut self.boards
    }

    pub fn board_states(&self) -> &[BoardState] {
        &self.boards
    }

    pub fn board_state_mut(&mut self, index: usize) -> Option<&mut BoardState> {
        self.boards.get_mut(index)
    }

    #[allow(dead_code)]
    pub fn board_state_by_id_mut(&mut self, id: &str) -> Option<&mut BoardState> {
        self.boards.iter_mut().find(|board| board.spec.id == id)
    }

    pub fn remove_active_board(&mut self) -> bool {
        if self.boards.len() <= 1 {
            return false;
        }
        let removed_id = self.boards[self.active_index].spec.id.clone();
        self.boards.remove(self.active_index);
        if self.active_index >= self.boards.len() {
            self.active_index = self.boards.len().saturating_sub(1);
        }
        if self.default_board_id == removed_id {
            self.default_board_id = self.boards.get(self.active_index).map_or_else(
                || BOARD_ID_TRANSPARENT.to_string(),
                |board| board.spec.id.clone(),
            );
        }
        true
    }

    pub fn move_board(&mut self, from: usize, to: usize) -> bool {
        let len = self.boards.len();
        if from >= len || to >= len || from == to {
            return false;
        }
        let active_id = self.active_board_id().to_string();
        let board = self.boards.remove(from);
        self.boards.insert(to, board);
        if let Some(index) = self.boards.iter().position(|b| b.spec.id == active_id) {
            self.active_index = index;
        } else {
            self.active_index = self.active_index.min(self.boards.len().saturating_sub(1));
        }
        true
    }

    pub fn set_board_pages(&mut self, id: &str, pages: BoardPages) -> bool {
        if let Some(board) = self.ensure_board(id) {
            board.pages = pages;
            return true;
        }
        false
    }

    /// Insert a board at the given index.
    /// Returns true if successful, false if the board limit is reached.
    pub fn insert_board(&mut self, index: usize, board: BoardState) -> bool {
        if self.boards.len() >= self.max_count {
            return false;
        }
        let insert_at = index.min(self.boards.len());
        self.boards.insert(insert_at, board);
        self.active_index = insert_at;
        true
    }
}
