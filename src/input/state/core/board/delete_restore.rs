use super::super::base::{
    BOARD_DELETE_CONFIRM_MS, BOARD_UNDO_EXPIRE_MS, InputState, PAGE_DELETE_CONFIRM_MS,
    PAGE_UNDO_EXPIRE_MS, PendingBoardDelete, PendingPageDelete, UiToastKind,
};
use crate::config::Action;
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

    pub fn delete_active_board(&mut self) {
        if self.board_is_transparent() {
            self.set_ui_toast(UiToastKind::Info, "Overlay board cannot be deleted.");
            self.pending_board_delete = None;
            return;
        }
        if self.boards.board_count() <= 1 {
            self.set_ui_toast(UiToastKind::Info, "At least one board must remain.");
            self.pending_board_delete = None;
            return;
        }

        let current_id = self.boards.active_board_id().to_string();
        let now = Instant::now();
        if self
            .pending_board_delete
            .as_ref()
            .is_some_and(|pending| now > pending.expires_at)
        {
            self.pending_board_delete = None;
        }
        let confirmed = self
            .pending_board_delete
            .as_ref()
            .is_some_and(|pending| pending.board_id == current_id && now <= pending.expires_at);
        if !confirmed {
            let name = self.boards.active_board_name();
            self.pending_board_delete = Some(PendingBoardDelete {
                board_id: current_id,
                expires_at: now + Duration::from_millis(BOARD_DELETE_CONFIRM_MS),
            });
            self.set_ui_toast_with_action_and_duration(
                UiToastKind::Warning,
                format!("Delete board '{name}'? Click to confirm."),
                "Delete",
                Action::BoardDelete,
                BOARD_DELETE_CONFIRM_MS,
            );
            return;
        }
        self.pending_board_delete = None;

        // Save board state before deletion for potential undo
        let deleted_board = self.boards.active_board().clone();
        let deleted_name = deleted_board.spec.name.clone();

        if self.switch_board_with(|boards| boards.remove_active_board(), &current_id) {
            self.remove_board_recent(&current_id);
            self.queue_board_config_save();

            // Store for undo
            self.deleted_boards.push((deleted_board, Instant::now()));

            // Show toast with undo action
            self.set_ui_toast_with_action(
                UiToastKind::Info,
                format!("Board deleted: {deleted_name}"),
                "Undo",
                Action::BoardRestoreDeleted,
            );
        }
    }

    /// Restore the most recently deleted board.
    pub fn restore_deleted_board(&mut self) {
        // Expire old entries first
        self.expire_deleted_boards();

        let Some((board, _timestamp)) = self.deleted_boards.pop() else {
            self.set_ui_toast(UiToastKind::Info, "No deleted board to restore.");
            return;
        };

        let name = board.spec.name.clone();
        let current_id = self.boards.active_board_id().to_string();

        if self.switch_board_with(
            |boards| boards.insert_board(boards.board_count(), board),
            &current_id,
        ) {
            self.queue_board_config_save();
            self.set_ui_toast(UiToastKind::Info, format!("Board restored: {name}"));
        } else {
            self.set_ui_toast(UiToastKind::Warning, "Board limit reached; cannot restore.");
        }
    }

    /// Remove deleted boards that have expired (older than BOARD_UNDO_EXPIRE_MS).
    fn expire_deleted_boards(&mut self) {
        let now = Instant::now();
        let expire_duration = std::time::Duration::from_millis(BOARD_UNDO_EXPIRE_MS);
        self.deleted_boards.retain(|(_board, timestamp)| {
            now.saturating_duration_since(*timestamp) < expire_duration
        });
    }

    pub(crate) fn delete_page_in_board(
        &mut self,
        board_index: usize,
        page_index: usize,
    ) -> crate::draw::PageDeleteOutcome {
        let Some(board) = self.boards.board_state_mut(board_index) else {
            return crate::draw::PageDeleteOutcome::Pending;
        };
        let page_count = board.pages.page_count();
        if page_index >= page_count {
            return crate::draw::PageDeleteOutcome::Pending;
        }
        let board_name = board.spec.name.clone();
        let board_id = board.spec.id.clone();

        if page_count <= 1 {
            let outcome = board.pages.delete_page_at(page_index);
            if self.boards.active_index() == board_index {
                self.prepare_page_switch();
            } else {
                self.dirty_tracker.mark_full();
                self.needs_redraw = true;
                self.mark_session_dirty();
            }
            self.set_ui_toast(
                UiToastKind::Info,
                format!("Page cleared on '{board_name}' ({board_id})"),
            );
            return outcome;
        }

        let now = Instant::now();
        if self
            .pending_page_delete
            .as_ref()
            .is_some_and(|pending| now > pending.expires_at)
        {
            self.pending_page_delete = None;
        }
        let confirmed = self.pending_page_delete.as_ref().is_some_and(|pending| {
            pending.board_id == board_id
                && pending.page_index == page_index
                && now <= pending.expires_at
        });
        if !confirmed {
            self.pending_page_delete = Some(PendingPageDelete {
                board_id: board_id.clone(),
                page_index,
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
            return crate::draw::PageDeleteOutcome::Pending;
        }

        self.pending_page_delete = None;
        let outcome = board.pages.delete_page_at(page_index);
        let new_page_num = board.pages.active_index() + 1;
        let new_page_count = board.pages.page_count();
        if self.boards.active_index() == board_index {
            self.prepare_page_switch();
        } else {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
            self.mark_session_dirty();
        }
        if matches!(outcome, crate::draw::PageDeleteOutcome::Removed) {
            self.set_ui_toast(
                UiToastKind::Info,
                format!(
                    "Page deleted on '{board_name}' ({board_id}) ({new_page_num}/{new_page_count})"
                ),
            );
        }
        outcome
    }

    pub fn page_delete(&mut self) -> crate::draw::PageDeleteOutcome {
        let page_count = self.boards.page_count();
        let page_index = self.boards.active_page_index();
        let board_id = self.boards.active_board_id().to_string();

        // If this is the last page, skip confirmation (just clears)
        if page_count <= 1 {
            let outcome = self.boards.delete_page();
            self.prepare_page_switch();
            self.set_ui_toast(UiToastKind::Info, "Page cleared (last page)");
            return outcome;
        }

        let now = Instant::now();

        // Expire old pending confirmation
        if self
            .pending_page_delete
            .as_ref()
            .is_some_and(|pending| now > pending.expires_at)
        {
            self.pending_page_delete = None;
        }

        // Check if we have a valid pending confirmation
        let confirmed = self.pending_page_delete.as_ref().is_some_and(|pending| {
            pending.board_id == board_id
                && pending.page_index == page_index
                && now <= pending.expires_at
        });

        if !confirmed {
            // First press: request confirmation
            self.pending_page_delete = Some(PendingPageDelete {
                board_id,
                page_index,
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
            return crate::draw::PageDeleteOutcome::Pending;
        }

        // Confirmed: proceed with deletion
        self.pending_page_delete = None;

        // Save page before deletion for undo
        let deleted_page = self.boards.active_pages().active_frame().clone();

        let outcome = self.boards.delete_page();
        self.prepare_page_switch();
        let new_page_num = self.boards.active_page_index() + 1;
        let new_page_count = self.boards.page_count();

        if matches!(outcome, crate::draw::PageDeleteOutcome::Removed) {
            // Store for undo
            let board_id = self.boards.active_board_id().to_string();
            self.deleted_pages
                .push((board_id, deleted_page, Instant::now()));

            // Show toast with undo action
            self.set_ui_toast_with_action(
                UiToastKind::Info,
                format!("Page deleted ({new_page_num}/{new_page_count})"),
                "Undo",
                Action::PageRestoreDeleted,
            );
        }
        outcome
    }

    /// Restore the most recently deleted page.
    pub fn restore_deleted_page(&mut self) {
        // Expire old entries
        let now = Instant::now();
        let expire_duration = Duration::from_millis(PAGE_UNDO_EXPIRE_MS);
        self.deleted_pages.retain(|(_, _, deleted_at)| {
            now.saturating_duration_since(*deleted_at) < expire_duration
        });

        // Get the most recent deleted page for the current board
        let board_id = self.boards.active_board_id().to_string();
        let position = self
            .deleted_pages
            .iter()
            .rposition(|(id, _, _)| id == &board_id);

        if let Some(idx) = position {
            let (_, page, _) = self.deleted_pages.remove(idx);
            self.boards.insert_page(page);
            self.prepare_page_switch();
            let page_num = self.boards.active_page_index() + 1;
            let page_count = self.boards.page_count();
            self.set_ui_toast(
                UiToastKind::Info,
                format!("Page restored ({page_num}/{page_count})"),
            );
        } else {
            self.set_ui_toast(UiToastKind::Info, "No deleted page to restore.");
        }
    }
}
