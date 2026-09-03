//! Transient state around board switching, deletion confirmation, and undo.

use super::super::base::{
    BOARD_DELETE_CONFIRM_MS, BOARD_UNDO_EXPIRE_MS, PAGE_DELETE_CONFIRM_MS, PAGE_UNDO_EXPIRE_MS,
};
use crate::draw::Color;
use crate::input::boards::{
    BoardDeleteConfirmation, BoardRestoreRequest, PageDeleteConfirmation, PageRestoreRequest,
};
use std::time::{Duration, Instant};

pub(super) const BOARD_RECENT_LIMIT: usize = 5;

#[derive(Debug, Clone)]
pub(super) struct PendingBoardDelete {
    confirmation: BoardDeleteConfirmation,
    expires_at: Instant,
}

impl PendingBoardDelete {
    pub(super) fn confirmation(&self) -> &BoardDeleteConfirmation {
        &self.confirmation
    }

    #[cfg(test)]
    pub(super) fn expires_at(&self) -> Instant {
        self.expires_at
    }
}

#[derive(Debug, Clone)]
pub(super) struct PendingPageDelete {
    confirmation: PageDeleteConfirmation,
    expires_at: Instant,
}

impl PendingPageDelete {
    pub(super) fn confirmation(&self) -> &PageDeleteConfirmation {
        &self.confirmation
    }

    #[cfg(test)]
    pub(super) fn expires_at(&self) -> Instant {
        self.expires_at
    }
}

#[derive(Debug, Default)]
pub(in crate::input::state) struct BoardTransitions {
    previous_color: Option<Color>,
    recent: Vec<String>,
    pending_board_delete: Option<PendingBoardDelete>,
    pending_page_delete: Option<PendingPageDelete>,
    deleted_boards: Vec<(BoardRestoreRequest, Instant)>,
    deleted_pages: Vec<(PageRestoreRequest, Instant)>,
}

impl BoardTransitions {
    pub(in crate::input::state) fn previous_color(&self) -> Option<Color> {
        self.previous_color
    }

    pub(in crate::input::state) fn remember_previous_color(&mut self, color: Color) {
        self.previous_color = Some(color);
    }

    pub(in crate::input::state) fn set_previous_color(&mut self, color: Option<Color>) {
        self.previous_color = color;
    }

    pub(in crate::input::state) fn take_previous_color(&mut self) -> Option<Color> {
        self.previous_color.take()
    }

    pub(in crate::input::state) fn recent(&self) -> &[String] {
        &self.recent
    }

    pub(in crate::input::state) fn note_switched_to(&mut self, board_id: &str) {
        self.recent.retain(|id| id != board_id);
        self.recent.insert(0, board_id.to_string());
        self.recent.truncate(BOARD_RECENT_LIMIT);
    }

    pub(in crate::input::state) fn forget(&mut self, board_id: &str) {
        self.recent.retain(|id| id != board_id);
    }

    #[cfg(test)]
    pub(in crate::input::state) fn replace_recent_for_test(&mut self, recent: Vec<String>) {
        self.recent = recent;
    }

    pub(super) fn has_pending_board_delete(&self) -> bool {
        self.pending_board_delete.is_some()
    }

    pub(super) fn has_pending_page_delete(&self) -> bool {
        self.pending_page_delete.is_some()
    }

    pub(super) fn cancel_pending_board_delete(&mut self) -> bool {
        self.pending_board_delete.take().is_some()
    }

    pub(super) fn cancel_pending_page_delete(&mut self) -> bool {
        self.pending_page_delete.take().is_some()
    }

    pub(super) fn clear_pending_confirmations(&mut self) {
        self.pending_board_delete = None;
        self.pending_page_delete = None;
    }

    pub(super) fn clear_all(&mut self) {
        self.clear_pending_confirmations();
        self.deleted_boards.clear();
        self.deleted_pages.clear();
    }

    pub(super) fn begin_board_delete(
        &mut self,
        confirmation: BoardDeleteConfirmation,
        now: Instant,
    ) {
        self.pending_board_delete = Some(PendingBoardDelete {
            confirmation,
            expires_at: now + Duration::from_millis(BOARD_DELETE_CONFIRM_MS),
        });
    }

    pub(super) fn confirm_board_delete(&mut self, now: Instant) -> Option<PendingBoardDelete> {
        if self
            .pending_board_delete
            .as_ref()
            .is_some_and(|pending| now > pending.expires_at)
        {
            self.pending_board_delete = None;
        }
        self.pending_board_delete.take()
    }

    pub(super) fn begin_page_delete(&mut self, confirmation: PageDeleteConfirmation, now: Instant) {
        self.pending_page_delete = Some(PendingPageDelete {
            confirmation,
            expires_at: now + Duration::from_millis(PAGE_DELETE_CONFIRM_MS),
        });
    }

    pub(super) fn confirm_page_delete(&mut self, now: Instant) -> Option<PendingPageDelete> {
        self.expire_page_confirmation(now);
        self.pending_page_delete.take()
    }

    pub(super) fn confirm_page_delete_for(
        &mut self,
        now: Instant,
        board_id: &str,
        page_index: usize,
    ) -> Option<PendingPageDelete> {
        self.expire_page_confirmation(now);
        let matches = self.pending_page_delete.as_ref().is_some_and(|pending| {
            pending.confirmation.board_id == board_id
                && pending.confirmation.page_index == page_index
        });
        matches.then(|| self.pending_page_delete.take()).flatten()
    }

    fn expire_page_confirmation(&mut self, now: Instant) {
        if self
            .pending_page_delete
            .as_ref()
            .is_some_and(|pending| now > pending.expires_at)
        {
            self.pending_page_delete = None;
        }
    }

    pub(super) fn push_deleted_board(&mut self, request: BoardRestoreRequest, now: Instant) {
        self.deleted_boards.push((request, now));
    }

    pub(super) fn take_restorable_board(
        &mut self,
        now: Instant,
    ) -> Option<(BoardRestoreRequest, Instant)> {
        self.expire_boards(now);
        self.deleted_boards.pop()
    }

    pub(super) fn return_deleted_board(
        &mut self,
        request: BoardRestoreRequest,
        deleted_at: Instant,
    ) {
        self.deleted_boards.push((request, deleted_at));
    }

    pub(super) fn expire_boards(&mut self, now: Instant) -> bool {
        let before = self.deleted_boards.len();
        let expire_duration = Duration::from_millis(BOARD_UNDO_EXPIRE_MS);
        self.deleted_boards
            .retain(|(_, deleted_at)| now.saturating_duration_since(*deleted_at) < expire_duration);
        self.deleted_boards.len() != before
    }

    pub(super) fn push_deleted_page(&mut self, request: PageRestoreRequest, now: Instant) {
        self.deleted_pages.push((request, now));
    }

    pub(super) fn take_restorable_page(
        &mut self,
        now: Instant,
    ) -> Option<(PageRestoreRequest, Instant)> {
        self.expire_pages(now);
        self.deleted_pages.pop()
    }

    pub(super) fn return_deleted_page(&mut self, request: PageRestoreRequest, deleted_at: Instant) {
        self.deleted_pages.push((request, deleted_at));
    }

    pub(super) fn expire_pages(&mut self, now: Instant) -> bool {
        let before = self.deleted_pages.len();
        let expire_duration = Duration::from_millis(PAGE_UNDO_EXPIRE_MS);
        self.deleted_pages
            .retain(|(_, deleted_at)| now.saturating_duration_since(*deleted_at) < expire_duration);
        self.deleted_pages.len() != before
    }

    #[cfg(test)]
    pub(super) fn deleted_board_count(&self) -> usize {
        self.deleted_boards.len()
    }

    #[cfg(test)]
    pub(super) fn deleted_page_count(&self) -> usize {
        self.deleted_pages.len()
    }

    #[cfg(test)]
    pub(super) fn latest_deleted_board_at(&self) -> Option<Instant> {
        self.deleted_boards.last().map(|(_, at)| *at)
    }

    #[cfg(test)]
    pub(super) fn latest_deleted_page_at(&self) -> Option<Instant> {
        self.deleted_pages.last().map(|(_, at)| *at)
    }

    #[cfg(test)]
    pub(super) fn pending_board_delete_expires_at(&self) -> Option<Instant> {
        self.pending_board_delete
            .as_ref()
            .map(PendingBoardDelete::expires_at)
    }

    #[cfg(test)]
    pub(super) fn pending_page_delete_expires_at(&self) -> Option<Instant> {
        self.pending_page_delete
            .as_ref()
            .map(PendingPageDelete::expires_at)
    }
}

#[cfg(test)]
mod tests {
    use super::{BOARD_RECENT_LIMIT, BoardTransitions};
    use crate::config::BoardsConfig;
    use crate::draw::Frame;
    use crate::input::BoardManager;
    use crate::input::boards::{
        BoardDeleteConfirmation, BoardIdentityGeneration, BoardRestoreRequest,
        PageDeleteConfirmation, PageRestorePlacement, PageRestoreRequest,
    };
    use std::time::{Duration, Instant};

    fn board_confirmation(id: &str) -> BoardDeleteConfirmation {
        BoardDeleteConfirmation {
            board_id: id.to_string(),
            board_name: id.to_string(),
            board_identity_generation: BoardIdentityGeneration(1),
        }
    }

    fn page_confirmation(id: &str) -> PageDeleteConfirmation {
        PageDeleteConfirmation {
            board_id: id.to_string(),
            board_name: id.to_string(),
            board_identity_generation: BoardIdentityGeneration(1),
            page_index: 0,
            page_count: 2,
            page_generation: 1,
        }
    }

    fn board_restore_request() -> BoardRestoreRequest {
        let boards = BoardManager::from_config(BoardsConfig::default());
        BoardRestoreRequest {
            board: boards.active_board().clone(),
            preferred_index: None,
            pin_seed: false,
        }
    }

    fn page_restore_request() -> PageRestoreRequest {
        PageRestoreRequest {
            board_id: "board".to_string(),
            page: Frame::new(),
            placement: PageRestorePlacement::AfterActivePage,
        }
    }

    #[test]
    fn recents_are_unique_most_recent_first_and_capped() {
        let mut transitions = BoardTransitions::default();
        for index in 0..BOARD_RECENT_LIMIT + 2 {
            transitions.note_switched_to(&format!("board-{index}"));
        }
        transitions.note_switched_to("board-3");

        assert_eq!(transitions.recent().len(), BOARD_RECENT_LIMIT);
        assert_eq!(
            transitions.recent().first().map(String::as_str),
            Some("board-3")
        );
        assert_eq!(
            transitions
                .recent()
                .iter()
                .filter(|id| id.as_str() == "board-3")
                .count(),
            1
        );
    }

    #[test]
    fn board_delete_confirmation_inside_window_is_returned_once() {
        let mut transitions = BoardTransitions::default();
        let now = Instant::now();
        transitions.begin_board_delete(board_confirmation("board"), now);

        let pending = transitions
            .confirm_board_delete(now + Duration::from_millis(1))
            .expect("pending confirmation");

        assert_eq!(pending.confirmation().board_id, "board");
        assert!(transitions.confirm_board_delete(now).is_none());
    }

    #[test]
    fn expire_boards_drops_only_entries_past_the_deadline() {
        let mut transitions = BoardTransitions::default();
        let now = Instant::now();
        transitions.push_deleted_board(board_restore_request(), now);
        transitions.push_deleted_board(board_restore_request(), now + Duration::from_millis(10));

        assert!(
            transitions.expire_boards(now + Duration::from_millis(super::BOARD_UNDO_EXPIRE_MS + 1))
        );
        assert_eq!(transitions.deleted_board_count(), 1);
    }

    #[test]
    fn clear_all_empties_restore_queues_and_pending_confirmations() {
        let mut transitions = BoardTransitions::default();
        let now = Instant::now();
        transitions.begin_board_delete(board_confirmation("board"), now);
        transitions.begin_page_delete(page_confirmation("board"), now);
        transitions.push_deleted_board(board_restore_request(), now);
        transitions.push_deleted_page(page_restore_request(), now);

        transitions.clear_all();

        assert!(!transitions.has_pending_board_delete());
        assert!(!transitions.has_pending_page_delete());
        assert_eq!(transitions.deleted_board_count(), 0);
        assert_eq!(transitions.deleted_page_count(), 0);
    }
}
