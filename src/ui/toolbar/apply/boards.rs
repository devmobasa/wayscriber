use crate::draw::Color;
use crate::input::InputState;

impl InputState {
    pub(super) fn apply_toolbar_set_board_color(&mut self, color: Color) -> bool {
        self.set_active_board_background_color(color)
    }

    pub(super) fn apply_toolbar_rename_board(&mut self) -> bool {
        if !self.is_board_picker_open() {
            self.open_board_picker();
        }
        let active = self.boards.active_index();
        self.board_picker_set_selected(active);
        self.board_picker_rename_selected();
        self.needs_redraw = true;
        true
    }
}
