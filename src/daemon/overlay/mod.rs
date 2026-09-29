use std::time::Duration;

use anyhow::{Result, anyhow};
use log::{debug, info};

use crate::{runtime_session_override, set_runtime_session_override};

use super::core::Daemon;
use super::types::OverlayState;

mod process;
mod spawn;

/// What [`Daemon::show_overlay`] did with a request to show the overlay.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ShowOutcome {
    /// The overlay is visible, or the internal runner ran it to completion.
    Shown,
    /// A recent spawn failure holds starts back, so nothing was started.
    BackingOff { retry_in: Duration },
}

impl ShowOutcome {
    /// The outcome as a result, for a caller that must report a deferred
    /// start to whoever asked for the overlay rather than claim it happened.
    pub(super) fn require_shown(self) -> Result<()> {
        match self {
            Self::Shown => Ok(()),
            Self::BackingOff { retry_in } => Err(anyhow!(overlay_start_backoff_reason(retry_in))),
        }
    }
}

/// Why a request that needs the overlay started is refused during backoff.
pub(super) fn overlay_start_backoff_reason(retry_in: Duration) -> String {
    format!(
        "overlay start is backing off after a spawn failure (retry in {}s)",
        retry_in.as_secs().max(1)
    )
}

impl Daemon {
    /// Toggle overlay visibility
    pub(super) fn toggle_overlay(&mut self) -> Result<()> {
        match self.overlay_state {
            OverlayState::Hidden => {
                info!("Showing overlay");
                self.show_overlay()?.require_shown()?;
            }
            OverlayState::Visible => {
                info!("Hiding overlay");
                self.hide_overlay()?;
            }
        }
        Ok(())
    }

    /// Show overlay (create layer surface and enter drawing mode)
    ///
    /// A start attempt consumes the pending request: once the attempt has
    /// started the overlay, been held back by the spawn backoff, or failed,
    /// the request is gone, so a later start that nobody asked to shape
    /// cannot inherit its mode or session file.
    pub(super) fn show_overlay(&mut self) -> Result<ShowOutcome> {
        if self.overlay_state == OverlayState::Visible {
            debug!("Overlay already visible");
            return Ok(ShowOutcome::Shown);
        }

        if let Some(runner) = self.backend_runner.clone() {
            self.overlay_state = OverlayState::Visible;
            self.active_named_session_file = self.effective_named_session_file();
            info!("Overlay state set to Visible");
            self.clear_overlay_spawn_error();
            let previous_override = runtime_session_override();
            let request_override = self
                .pending_toggle_request
                .as_ref()
                .and_then(|request| request.session_resume_override());
            set_runtime_session_override(
                request_override.or_else(|| self.session_resume_override()),
            );
            let requested_mode = self
                .pending_toggle_request
                .as_ref()
                .and_then(|request| request.mode.clone())
                .or_else(|| self.initial_mode.clone());
            let result = runner(requested_mode);
            set_runtime_session_override(previous_override);
            self.pending_toggle_request = None;
            self.active_named_session_file = None;
            self.overlay_state = OverlayState::Hidden;
            info!("Overlay closed, back to daemon mode");
            return result.map(|()| ShowOutcome::Shown);
        }

        if let Some(retry_in) = self.overlay_spawn_backoff_remaining() {
            self.pending_toggle_request = None;
            self.pending_activation_token = None;
            return Ok(ShowOutcome::BackingOff { retry_in });
        }

        let spawned = self.spawn_overlay_process();
        self.pending_toggle_request = None;
        if let Err(err) = spawned {
            self.record_overlay_spawn_failure(err.to_string());
            return Err(err);
        }

        self.clear_overlay_spawn_error();
        Ok(ShowOutcome::Shown)
    }

    /// How long the spawn backoff still holds back a start of the hidden
    /// overlay, checked before a request commits to one. An internal runner
    /// never backs off.
    pub(super) fn overlay_start_backoff(&mut self) -> Option<Duration> {
        if self.backend_runner.is_some() {
            return None;
        }
        self.overlay_spawn_backoff_remaining()
    }

    /// Hide overlay (destroy layer surface, return to hidden state)
    pub(super) fn hide_overlay(&mut self) -> Result<()> {
        if self.overlay_state == OverlayState::Hidden {
            debug!("Overlay already hidden");
            return Ok(());
        }

        if self.backend_runner.is_some() {
            // Internal runner does not keep additional state to tear down
            debug!("Internal backend runner hidden");
            self.pending_toggle_request = None;
            self.active_named_session_file = None;
            self.overlay_state = OverlayState::Hidden;
            return Ok(());
        }

        self.terminate_overlay_process()?;
        self.pending_toggle_request = None;
        self.active_named_session_file = None;
        self.overlay_state = OverlayState::Hidden;
        Ok(())
    }
}
