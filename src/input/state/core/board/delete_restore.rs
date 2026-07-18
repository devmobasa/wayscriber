use super::super::base::{
    BOARD_DELETE_CONFIRM_MS, BOARD_UNDO_EXPIRE_MS, InputState, PendingBoardDelete, UiToastKind,
};
use crate::domain::Action;
use crate::input::boards::{
    BoardDeleteOutcome, BoardDeleteRejection, BoardDeleteRequest, BoardDeleteTarget,
    BoardIdentityGeneration, BoardRestoreOutcome, BoardRestoreRejection, BoardRestoreRequest,
};
use std::time::{Duration, Instant};

mod page;

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
        self.clear_delete_action_toast(false);
    }

    pub(crate) fn clear_session_delete_restore_state(&mut self) {
        self.pending_board_delete = None;
        self.pending_page_delete = None;
        self.deleted_boards.clear();
        self.deleted_pages.clear();
        self.clear_delete_action_toast(true);
    }

    fn clear_delete_action_toast(&mut self, include_restore_actions: bool) {
        let has_delete_action = self.ui_toast.as_ref().is_some_and(|toast| {
            toast
                .action
                .as_ref()
                .is_some_and(|action| match action.action {
                    Action::BoardDelete | Action::PageDelete => true,
                    Action::BoardRestoreDeleted | Action::PageRestoreDeleted => {
                        include_restore_actions
                    }
                    _ => false,
                })
        });
        if has_delete_action {
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
}

#[cfg(test)]
mod tests;
