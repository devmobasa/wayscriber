use crate::domain::Action;
use crate::draw::PageDeleteOutcome;
use crate::input::state::{Toast, ToastPriority};
use crate::input::{BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD};
use log::info;

use super::super::InputState;

impl InputState {
    pub(in crate::input::state) fn handle_board_pages_action_with_measurer(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
        action: Action,
    ) -> bool {
        match action {
            Action::ToggleWhiteboard => {
                if self.boards.has_board(BOARD_ID_WHITEBOARD) {
                    log::info!("Toggling whiteboard board");
                    self.switch_board_with_measurer(measurer, BOARD_ID_WHITEBOARD);
                }
                true
            }
            Action::ToggleBlackboard => {
                if self.boards.has_board(BOARD_ID_BLACKBOARD) {
                    log::info!("Toggling blackboard board");
                    self.switch_board_with_measurer(measurer, BOARD_ID_BLACKBOARD);
                }
                true
            }
            Action::ReturnToTransparent => {
                if self.boards.has_board(BOARD_ID_TRANSPARENT) {
                    log::info!("Returning to transparent board");
                    self.switch_board_with_measurer(measurer, BOARD_ID_TRANSPARENT);
                }
                true
            }
            Action::PagePrev => {
                if self.page_prev_with_measurer(measurer) {
                    info!("Switched to previous page");
                } else {
                    self.push_toast(
                        ToastPriority::Info,
                        "page.nav",
                        Toast::info("Already on the first page."),
                    );
                }
                true
            }
            Action::PageNext => {
                if self.page_next_with_measurer(measurer) {
                    info!("Switched to next page");
                } else {
                    self.push_toast(
                        ToastPriority::Info,
                        "page.nav",
                        Toast::info("Already on the last page."),
                    );
                }
                true
            }
            Action::PageNew => {
                self.page_new_with_measurer(measurer);
                info!("Created new page");
                true
            }
            Action::PageDuplicate => {
                self.page_duplicate_with_measurer(measurer);
                info!("Duplicated page");
                true
            }
            Action::PageDelete => {
                let outcome = self.page_delete_with_measurer(measurer);
                if matches!(outcome, PageDeleteOutcome::Removed) {
                    info!("Deleted page");
                }
                true
            }
            Action::PageRestoreDeleted => {
                self.restore_deleted_page_with_measurer(measurer);
                true
            }
            Action::Board1 => {
                self.switch_board_slot_with_measurer(measurer, 0);
                true
            }
            Action::Board2 => {
                self.switch_board_slot_with_measurer(measurer, 1);
                true
            }
            Action::Board3 => {
                self.switch_board_slot_with_measurer(measurer, 2);
                true
            }
            Action::Board4 => {
                self.switch_board_slot_with_measurer(measurer, 3);
                true
            }
            Action::Board5 => {
                self.switch_board_slot_with_measurer(measurer, 4);
                true
            }
            Action::Board6 => {
                self.switch_board_slot_with_measurer(measurer, 5);
                true
            }
            Action::Board7 => {
                self.switch_board_slot_with_measurer(measurer, 6);
                true
            }
            Action::Board8 => {
                self.switch_board_slot_with_measurer(measurer, 7);
                true
            }
            Action::Board9 => {
                self.switch_board_slot_with_measurer(measurer, 8);
                true
            }
            Action::BoardNext => {
                self.switch_board_next_with_measurer(measurer);
                true
            }
            Action::BoardPrev => {
                self.switch_board_prev_with_measurer(measurer);
                true
            }
            Action::BoardNew => {
                if !self.create_board_with_measurer(measurer) {
                    self.push_toast(
                        ToastPriority::Info,
                        "page.nav",
                        Toast::info("Board limit reached."),
                    );
                }
                true
            }
            Action::BoardDelete => {
                self.delete_active_board_with_measurer(measurer);
                true
            }
            Action::BoardPicker => {
                self.toggle_board_picker_with_measurer(measurer);
                true
            }
            Action::BoardRestoreDeleted => {
                self.restore_deleted_board();
                true
            }
            Action::BoardDuplicate => {
                self.duplicate_board_with_measurer(measurer);
                true
            }
            Action::BoardSwitchRecent => {
                self.switch_board_recent_with_measurer(measurer);
                true
            }
            _ => false,
        }
    }
}
