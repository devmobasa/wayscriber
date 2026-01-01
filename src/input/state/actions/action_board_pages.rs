use crate::config::Action;
use crate::draw::PageDeleteOutcome;
use crate::input::board_mode::BoardMode;
use log::info;

use super::super::{InputState, UiToastKind};

impl InputState {
    pub(super) fn handle_board_pages_action(&mut self, action: Action) -> bool {
        match action {
            Action::ToggleWhiteboard => {
                if self.board_config.enabled {
                    log::info!("Toggling whiteboard mode");
                    self.switch_board_mode(BoardMode::Whiteboard);
                }
                true
            }
            Action::ToggleBlackboard => {
                if self.board_config.enabled {
                    log::info!("Toggling blackboard mode");
                    self.switch_board_mode(BoardMode::Blackboard);
                }
                true
            }
            Action::ReturnToTransparent => {
                if self.board_config.enabled {
                    log::info!("Returning to transparent mode");
                    self.switch_board_mode(BoardMode::Transparent);
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
                match self.page_delete() {
                    PageDeleteOutcome::Removed => {
                        info!("Deleted page");
                    }
                    PageDeleteOutcome::Cleared => {
                        self.set_ui_toast(UiToastKind::Info, "Cleared the last page.");
                    }
                }
                true
            }
            _ => false,
        }
    }
}
