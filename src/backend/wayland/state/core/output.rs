use crate::input::state::{Toast, ToastPriority};
use log::{debug, info, warn};
use smithay_client_toolkit::shell::{WaylandSurface, wlr_layer::Anchor};
use std::time::{Duration, Instant};

use super::super::*;
use crate::{
    backend::wayland::{
        backend::event_loop::session_save,
        session::{
            self as runtime_session, PersistenceOperation, PersistenceOutcome, SaveStrategy,
        },
    },
    input::state::OutputFocusAction,
    notification,
    session::{self, SessionSnapshot},
};

mod focus;
mod identity;
mod session_ops;
mod transition;

const OUTPUT_BADGE_MAX_LEN: usize = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputTransitionStart {
    IgnoreCurrentTarget,
    KeepPending,
    DeferForInteraction,
    LoadInitial,
    ResolveTransition,
}

fn output_transition_start(
    loaded: bool,
    target_changed: bool,
    matching_pending: bool,
    same_epoch_pending: bool,
    live_source_resolution_pending: bool,
    interaction_active: bool,
) -> OutputTransitionStart {
    let superseding_pending_destination = same_epoch_pending && !matching_pending;
    if !target_changed
        && (loaded || superseding_pending_destination || live_source_resolution_pending)
    {
        OutputTransitionStart::IgnoreCurrentTarget
    } else if matching_pending {
        OutputTransitionStart::KeepPending
    } else if interaction_active {
        OutputTransitionStart::DeferForInteraction
    } else if loaded || same_epoch_pending {
        OutputTransitionStart::ResolveTransition
    } else {
        OutputTransitionStart::LoadInitial
    }
}

fn output_transition_retry_at(backoff: Duration) -> Instant {
    Instant::now() + backoff
}

fn live_source_reconciliation_ready(
    live_source_resolution_pending: bool,
    output_transition_pending: bool,
    interaction_active: bool,
    worker_healthy: bool,
) -> bool {
    live_source_resolution_pending
        && !output_transition_pending
        && !interaction_active
        && worker_healthy
}

fn replace_output_session_snapshot(
    input_state: &mut crate::input::InputState,
    measurer: &crate::draw::TextMeasurer,
    snapshot: Option<SessionSnapshot>,
    options: &session::SessionOptions,
) -> anyhow::Result<()> {
    let snapshot = snapshot.unwrap_or_else(|| SessionSnapshot {
        active_board_id: input_state.board_id().to_string(),
        boards: Vec::new(),
        tool_state: None,
    });
    session::apply_snapshot_replacing_boards(input_state, measurer, snapshot, options)
}

#[cfg(test)]
mod tests;
