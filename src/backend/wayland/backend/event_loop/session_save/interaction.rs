use super::*;

pub(in crate::backend::wayland) fn should_defer_for_interaction(state: &WaylandState) -> bool {
    persistence_interaction_active(
        input_persistence_interaction_active(&state.input_state),
        state.toolbar_drag.item_dragging(),
        state.toolbar_drag.is_moving(),
        state.pointer.board_pan_active(),
        state.zoom_panning_active(),
        stylus_tip_down(state),
    )
}

pub(super) fn persistence_interaction_active(
    input: bool,
    toolbar_drag: bool,
    move_drag: bool,
    board_pan: bool,
    zoom_pan: bool,
    stylus_tip: bool,
) -> bool {
    input || toolbar_drag || move_drag || board_pan || zoom_pan || stylus_tip
}

pub(super) fn input_persistence_interaction_active(input_state: &crate::input::InputState) -> bool {
    input_state.has_active_pointer_interaction()
        || matches!(
            input_state.state,
            crate::input::DrawingState::TextInput { .. }
        )
        || input_state.has_pending_spotlight_magnification_gesture()
}

pub(super) fn min_optional_timeout(a: Option<Duration>, b: Option<Duration>) -> Option<Duration> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

pub(super) fn defer_pending_autosave_for_interaction(
    session: &mut SessionState,
    now: Instant,
    options: &session::SessionOptions,
) -> bool {
    if session.autosave_timeout(now, options).is_none() {
        return false;
    }

    let delay = runtime_session::interaction_defer_interval();
    session.defer_autosave(now, delay);
    log::debug!(
        "Deferring autosave for {:?} while an input interaction is active",
        delay
    );
    true
}

pub(super) fn defer_autosave_for_active_interaction(
    session: &mut SessionState,
    now: Instant,
    options: &session::SessionOptions,
    interaction_active: bool,
) -> bool {
    interaction_active && defer_pending_autosave_for_interaction(session, now, options)
}

pub(super) fn finalize_spotlight_wheel_for_shutdown_persistence(
    input_state: &mut crate::input::InputState,
    spotlight_wheel_idle_deadline: &mut Option<Instant>,
) {
    input_state.flush_spotlight_magnification_gesture();
    *spotlight_wheel_idle_deadline = None;
}

#[cfg(feature = "tablet-input")]
fn stylus_tip_down(state: &WaylandState) -> bool {
    state.tablet.is_canvas_gesture()
}

#[cfg(not(feature = "tablet-input"))]
fn stylus_tip_down(_state: &WaylandState) -> bool {
    false
}
