use crate::config::Action;
use crate::draw::PageDeleteOutcome;
use crate::input::{BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD};
use log::info;

use super::super::{InputState, UiToastKind};

impl InputState {
    pub(super) fn handle_board_pages_action(&mut self, action: Action) -> bool {
        match action {
            Action::ToggleWhiteboard => {
                if self.boards.has_board(BOARD_ID_WHITEBOARD) {
                    log::info!("Toggling whiteboard board");
                    self.switch_board(BOARD_ID_WHITEBOARD);
                }
                true
            }
            Action::ToggleBlackboard => {
                if self.boards.has_board(BOARD_ID_BLACKBOARD) {
                    log::info!("Toggling blackboard board");
                    self.switch_board(BOARD_ID_BLACKBOARD);
                }
                true
            }
            Action::ReturnToTransparent => {
                if self.boards.has_board(BOARD_ID_TRANSPARENT) {
                    log::info!("Returning to transparent board");
                    self.switch_board(BOARD_ID_TRANSPARENT);
                }
                true
            }
            Action::PagePrev => {
                if self.page_prev() {
                    info!("Switched to previous page");
                } else {
                    self.set_ui_toast(UiToastKind::Info, "Already on the first page.");
                }
                true
            }
            Action::PageNext => {
                if self.page_next() {
                    info!("Switched to next page");
                } else {
                    self.set_ui_toast(UiToastKind::Info, "Already on the last page.");
                }
                true
            }
            Action::PageNew => {
                self.page_new();
                info!("Created new page");
                true
            }
            Action::PageDuplicate => {
                self.page_duplicate();
                info!("Duplicated page");
                true
            }
            Action::PageDelete => {
                let outcome = self.page_delete();
                if matches!(outcome, PageDeleteOutcome::Removed) {
                    info!("Deleted page");
                }
                true
            }
            Action::PageRestoreDeleted => {
                self.restore_deleted_page();
                true
            }
            Action::Board1 => {
                self.switch_board_slot(0);
                true
            }
            Action::Board2 => {
                self.switch_board_slot(1);
                true
            }
            Action::Board3 => {
                self.switch_board_slot(2);
                true
            }
            Action::Board4 => {
                self.switch_board_slot(3);
                true
            }
            Action::Board5 => {
                self.switch_board_slot(4);
                true
            }
            Action::Board6 => {
                self.switch_board_slot(5);
                true
            }
            Action::Board7 => {
                self.switch_board_slot(6);
                true
            }
            Action::Board8 => {
                self.switch_board_slot(7);
                true
            }
            Action::Board9 => {
                self.switch_board_slot(8);
                true
            }
            Action::BoardNext => {
                self.switch_board_next();
                true
            }
            Action::BoardPrev => {
                self.switch_board_prev();
                true
            }
            Action::BoardNew => {
                if !self.create_board() {
                    self.set_ui_toast(UiToastKind::Info, "Board limit reached.");
                }
                true
            }
            Action::BoardDelete => {
                self.delete_active_board();
                true
            }
            Action::BoardPicker => {
                self.toggle_board_picker();
                true
            }
            Action::BoardRestoreDeleted => {
                self.restore_deleted_board();
                true
            }
            Action::BoardDuplicate => {
                self.duplicate_board();
                true
            }
            Action::BoardSwitchRecent => {
                self.switch_board_recent();
                true
            }
            _ => false,
        }
    }
}
