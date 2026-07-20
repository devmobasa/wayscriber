use crate::input::InputState;
use crate::input::state::{Toast, ToastPriority};

impl InputState {
    pub(super) fn apply_toolbar_board_prev(&mut self) -> bool {
        self.switch_board_prev();
        true
    }

    pub(super) fn apply_toolbar_board_next(&mut self) -> bool {
        self.switch_board_next();
        true
    }

    pub(super) fn apply_toolbar_board_new(&mut self) -> bool {
        if self.create_board() {
            true
        } else {
            self.push_toast(
                ToastPriority::Info,
                "board.switch",
                Toast::info("Board limit reached."),
            );
            false
        }
    }

    pub(super) fn apply_toolbar_board_delete(&mut self) -> bool {
        self.delete_active_board();
        true
    }

    pub(super) fn apply_toolbar_toggle_board_picker(&mut self) -> bool {
        self.toggle_board_picker();
        true
    }

    pub(super) fn apply_toolbar_board_duplicate(&mut self) -> bool {
        self.duplicate_board();
        true
    }

    pub(super) fn apply_toolbar_board_rename(&mut self) -> bool {
        // Open board picker in rename mode for active board
        self.toggle_board_picker_quick();
        // The board picker handles rename mode internally via its UI
        true
    }
}
