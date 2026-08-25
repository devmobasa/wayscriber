//! Deadlines that finish a pointer or keyboard gesture the user has stopped
//! driving.
//!
//! Separate from capture and render deadlines because the thing being closed is
//! input history, not a compositor request: a gesture that ends because nothing
//! more arrived still owes its undo entry, and burying that in a capture poll
//! hides when it happens.

use std::time::{Duration, Instant};

use crate::input::InputState;

/// Fires any interaction deadline that has come due.
pub(super) fn poll_interaction_deadlines(
    input_state: &mut InputState,
    spotlight_wheel_idle_deadline: &mut Option<Instant>,
    now: Instant,
) {
    // A wheel burst over a loupe is one undo entry. Discrete wheels send no
    // end-of-gesture signal, so a quiet period is what ends it; without this
    // the entry would wait for some unrelated interaction to close it.
    if spotlight_wheel_idle_deadline.is_some_and(|deadline| now >= deadline) {
        input_state.flush_spotlight_magnification_gesture();
        *spotlight_wheel_idle_deadline = None;
    }
}

/// How long the loop may sleep before an interaction deadline needs it awake.
pub(super) fn interaction_timeout(
    spotlight_wheel_idle_deadline: Option<Instant>,
    now: Instant,
) -> Option<Duration> {
    spotlight_wheel_idle_deadline.map(|deadline| deadline.saturating_duration_since(now))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Action;
    use crate::draw::Shape;
    use crate::input::state::{SpotlightWheelOutcome, test_support::make_test_input_state};

    #[test]
    fn polling_the_owning_path_finishes_one_idle_wheel_burst() {
        let mut input_state = make_test_input_state();
        let shape_id = input_state
            .boards
            .active_frame_mut()
            .add_shape(Shape::Spotlight {
                cx: 200,
                cy: 200,
                rx: 60,
                ry: 40,
                magnification: 2.0,
            });
        let now = Instant::now();
        let mut deadline = Some(now + Duration::from_millis(600));

        assert_eq!(
            input_state.nudge_spotlight_magnification_at(200, 200, 1),
            SpotlightWheelOutcome::Adjusted
        );
        poll_interaction_deadlines(
            &mut input_state,
            &mut deadline,
            now + Duration::from_millis(599),
        );
        assert!(
            deadline.is_some(),
            "the gesture is still inside its quiet period"
        );

        poll_interaction_deadlines(
            &mut input_state,
            &mut deadline,
            now + Duration::from_millis(600),
        );
        assert!(
            deadline.is_none(),
            "the owning poll clears a fired deadline"
        );

        assert_eq!(
            input_state.nudge_spotlight_magnification_at(200, 200, 1),
            SpotlightWheelOutcome::Adjusted
        );
        input_state.handle_action(Action::Undo);
        let magnification = match input_state
            .boards
            .active_frame()
            .shape(shape_id)
            .expect("spotlight")
            .shape
        {
            Shape::Spotlight { magnification, .. } => magnification,
            ref other => panic!("expected a spotlight, got {other:?}"),
        };
        assert_eq!(
            magnification, 2.25,
            "the post-idle tick must be a separately undoable gesture"
        );

        input_state.handle_action(Action::Undo);
        let magnification = match input_state
            .boards
            .active_frame()
            .shape(shape_id)
            .expect("spotlight")
            .shape
        {
            Shape::Spotlight { magnification, .. } => magnification,
            ref other => panic!("expected a spotlight, got {other:?}"),
        };
        assert_eq!(magnification, 2.0);
    }

    #[test]
    fn the_timeout_shrinks_as_the_deadline_approaches_and_never_goes_negative() {
        let now = Instant::now();
        let deadline = now + Duration::from_millis(600);

        assert_eq!(
            interaction_timeout(Some(deadline), now),
            Some(Duration::from_millis(600))
        );
        // A deadline already passed asks for an immediate wake, not a wrap.
        assert_eq!(
            interaction_timeout(Some(deadline), now + Duration::from_secs(5)),
            Some(Duration::ZERO)
        );
        assert_eq!(interaction_timeout(None, now), None);
    }
}
