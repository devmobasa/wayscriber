use super::super::super::base::{
    InputState, PAGE_DELETE_CONFIRM_MS, PAGE_UNDO_EXPIRE_MS, PendingPageDelete,
};
use crate::domain::Action;
use crate::draw::PageDeleteOutcome as CanvasPageDeleteOutcome;
use crate::input::boards::{
    PageDeleteBoardTarget, PageDeleteOutcome, PageDeleteRequest, PageDeleteTarget,
    PageOperationRejection, PageRestoreOutcome, PageRestorePlacement, PageRestoreRejection,
    PageRestoreRequest,
};
use crate::input::state::{Toast, ToastPriority};
use std::time::{Duration, Instant};

impl InputState {
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
                self.push_toast(ToastPriority::Info, "page.delete", Toast::warning(format!(
                        "Delete page {}/{} on '{board_name}' ({board_id})? Click delete again to confirm.",
                        page_index + 1,
                        page_count
                    )).duration_ms(PAGE_DELETE_CONFIRM_MS));
                CanvasPageDeleteOutcome::Pending
            }
            PageDeleteOutcome::ClearedLastPage { .. } => {
                self.pending_page_delete = None;
                self.finish_page_delete_surface_change(is_active_board);
                self.push_toast(
                    ToastPriority::Info,
                    "page.delete",
                    Toast::info(format!("Page cleared on '{board_name}' ({board_id})")),
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
                self.push_toast(
                    ToastPriority::Info,
                    "page.delete",
                    Toast::info(format!(
                        "Page deleted on '{board_name}' ({board_id}) ({}/{})",
                        new_page_index + 1,
                        new_page_count
                    )),
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
                self.push_toast(
                    ToastPriority::Action,
                    "page.delete",
                    Toast::warning(format!(
                        "Delete page {}/{}? Click to confirm.",
                        page_index + 1,
                        page_count
                    ))
                    .action("Delete", Action::PageDelete)
                    .duration_ms(PAGE_DELETE_CONFIRM_MS),
                );
                CanvasPageDeleteOutcome::Pending
            }
            PageDeleteOutcome::ClearedLastPage { .. } => {
                self.pending_page_delete = None;
                self.finish_page_delete_surface_change(active_target);
                self.push_toast(
                    ToastPriority::Info,
                    "page.delete",
                    Toast::info("Page cleared (last page)"),
                );
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
                self.push_toast(
                    ToastPriority::Action,
                    "page.delete",
                    Toast::info(format!(
                        "Page deleted ({}/{new_page_count})",
                        new_page_index + 1
                    ))
                    .action("Undo", Action::PageRestoreDeleted),
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
            self.push_toast(
                ToastPriority::Info,
                "page.delete",
                Toast::warning("Page deletion changed; try again."),
            );
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
                    self.push_toast(
                        ToastPriority::Info,
                        "page.delete",
                        Toast::info(format!("Page restored ({}/{page_count})", page_index + 1)),
                    );
                }
                PageRestoreOutcome::Rejected(PageRestoreRejection::MissingBoard { request }) => {
                    self.deleted_pages.push((request, deleted_at));
                    self.push_toast(
                        ToastPriority::Info,
                        "page.delete",
                        Toast::warning("Board missing; cannot restore page."),
                    );
                }
            }
        } else {
            self.push_toast(
                ToastPriority::Info,
                "page.delete",
                Toast::info("No deleted page to restore."),
            );
        }
    }
}
