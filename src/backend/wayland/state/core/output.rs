use crate::input::state::{Toast, ToastPriority};
use log::{info, warn};
use smithay_client_toolkit::shell::{WaylandSurface, wlr_layer::Anchor};
use std::time::{Duration, Instant};

use super::super::*;
use crate::{
    backend::wayland::backend::event_loop::session_save, input::state::OutputFocusAction,
    notification, session,
};

mod focus;
mod identity;
mod session_ops;
mod transition;

const OUTPUT_BADGE_MAX_LEN: usize = 28;

impl WaylandState {
    /// Publish compositor membership changes once, using the selected output's scale.
    pub(in crate::backend::wayland) fn refresh_surface_output(
        &mut self,
        previous: Option<&wl_output::WlOutput>,
        exclude: Option<&wl_output::WlOutput>,
    ) {
        let active = self.surface.current_output();
        let changed = previous != active.as_ref();
        if changed {
            if active.is_some() {
                self.toolbar_chrome.set_needs_recreate(true);
            }
            if let Some(info) = active
                .as_ref()
                .and_then(|output| self.protocol.output().info(output))
            {
                let scale = info.scale_factor.max(1);
                self.surface.set_scale(scale);
                self.toolbar.maybe_update_scale(active.as_ref(), scale);
            }
            self.buffer_damage
                .mark_all_full(FullDamageReason::OutputChanged);
            self.toolbar.mark_dirty();
        }

        self.refresh_active_output_label_excluding(exclude);
        self.refresh_freeze_zoom_geometry_excluding(exclude);

        if changed {
            self.frozen.unfreeze(&mut self.input_state);
            self.zoom.handle_output_change(&mut self.input_state);
            let (width, height) = self.surface.physical_dimensions();
            self.frozen
                .handle_resize(width, height, &mut self.input_state);
            self.zoom
                .handle_resize(width, height, &mut self.input_state);

            if let Some(output) = active.as_ref() {
                let identity = self.output_identity_for(output);
                self.begin_session_output_transition(identity, "surface output change");
            }
            self.input_state.needs_redraw = true;
        }

        self.cancel_screen_modals_if_source_changed();
    }
}

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

#[cfg(test)]
mod tests;
