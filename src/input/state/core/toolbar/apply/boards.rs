use crate::draw::TextMeasurer;
use crate::input::InputState;
use crate::input::state::{Toast, ToastPriority};

impl InputState {
    pub(super) fn apply_toolbar_board_prev_with(&mut self, measurer: &TextMeasurer) -> bool {
        self.switch_board_prev_with_measurer(measurer);
        true
    }

    pub(super) fn apply_toolbar_board_next_with(&mut self, measurer: &TextMeasurer) -> bool {
        self.switch_board_next_with_measurer(measurer);
        true
    }

    pub(super) fn apply_toolbar_board_new_with(&mut self, measurer: &TextMeasurer) -> bool {
        if self.create_board_with_measurer(measurer) {
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

    pub(super) fn apply_toolbar_board_delete_with(&mut self, measurer: &TextMeasurer) -> bool {
        self.delete_active_board_with_measurer(measurer);
        true
    }

    pub(super) fn apply_toolbar_toggle_board_picker_with(
        &mut self,
        measurer: &TextMeasurer,
    ) -> bool {
        self.toggle_board_picker_with_measurer(measurer);
        true
    }

    pub(super) fn apply_toolbar_board_duplicate_with(&mut self, measurer: &TextMeasurer) -> bool {
        self.duplicate_board_with_measurer(measurer);
        true
    }

    pub(super) fn apply_toolbar_board_rename_with(&mut self, measurer: &TextMeasurer) -> bool {
        // Open board picker in rename mode for active board
        self.toggle_board_picker_quick_with(measurer);
        // The board picker handles rename mode internally via its UI
        true
    }
}
