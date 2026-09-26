//! Launch notice for ink restored onto the transparent overlay board.

use super::super::base::{InputState, Toast, ToastPriority};
use crate::domain::Action;

const RESTORE_NOTICE_KEY: &str = "session.restored";
/// The notice carries a button, so it stays up long enough to reach it.
const RESTORE_NOTICE_DURATION_MS: u64 = 8000;
const CLEAR_LABEL: &str = "Clear";

impl InputState {
    /// Tell the user that earlier ink came back on the transparent overlay
    /// board, where it now sits over whatever is on screen, and offer to clear
    /// it (undoable, like Clear Canvas). Solid boards and empty pages stay
    /// quiet. Returns whether a notice was queued.
    pub(crate) fn announce_restored_annotations(&mut self) -> bool {
        if !self.board_is_transparent() {
            return false;
        }
        let count = self.boards.active_frame().shapes.len();
        if count == 0 {
            return false;
        }

        let toast = Toast::info(restored_annotations_message(count))
            .duration_ms(RESTORE_NOTICE_DURATION_MS)
            .action(CLEAR_LABEL, Action::ClearCanvas);
        self.push_toast(ToastPriority::Action, RESTORE_NOTICE_KEY, toast)
            .accepted()
    }
}

fn restored_annotations_message(count: usize) -> String {
    match count {
        1 => "Restored 1 annotation from last session".to_string(),
        count => format!("Restored {count} annotations from last session"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::Shape;
    use crate::input::BOARD_ID_WHITEBOARD;
    use crate::input::state::ToastCommand;
    use crate::input::state::test_support::make_test_input_state;

    fn add_rect(state: &mut InputState) {
        let color = state.style.current_color;
        state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
            fill: false,
            color,
            thick: 3.0,
        });
    }

    #[test]
    fn restored_overlay_ink_is_announced_with_a_clear_button() {
        let mut state = make_test_input_state();
        assert!(state.board_is_transparent());
        for _ in 0..7 {
            add_rect(&mut state);
        }

        assert!(state.announce_restored_annotations());

        let toast = state.active_toast().expect("restore notice");
        assert_eq!(toast.message, "Restored 7 annotations from last session");
        assert_eq!(toast.duration_ms, RESTORE_NOTICE_DURATION_MS);
        let clear = toast.action.as_ref().expect("clear button");
        assert_eq!(clear.label, "Clear");
        assert_eq!(clear.command, ToastCommand::Dispatch(Action::ClearCanvas));
    }

    #[test]
    fn a_single_restored_annotation_reads_in_the_singular() {
        assert_eq!(
            restored_annotations_message(1),
            "Restored 1 annotation from last session"
        );
    }

    #[test]
    fn empty_pages_and_solid_boards_stay_quiet() {
        let mut state = make_test_input_state();
        assert!(!state.announce_restored_annotations(), "nothing to clear");
        assert!(state.active_toast().is_none());

        state.switch_board_force(BOARD_ID_WHITEBOARD);
        assert!(!state.board_is_transparent());
        add_rect(&mut state);
        assert!(
            !state.announce_restored_annotations(),
            "a solid board hides the desktop, so its ink cannot be mistaken for it"
        );
        assert!(state.active_toast().is_none());
    }
}
