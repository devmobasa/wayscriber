use crate::input::InputState;

impl InputState {
    pub(super) fn apply_toolbar_toggle_board_picker(&mut self) -> bool {
        self.toggle_board_picker_quick();
        true
    }
}
