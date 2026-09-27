//! Launch notice for ink restored onto the transparent overlay board.

use super::super::base::{InputState, Toast, ToastPriority};
use crate::domain::Action;

const RESTORE_NOTICE_KEY: &str = "session.restored";
/// The notice carries a button, so it stays up long enough to reach it.
const RESTORE_NOTICE_DURATION_MS: u64 = 8000;
const CLEAR_LABEL: &str = "Clear";

/// The board and page a restore notice describes. Its Clear button runs Clear
/// Canvas on whatever page is active, so the notice may only stand while that
/// is still the page it counted. An index alone cannot say so: deleting the
/// restored page slides the next one into its slot. The board's page
/// generation changes on every page insert, delete, and move (not on edits
/// or renames), so together they name that one page within a session. Session
/// replacement retracts the notice explicitly because generations may repeat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::input::state) struct RestoreNoticeContext {
    board_id: String,
    page_index: usize,
    pages_generation: u64,
}

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
        let accepted = self
            .push_toast(ToastPriority::Action, RESTORE_NOTICE_KEY, toast)
            .accepted();
        if accepted {
            self.restore_notice = Some(self.restore_notice_context());
        }
        accepted
    }

    /// Take the notice away once another board or page is active: its Clear
    /// would erase that page, not the restored ink it counted. Called when the
    /// board surface changes; a change that leaves the same page active (a
    /// content edit, a rename) keeps it.
    pub(in crate::input::state) fn retract_restore_notice_if_context_changed(&mut self) {
        let Some(context) = &self.restore_notice else {
            return;
        };
        if *context == self.restore_notice_context() {
            return;
        }

        self.retract_restore_notice();
    }

    /// Remove both the visible and queued notice before its restored ink is
    /// replaced, including session switches that reuse the same page context.
    pub(crate) fn retract_restore_notice(&mut self) {
        self.restore_notice = None;
        self.remove_matching_toasts(|key, _| key == RESTORE_NOTICE_KEY);
    }

    fn restore_notice_context(&self) -> RestoreNoticeContext {
        RestoreNoticeContext {
            board_id: self.board_id().to_string(),
            page_index: self.boards.active_page_index(),
            pages_generation: self.boards.active_page_generation(),
        }
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
            fill_color: None,
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
    fn leaving_the_restored_page_retracts_the_notice_and_its_clear_button() {
        let mut state = make_test_input_state();
        add_rect(&mut state);
        assert!(state.announce_restored_annotations());

        state.switch_board_force(BOARD_ID_WHITEBOARD);

        assert!(
            state.active_toast().is_none(),
            "Clear would now erase the whiteboard, not the restored ink"
        );
        assert_eq!(state.test_pending_toast_count(), 0);
    }

    #[test]
    fn a_new_page_retracts_the_notice() {
        let mut state = make_test_input_state();
        add_rect(&mut state);
        assert!(state.announce_restored_annotations());

        state.page_new();

        assert_eq!(
            state.test_active_toast_key(),
            Some("page.nav"),
            "only the page's own toast remains"
        );
        assert_eq!(state.test_pending_toast_count(), 0);
    }

    /// Deleting the restored page slides the next page into its index; Clear
    /// would then erase that page instead.
    #[test]
    fn deleting_the_restored_page_retracts_the_notice() {
        let measurer = crate::draw::TextMeasurer::default();
        let mut state = make_test_input_state();
        state.page_new();
        add_rect(&mut state);
        state.switch_to_page(0);
        add_rect(&mut state);
        assert!(state.announce_restored_annotations());

        let requested_at = std::time::Instant::now();
        state.delete_active_page_at_with_measurer(&measurer, requested_at);
        state.delete_active_page_at_with_measurer(
            &measurer,
            requested_at + std::time::Duration::from_millis(1),
        );

        assert_eq!(state.boards.page_count(), 1);
        assert_eq!(
            state.boards.active_page_index(),
            0,
            "another page, same index"
        );
        assert_ne!(state.test_active_toast_key(), Some(RESTORE_NOTICE_KEY));
        assert!(
            state.restore_notice.is_none(),
            "nor may it come back from the queue"
        );
    }

    #[test]
    fn opening_another_session_retracts_active_and_queued_restore_notices() {
        use crate::session::{
            SessionOptions, apply_snapshot, apply_snapshot_replacing_boards, snapshot_from_input,
        };

        let measurer = crate::draw::TextMeasurer::default();
        let mut options = SessionOptions::new(std::path::PathBuf::from("/tmp"), "restore-notice");
        options.persist_transparent = true;
        options.restore_tool_state = false;

        for queued in [false, true] {
            let mut source = make_test_input_state();
            add_rect(&mut source);
            let mut state = make_test_input_state();
            apply_snapshot(
                &mut state,
                snapshot_from_input(&source, &options).expect("initial session"),
                &options,
            );
            let restored_generation = state.boards.active_page_generation();
            if queued {
                state.push_toast(
                    ToastPriority::Critical,
                    "test.blocker",
                    Toast::warning("Blocking"),
                );
            }
            assert!(state.announce_restored_annotations());
            assert_eq!(state.test_pending_toast_count(), usize::from(queued));

            add_rect(&mut source);
            apply_snapshot_replacing_boards(
                &mut state,
                &measurer,
                snapshot_from_input(&source, &options).expect("replacement session"),
                &options,
            )
            .expect("open replacement session");

            assert_eq!(state.boards.active_frame().shapes.len(), 2);
            assert_eq!(
                state.boards.active_page_generation(),
                restored_generation,
                "session replacement may reuse the old page generation"
            );
            assert_ne!(
                state.test_active_toast_key(),
                Some(RESTORE_NOTICE_KEY),
                "Clear must not target ink from the replacement session"
            );
            assert_eq!(
                state.test_pending_toast_count(),
                0,
                "nor may it reappear later"
            );
        }
    }

    #[test]
    fn edits_on_the_restored_page_keep_the_notice() {
        let mut state = make_test_input_state();
        add_rect(&mut state);
        assert!(state.announce_restored_annotations());

        state.retract_restore_notice_if_context_changed();

        assert_eq!(
            state.test_active_toast_key(),
            Some(RESTORE_NOTICE_KEY),
            "the same page is still active"
        );
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
