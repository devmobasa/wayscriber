use crate::domain::BoardBackground;
use crate::input::InputState;
use crate::input::boards::{BoardAppearance, BoardPenOrigin};

impl InputState {
    /// Publish one appearance change without touching drawing history.
    pub(in crate::input::state::core) fn apply_board_appearance_value(
        &mut self,
        index: usize,
        appearance: BoardAppearance,
    ) -> bool {
        let active = index == self.boards.active_index();
        let Some(board) = self.boards.board_state_mut(index) else {
            return false;
        };
        let current = BoardAppearance::from_spec(&board.spec);
        if current == appearance
            || current.background.is_transparent()
            || appearance.background.is_transparent()
        {
            return false;
        }
        let recolored = current.background != appearance.background;
        appearance.apply_to(&mut board.spec);
        let pen = if recolored && board.spec.auto_adjust_pen {
            let BoardBackground::Solid(color) = board.spec.background else {
                unreachable!()
            };
            let pen = crate::input::runtime_contrast_pen_color(color);
            board.spec.default_pen_color = Some(pen);
            board.pen_origin = BoardPenOrigin::RuntimeContrast;
            active.then_some(pen)
        } else {
            None
        };
        board.appearance_explicit = true;
        if let Some(pen) = pen {
            self.set_pen_color_from_board(pen);
        }
        self.mark_board_surface_changed();
        true
    }
}
