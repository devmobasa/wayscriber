use super::super::base::{
    BOARD_DELETE_CONFIRM_MS, BOARD_UNDO_EXPIRE_MS, InputState, PAGE_DELETE_CONFIRM_MS,
    PAGE_UNDO_EXPIRE_MS, PendingBoardDelete, PendingPageDelete, UiToastKind,
};
use crate::config::Action;
use crate::draw::PageDeleteOutcome as CanvasPageDeleteOutcome;
use crate::input::boards::{
    BoardDeleteOutcome, BoardDeleteRejection, BoardDeleteRequest, BoardDeleteTarget,
    BoardIdentityGeneration, BoardRestoreOutcome, BoardRestoreRejection, BoardRestoreRequest,
    PageDeleteBoardTarget, PageDeleteOutcome, PageDeleteRequest, PageDeleteTarget,
    PageOperationRejection, PageRestoreOutcome, PageRestorePlacement, PageRestoreRejection,
    PageRestoreRequest,
};
use std::time::{Duration, Instant};

impl InputState {
    /// Returns true if there's a pending board deletion confirmation.
    pub fn has_pending_board_delete(&self) -> bool {
        self.pending_board_delete.is_some()
    }

    /// Cancels pending board deletion and clears the toast.
    pub fn cancel_pending_board_delete(&mut self) {
        if self.pending_board_delete.is_some() {
            self.pending_board_delete = None;
            self.ui_toast = None;
            self.set_ui_toast(UiToastKind::Info, "Board deletion cancelled.");
        }
    }

    /// Returns true if there's a pending page deletion confirmation.
    pub fn has_pending_page_delete(&self) -> bool {
        self.pending_page_delete.is_some()
    }

    /// Cancels pending page deletion and clears the toast.
    pub fn cancel_pending_page_delete(&mut self) {
        if self.pending_page_delete.is_some() {
            self.pending_page_delete = None;
            self.ui_toast = None;
            self.set_ui_toast(UiToastKind::Info, "Page deletion cancelled.");
        }
    }

    pub(crate) fn clear_pending_delete_confirmations(&mut self) {
        self.pending_board_delete = None;
        self.pending_page_delete = None;
        if self.ui_toast.as_ref().is_some_and(|toast| {
            toast.action.as_ref().is_some_and(|action| {
                matches!(action.action, Action::BoardDelete | Action::PageDelete)
            })
        }) {
            self.ui_toast = None;
            self.ui_toast_bounds = None;
            self.needs_redraw = true;
        }
    }

    pub fn delete_active_board(&mut self) {
        self.delete_active_board_at(Instant::now());
    }

    pub(crate) fn delete_active_board_at(&mut self, now: Instant) {
        if self
            .pending_board_delete
            .as_ref()
            .is_some_and(|pending| now > pending.expires_at)
        {
            self.pending_board_delete = None;
        }

        let request = self
            .pending_board_delete
            .as_ref()
            .map(|pending| BoardDeleteRequest::Confirm(pending.confirmation.clone()))
            .unwrap_or(BoardDeleteRequest::Request(BoardDeleteTarget::Active));

        let active_id_before = self.boards.active_board_id().to_string();
        let current_spec = self.boards.active_board().spec.clone();
        let generation_before = self.boards.board_identity_generation();
        let deleted_index = match &request {
            BoardDeleteRequest::Confirm(confirmation) => self
                .boards
                .board_states()
                .iter()
                .position(|board| board.spec.id == confirmation.board_id),
            BoardDeleteRequest::Request(BoardDeleteTarget::Active) => {
                Some(self.boards.active_index())
            }
            BoardDeleteRequest::Request(BoardDeleteTarget::BoardIndex(index)) => Some(*index),
            BoardDeleteRequest::Request(BoardDeleteTarget::BoardId(id)) => self
                .boards
                .board_states()
                .iter()
                .position(|board| board.spec.id == *id),
        };
        let deleting_active = match &request {
            BoardDeleteRequest::Confirm(confirmation) => confirmation.board_id == active_id_before,
            _ => true,
        };
        if deleting_active && matches!(request, BoardDeleteRequest::Confirm(_)) {
            self.cancel_active_interaction();
        }

        match self.boards.delete_board(request) {
            BoardDeleteOutcome::RequiresConfirmation { confirmation } => {
                let name = confirmation.board_name.clone();
                self.pending_board_delete = Some(PendingBoardDelete {
                    confirmation,
                    expires_at: now + Duration::from_millis(BOARD_DELETE_CONFIRM_MS),
                });
                self.set_ui_toast_with_action_and_duration(
                    UiToastKind::Warning,
                    format!("Delete board '{name}'? Click to confirm."),
                    "Delete",
                    Action::BoardDelete,
                    BOARD_DELETE_CONFIRM_MS,
                );
            }
            BoardDeleteOutcome::Deleted {
                deleted_board,
                deleted_id,
                deleted_name,
                ..
            } => {
                self.pending_board_delete = None;
                self.clear_pending_deletes_after_board_generation_change(generation_before);
                self.remove_board_recent(&deleted_id);
                self.queue_board_config_save();
                self.deleted_boards.push((
                    BoardRestoreRequest {
                        board: deleted_board,
                        preferred_index: deleted_index,
                    },
                    now,
                ));
                self.set_ui_toast_with_action(
                    UiToastKind::Info,
                    format!("Board deleted: {deleted_name}"),
                    "Undo",
                    Action::BoardRestoreDeleted,
                );

                if self.boards.active_board_id() != active_id_before {
                    self.finish_board_transition_from(current_spec, &active_id_before, false);
                } else {
                    self.mark_board_surface_changed();
                }
            }
            BoardDeleteOutcome::Rejected(rejection) => {
                self.pending_board_delete = None;
                self.set_board_delete_rejection_toast(rejection);
            }
        }
    }

    fn set_board_delete_rejection_toast(&mut self, rejection: BoardDeleteRejection) {
        match rejection {
            BoardDeleteRejection::MissingBoard | BoardDeleteRejection::StaleConfirmation => {
                self.set_ui_toast(UiToastKind::Warning, "Board deletion changed; try again.");
            }
            BoardDeleteRejection::TransparentBoard => {
                self.set_ui_toast(UiToastKind::Info, "Overlay board cannot be deleted.");
            }
            BoardDeleteRejection::LastBoard => {
                self.set_ui_toast(UiToastKind::Info, "At least one board must remain.");
            }
        }
    }

    pub(crate) fn clear_pending_deletes_after_board_generation_change(
        &mut self,
        before: BoardIdentityGeneration,
    ) {
        if self.boards.board_identity_generation() != before {
            self.clear_pending_delete_confirmations();
        }
    }

    /// Restore the most recently deleted board.
    pub fn restore_deleted_board(&mut self) {
        self.restore_deleted_board_at(Instant::now());
    }

    pub(crate) fn restore_deleted_board_at(&mut self, now: Instant) {
        // Expire old entries first
        self.expire_deleted_boards_at(now);

        let Some((request, timestamp)) = self.deleted_boards.pop() else {
            self.set_ui_toast(UiToastKind::Info, "No deleted board to restore.");
            return;
        };

        let current_id = self.boards.active_board_id().to_string();
        let current_spec = self.boards.active_board().spec.clone();
        let generation_before = self.boards.board_identity_generation();

        match self.boards.restore_board(request) {
            BoardRestoreOutcome::Restored { restored_name, .. } => {
                self.clear_pending_deletes_after_board_generation_change(generation_before);
                self.queue_board_config_save();
                self.finish_board_transition_from(current_spec, &current_id, false);
                self.set_ui_toast(
                    UiToastKind::Info,
                    format!("Board restored: {restored_name}"),
                );
            }
            BoardRestoreOutcome::Rejected(BoardRestoreRejection::MaxCountReached { request }) => {
                self.deleted_boards.push((request, timestamp));
                self.set_ui_toast(UiToastKind::Warning, "Board limit reached; cannot restore.");
            }
        }
    }

    /// Remove deleted boards that have expired (older than BOARD_UNDO_EXPIRE_MS).
    fn expire_deleted_boards_at(&mut self, now: Instant) {
        let expire_duration = std::time::Duration::from_millis(BOARD_UNDO_EXPIRE_MS);
        self.deleted_boards.retain(|(_request, timestamp)| {
            now.saturating_duration_since(*timestamp) < expire_duration
        });
    }

    pub(crate) fn delete_page_in_board(
        &mut self,
        board_index: usize,
        page_index: usize,
    ) -> CanvasPageDeleteOutcome {
        self.delete_page_in_board_at(board_index, page_index, Instant::now())
    }

    pub(crate) fn delete_page_in_board_at(
        &mut self,
        board_index: usize,
        page_index: usize,
        now: Instant,
    ) -> CanvasPageDeleteOutcome {
        let is_active_board = self.boards.active_index() == board_index;
        let Some(board) = self.boards.board_states().get(board_index) else {
            return CanvasPageDeleteOutcome::Pending;
        };
        let page_count = board.pages.page_count();
        if page_index >= page_count {
            return CanvasPageDeleteOutcome::Pending;
        }
        let board_name = board.spec.name.clone();
        let board_id = board.spec.id.clone();

        if self
            .pending_page_delete
            .as_ref()
            .is_some_and(|pending| now > pending.expires_at)
        {
            self.pending_page_delete = None;
        }

        let request = self
            .pending_page_delete
            .as_ref()
            .filter(|pending| {
                pending.confirmation.board_id == board_id
                    && pending.confirmation.page_index == page_index
            })
            .map(|pending| PageDeleteRequest::Confirm(pending.confirmation.clone()))
            .unwrap_or_else(|| {
                PageDeleteRequest::Request(PageDeleteTarget {
                    board: PageDeleteBoardTarget::BoardIndex(board_index),
                    page_index,
                })
            });
        let confirmation_is_current = matches!(&request, PageDeleteRequest::Confirm(confirmation) if self.page_delete_confirmation_is_current(confirmation));
        let should_prepare_active = is_active_board
            && ((matches!(&request, PageDeleteRequest::Request(_)) && page_count <= 1)
                || confirmation_is_current);
        if should_prepare_active {
            self.prepare_active_page_content_change();
        }

        match self.boards.delete_page(request) {
            PageDeleteOutcome::RequiresConfirmation { confirmation } => {
                self.pending_page_delete = Some(PendingPageDelete {
                    confirmation,
                    expires_at: now + Duration::from_millis(PAGE_DELETE_CONFIRM_MS),
                });
                self.set_ui_toast_with_duration(
                    UiToastKind::Warning,
                    format!(
                        "Delete page {}/{} on '{board_name}' ({board_id})? Click delete again to confirm.",
                        page_index + 1,
                        page_count
                    ),
                    PAGE_DELETE_CONFIRM_MS,
                );
                CanvasPageDeleteOutcome::Pending
            }
            PageDeleteOutcome::ClearedLastPage { .. } => {
                self.pending_page_delete = None;
                self.finish_page_delete_surface_change(is_active_board);
                self.set_ui_toast(
                    UiToastKind::Info,
                    format!("Page cleared on '{board_name}' ({board_id})"),
                );
                CanvasPageDeleteOutcome::Cleared
            }
            PageDeleteOutcome::Removed {
                new_page_index,
                new_page_count,
                ..
            } => {
                self.pending_page_delete = None;
                self.finish_page_delete_surface_change(is_active_board);
                self.set_ui_toast(
                    UiToastKind::Info,
                    format!(
                        "Page deleted on '{board_name}' ({board_id}) ({}/{})",
                        new_page_index + 1,
                        new_page_count
                    ),
                );
                CanvasPageDeleteOutcome::Removed
            }
            PageDeleteOutcome::Rejected(rejection) => {
                self.pending_page_delete = None;
                self.set_page_delete_rejection_toast(rejection);
                CanvasPageDeleteOutcome::Pending
            }
        }
    }

    pub fn page_delete(&mut self) -> CanvasPageDeleteOutcome {
        self.delete_active_page_at(Instant::now())
    }

    pub(crate) fn delete_active_page_at(&mut self, now: Instant) -> CanvasPageDeleteOutcome {
        let page_count = self.boards.page_count();
        let page_index = self.boards.active_page_index();

        if self
            .pending_page_delete
            .as_ref()
            .is_some_and(|pending| now > pending.expires_at)
        {
            self.pending_page_delete = None;
        }

        let request = self
            .pending_page_delete
            .as_ref()
            .map(|pending| PageDeleteRequest::Confirm(pending.confirmation.clone()))
            .unwrap_or_else(|| {
                PageDeleteRequest::Request(PageDeleteTarget {
                    board: PageDeleteBoardTarget::ActiveBoard,
                    page_index,
                })
            });
        let active_target = match &request {
            PageDeleteRequest::Confirm(confirmation) => {
                confirmation.board_id == self.boards.active_board_id()
            }
            PageDeleteRequest::Request(_) => true,
        };
        let confirmation_is_current = matches!(&request, PageDeleteRequest::Confirm(confirmation) if self.page_delete_confirmation_is_current(confirmation));
        let should_prepare_active = active_target
            && ((matches!(&request, PageDeleteRequest::Request(_)) && page_count <= 1)
                || confirmation_is_current);
        if should_prepare_active {
            self.prepare_active_page_content_change();
        }

        match self.boards.delete_page(request) {
            PageDeleteOutcome::RequiresConfirmation { confirmation } => {
                self.pending_page_delete = Some(PendingPageDelete {
                    confirmation,
                    expires_at: now + Duration::from_millis(PAGE_DELETE_CONFIRM_MS),
                });
                self.set_ui_toast_with_action_and_duration(
                    UiToastKind::Warning,
                    format!(
                        "Delete page {}/{}? Click to confirm.",
                        page_index + 1,
                        page_count
                    ),
                    "Delete",
                    Action::PageDelete,
                    PAGE_DELETE_CONFIRM_MS,
                );
                CanvasPageDeleteOutcome::Pending
            }
            PageDeleteOutcome::ClearedLastPage { .. } => {
                self.pending_page_delete = None;
                self.finish_page_delete_surface_change(active_target);
                self.set_ui_toast(UiToastKind::Info, "Page cleared (last page)");
                CanvasPageDeleteOutcome::Cleared
            }
            PageDeleteOutcome::Removed {
                board_id,
                deleted_page,
                new_page_index,
                new_page_count,
                ..
            } => {
                self.pending_page_delete = None;
                self.finish_page_delete_surface_change(active_target);
                self.deleted_pages.push((
                    PageRestoreRequest {
                        board_id,
                        page: deleted_page,
                        placement: PageRestorePlacement::AfterActivePage,
                    },
                    now,
                ));
                self.set_ui_toast_with_action(
                    UiToastKind::Info,
                    format!("Page deleted ({}/{new_page_count})", new_page_index + 1),
                    "Undo",
                    Action::PageRestoreDeleted,
                );
                CanvasPageDeleteOutcome::Removed
            }
            PageDeleteOutcome::Rejected(rejection) => {
                self.pending_page_delete = None;
                self.set_page_delete_rejection_toast(rejection);
                CanvasPageDeleteOutcome::Pending
            }
        }
    }

    fn finish_page_delete_surface_change(&mut self, active_target: bool) {
        if active_target {
            self.finish_active_page_content_change();
        } else {
            self.mark_board_surface_changed();
        }
    }

    fn page_delete_confirmation_is_current(
        &self,
        confirmation: &crate::input::boards::PageDeleteConfirmation,
    ) -> bool {
        if confirmation.board_identity_generation != self.boards.board_identity_generation() {
            return false;
        }
        self.boards
            .board_states()
            .iter()
            .find(|board| board.spec.id == confirmation.board_id)
            .is_some_and(|board| {
                confirmation.matches_identity(
                    &board.spec.id,
                    self.boards.board_identity_generation(),
                    confirmation.page_index,
                    board.pages.page_count(),
                    board.pages.generation(),
                ) && confirmation.page_index < board.pages.page_count()
            })
    }

    fn set_page_delete_rejection_toast(&mut self, rejection: PageOperationRejection) {
        if matches!(rejection, PageOperationRejection::StaleConfirmation) {
            self.set_ui_toast(UiToastKind::Warning, "Page deletion changed; try again.");
        }
    }

    /// Restore the most recently deleted page.
    pub fn restore_deleted_page(&mut self) {
        self.restore_deleted_page_at(Instant::now());
    }

    pub(crate) fn restore_deleted_page_at(&mut self, now: Instant) {
        // Expire old entries
        let expire_duration = Duration::from_millis(PAGE_UNDO_EXPIRE_MS);
        self.deleted_pages
            .retain(|(_, deleted_at)| now.saturating_duration_since(*deleted_at) < expire_duration);

        if let Some((request, deleted_at)) = self.deleted_pages.pop() {
            let active_target = request.board_id == self.boards.active_board_id();
            if active_target {
                self.prepare_active_page_content_change();
            }
            match self.boards.restore_page(request) {
                PageRestoreOutcome::Restored {
                    page_index,
                    page_count,
                    ..
                } => {
                    if active_target {
                        self.finish_active_page_content_change();
                    } else {
                        self.mark_board_surface_changed();
                    }
                    self.set_ui_toast(
                        UiToastKind::Info,
                        format!("Page restored ({}/{page_count})", page_index + 1),
                    );
                }
                PageRestoreOutcome::Rejected(PageRestoreRejection::MissingBoard { request }) => {
                    self.deleted_pages.push((request, deleted_at));
                    self.set_ui_toast(UiToastKind::Warning, "Board missing; cannot restore page.");
                }
            }
        } else {
            self.set_ui_toast(UiToastKind::Info, "No deleted page to restore.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::Frame;
    use crate::input::BOARD_ID_BLACKBOARD;
    use crate::input::state::test_support::make_test_input_state;

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

        state.restore_deleted_board_at(
            confirmed_at + Duration::from_millis(BOARD_UNDO_EXPIRE_MS + 1),
        );

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

        state
            .restore_deleted_page_at(confirmed_at + Duration::from_millis(PAGE_UNDO_EXPIRE_MS + 1));

        assert!(state.deleted_pages.is_empty());
        assert_eq!(state.boards.page_count(), page_count_after_delete);
        assert_eq!(
            state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
            Some("No deleted page to restore.")
        );
    }
}
