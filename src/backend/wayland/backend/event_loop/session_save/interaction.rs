use crate::input::{DrawingState, InputState};

use super::runtime_session;
use crate::backend::wayland::session::SessionState;
use crate::backend::wayland::state::WaylandState;
use crate::session;
use std::time::{Duration, Instant};

pub(in crate::backend::wayland) fn should_defer_for_interaction(state: &WaylandState) -> bool {
    persistence_interaction_active(PersistenceInteractions {
        input: input_persistence_interaction_active(&state.input_state),
        toolbar_drag: state.toolbar_drag.item_dragging(),
        move_drag: state.toolbar_drag.is_moving(),
        board_pan: state.pointer.board_pan_active(),
        zoom_pan: state.zoom_panning_active(),
        stylus_tip: stylus_tip_down(state),
    })
}

/// The in-progress interactions that hold off session persistence.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct PersistenceInteractions {
    pub(super) input: bool,
    pub(super) toolbar_drag: bool,
    pub(super) move_drag: bool,
    pub(super) board_pan: bool,
    pub(super) zoom_pan: bool,
    pub(super) stylus_tip: bool,
}

pub(super) fn persistence_interaction_active(active: PersistenceInteractions) -> bool {
    active.input
        || active.toolbar_drag
        || active.move_drag
        || active.board_pan
        || active.zoom_pan
        || active.stylus_tip
}

pub(super) fn input_persistence_interaction_active(input_state: &InputState) -> bool {
    input_state.has_active_pointer_interaction()
        || matches!(input_state.state, DrawingState::TextInput { .. })
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
    log::debug!("Deferring autosave for {delay:?} while an input interaction is active");
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
    input_state: &mut InputState,
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
