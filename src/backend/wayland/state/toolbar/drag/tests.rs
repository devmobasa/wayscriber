use super::*;
use crate::toolbar_gtk::{GtkToolbarDragPhase, GtkToolbarSurfaceSize};

const TEST_SURFACE_SIZE: GtkToolbarSurfaceSize = GtkToolbarSurfaceSize {
    width: 260,
    height: 789,
};

fn gtk_offset(phase: GtkToolbarDragPhase, seq: u64) -> GtkToolbarFeedback {
    GtkToolbarFeedback::SetTopOffset {
        x: 10.0,
        y: 20.0,
        surface_size: TEST_SURFACE_SIZE,
        seq,
        phase,
    }
}

fn moving(preview: bool) -> ToolbarDrag {
    let mut drag = ToolbarDrag::new();
    drag.set_preview_active(preview);
    drag.begin_move(MoveDragKind::Top, (1.0, 2.0), false, (24.0, 12.0));
    drag
}

#[test]
fn move_and_handoff_transition_table_is_explicit() {
    let now = Instant::now();
    let mut drag = moving(true);
    let ended = drag.end_move().unwrap();
    assert_eq!(ended.commit_base, Some(24.0));
    assert!(ended.had_preview);

    drag.begin_handoff(now + Duration::from_millis(10));
    assert_eq!(drag.finish_handoff_if_due(now), None);
    assert_eq!(
        drag.finish_handoff_if_due(now + Duration::from_millis(10)),
        Some(HandoffEnd::BuiltIn)
    );
    assert!(!drag.preview_active());
}

#[test]
fn move_cancel_can_return_directly_to_idle() {
    let mut drag = moving(false);
    assert!(drag.end_move().is_some());
    assert!(!drag.is_moving());
    assert_eq!(drag.finish_handoff(), None);
}

#[test]
fn gtk_preview_handoff_and_cancel_follow_their_own_phase() {
    let now = Instant::now();
    let mut drag = ToolbarDrag::new();
    drag.begin_handoff(now);
    drag.begin_gtk_preview(GtkToolbarKind::Top, 24.0);
    assert_eq!(drag.handoff_timeout(now), None);
    drag.begin_handoff(now + Duration::from_millis(10));
    assert_eq!(
        drag.finish_handoff_if_due(now + Duration::from_millis(10)),
        Some(HandoffEnd::Gtk)
    );

    drag.begin_gtk_preview(GtkToolbarKind::Top, 24.0);
    assert!(drag.cancel_gtk());
    assert!(!drag.cancel_gtk());
}

#[test]
fn throttle_reports_a_pending_terminal_apply() {
    let start = Instant::now();
    let mut drag = moving(false);
    let interval = Duration::from_millis(20);
    assert!(drag.should_apply(start, interval));
    assert!(!drag.should_apply(start + Duration::from_millis(5), interval));
    assert!(drag.end_move().unwrap().pending_apply);
}

#[test]
fn blocked_gtk_drag_advances_sequence_and_stays_blocked_until_end() {
    let mut drag = ToolbarDrag::new();
    drag.note_gtk_offset_seq(4);

    assert!(drag.gtk_note_feedback(true, &gtk_offset(GtkToolbarDragPhase::Start, 9)));
    assert!(drag.gtk_note_feedback(false, &gtk_offset(GtkToolbarDragPhase::Move, 8)));
    assert_eq!(drag.gtk_offset_seq(), 9);
    assert!(drag.gtk_note_feedback(false, &gtk_offset(GtkToolbarDragPhase::End, 10)));
    assert_eq!(drag.gtk_offset_seq(), 10);
    assert!(!drag.gtk_note_feedback(false, &gtk_offset(GtkToolbarDragPhase::Start, 11)));
}

#[test]
fn passive_and_capture_feedback_follow_modal_policy() {
    let mut drag = ToolbarDrag::new();
    drag.block_gtk_drag();
    assert!(!drag.gtk_note_feedback(
        true,
        &GtkToolbarFeedback::CaptureSuppressionReady { generation: 7 }
    ));
    let shortcut = GtkToolbarFeedback::PointerShortcut {
        button: 8,
        ctrl: false,
        shift: false,
        alt: false,
        logo: false,
    };
    assert!(drag.gtk_note_feedback(true, &shortcut));
    assert!(!drag.gtk_note_feedback(false, &shortcut));
    assert!(drag.gtk_note_feedback(false, &gtk_offset(GtkToolbarDragPhase::Move, 1)));
}

#[test]
fn local_and_screen_events_remain_continuous_as_the_toolbar_moves() {
    let mut drag = moving(false);
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Screen((104.0, 206.0)),
            (100.0, 200.0)
        ),
        Some((3.0, 4.0))
    );
    // Apply the delta to the toolbar origin, then return to the same local spot.
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Local((1.0, 2.0)),
            (103.0, 204.0)
        ),
        Some((0.0, 0.0))
    );
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Local((3.0, 1.0)),
            (103.0, 204.0)
        ),
        Some((2.0, -1.0))
    );
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Screen((108.0, 204.0)),
            (105.0, 203.0)
        ),
        Some((2.0, -1.0))
    );
}

#[test]
fn local_motion_normalizes_the_initial_sample_before_applying_offsets() {
    let mut drag = moving(false);
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Local((3.0, 4.0)),
            (100.0, 200.0)
        ),
        Some((2.0, 2.0))
    );
    // Subsequent layer-surface motion compares against the saved screen sample.
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Local((3.0, 4.0)),
            (102.0, 202.0)
        ),
        Some((2.0, 2.0))
    );
}

#[test]
fn local_preview_deltas_ignore_changes_to_the_suppressed_surface_origin() {
    let mut drag = moving(true);
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Local((3.0, 4.0)),
            (100.0, 200.0)
        ),
        Some((2.0, 2.0))
    );
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Local((4.0, 6.0)),
            (102.0, 202.0)
        ),
        Some((1.0, 2.0))
    );
}

#[test]
fn preview_converts_to_screen_but_rebases_on_return_to_local_motion() {
    let mut drag = moving(true);
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Screen((104.0, 206.0)),
            (100.0, 200.0)
        ),
        Some((3.0, 4.0))
    );
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Local((500.0, 600.0)),
            (103.0, 204.0)
        ),
        None
    );
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Local((501.0, 602.0)),
            (103.0, 204.0)
        ),
        Some((1.0, 2.0))
    );
}

#[test]
fn rejected_samples_consume_the_baseline_across_coordinate_routes() {
    let mut drag = moving(false);
    drag.note_move(MoveDragKind::Top, MoveSample::Local((20.0, 30.0)));
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Screen((126.0, 239.0)),
            (103.0, 204.0)
        ),
        Some((3.0, 5.0))
    );
    drag.note_move(MoveDragKind::Top, MoveSample::Screen((300.0, 400.0)));
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Local((197.0, 193.0)),
            (106.0, 209.0)
        ),
        Some((3.0, 2.0))
    );
}

#[test]
fn rejected_preview_samples_keep_their_local_baseline() {
    let mut drag = moving(true);
    drag.note_move(MoveDragKind::Top, MoveSample::Local((20.0, 30.0)));
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Local((23.0, 35.0)),
            (100.0, 200.0)
        ),
        Some((3.0, 5.0))
    );
    drag.note_move(MoveDragKind::Top, MoveSample::Screen((300.0, 400.0)));
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Local((20.0, 30.0)),
            (100.0, 200.0)
        ),
        None
    );
    assert_eq!(
        drag.move_to(
            MoveDragKind::Top,
            MoveSample::Local((21.0, 32.0)),
            (100.0, 200.0)
        ),
        Some((1.0, 2.0))
    );
}

#[test]
fn motion_outside_a_builtin_drag_cannot_create_a_baseline() {
    let mut drag = moving(false);
    drag.end_move().unwrap();
    let sample = MoveSample::Screen((300.0, 400.0));
    assert_eq!(drag.note_move(MoveDragKind::Top, sample), None);
    assert_eq!(drag.move_to(MoveDragKind::Top, sample, (0.0, 0.0)), None);
    drag.begin_gtk_preview(GtkToolbarKind::Top, 24.0);
    assert_eq!(drag.move_to(MoveDragKind::Top, sample, (0.0, 0.0)), None);
    assert_eq!(drag.frozen_base_x(), Some(24.0));
}

#[test]
fn gtk_preview_uses_the_base_frozen_at_drag_start_until_released() {
    let mut drag = ToolbarDrag::new();
    drag.begin_gtk_preview(GtkToolbarKind::Top, 24.0);
    drag.set_gtk_rebase(Some((300.0, 400.0)));
    assert_eq!(drag.frozen_base_x(), Some(24.0));
    assert_eq!(drag.frozen_base_y(), None);

    let deadline = Instant::now() + Duration::from_millis(10);
    drag.begin_handoff(deadline);
    assert_eq!(drag.frozen_base_x(), Some(24.0));
    drag.release_gtk_frozen_base(10.0);
    assert_eq!(drag.frozen_base_x(), Some(10.0));
    assert_eq!(drag.finish_handoff_if_due(deadline), Some(HandoffEnd::Gtk));
    assert_eq!(drag.frozen_base_x(), None);
}

#[test]
fn idle_and_handoff_layouts_do_not_reuse_a_stale_builtin_frozen_base() {
    let mut drag = moving(true);
    assert_eq!(drag.frozen_base_x(), Some(24.0));
    assert_eq!(drag.frozen_base_y(), Some(12.0));
    assert_eq!(drag.end_move().unwrap().commit_base, Some(24.0));
    assert_eq!(drag.frozen_base_x(), None);
    assert_eq!(drag.frozen_base_y(), None);

    let now = Instant::now();
    drag.begin_handoff(now);
    assert_eq!(drag.frozen_base_x(), None);
    assert_eq!(drag.finish_handoff_if_due(now), Some(HandoffEnd::BuiltIn));
    assert_eq!(drag.frozen_base_x(), None);
}

#[test]
fn cancelling_or_blocking_a_gtk_drag_releases_its_frozen_base() {
    let mut drag = ToolbarDrag::new();
    drag.begin_gtk_preview(GtkToolbarKind::Top, 24.0);
    assert!(drag.cancel_gtk());
    assert_eq!(drag.frozen_base_x(), None);

    drag.begin_gtk_preview(GtkToolbarKind::Top, 48.0);
    assert_eq!(drag.frozen_base_x(), Some(48.0));
    drag.block_gtk_drag();
    assert_eq!(drag.frozen_base_x(), None);
    assert!(drag.gtk_note_feedback(false, &gtk_offset(GtkToolbarDragPhase::End, 1)));
    assert_eq!(drag.frozen_base_x(), None);
}
