use std::time::Duration;

use anyhow::{Result, anyhow};
use log::{debug, info};

use super::core::Daemon;
use super::types::{OverlaySpawnCandidate, OverlayState};
use crate::daemon::control::DaemonToggleRequest;
use crate::daemon::protocol_v2::ReportedSession;
use crate::session::catalog::session_paths_match;

pub(super) mod launch;
pub(super) mod lifecycle;
mod process;
mod spawn;
#[cfg(test)]
pub(super) mod tests;

use launch::OverlayLaunchRequest;
use lifecycle::OverlayStartFailure;

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
    /// Turn a deferred start into an error rather than claiming it happened.
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
    /// Toggle overlay visibility.
    pub(super) fn toggle_overlay(&mut self) -> Result<()> {
        match self.overlay.state() {
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

    /// Each hidden start consumes one owned request. Backoff and exhausted
    /// candidates drop it; preparation errors retain only the activation token.
    pub(super) fn show_overlay(&mut self) -> Result<ShowOutcome> {
        if self.overlay.state() == OverlayState::Visible {
            debug!("Overlay already visible");
            return Ok(ShowOutcome::Shown);
        }

        let request = self.take_pending_launch();
        if let Some(runner) = self.backend_runner.clone() {
            self.clear_overlay_spawn_error();
            let resume_default = self.session_resume_override();

            let result = self
                .overlay
                .run_backend(runner.as_ref(), &request, resume_default);

            // Preserve the in-process backend's existing token retention, on
            // both success and failure, without retaining any launch options.
            self.retain_launch_token(request);

            return result.map(|()| ShowOutcome::Shown);
        }

        if let Some(retry_in) = self.overlay.spawn_backoff_remaining() {
            return Ok(ShowOutcome::BackingOff { retry_in });
        }

        let candidates = self.overlay_spawn_candidates();
        self.start_launch(request, &candidates)
    }

    fn start_launch(
        &mut self,
        request: OverlayLaunchRequest,
        candidates: &[OverlaySpawnCandidate],
    ) -> Result<ShowOutcome> {
        match self.spawn_overlay_process(&request, candidates) {
            Ok(()) => {
                self.clear_overlay_spawn_error();
                self.forget_remembered_session_if_started_at_home(&request);
                Ok(ShowOutcome::Shown)
            }
            Err(failure) => {
                let error = match failure {
                    OverlayStartFailure::BeforeAttempt(error) => {
                        self.retain_launch_token(request);
                        error
                    }
                    OverlayStartFailure::Attempt(error) => error,
                };
                self.record_overlay_spawn_failure(error.to_string());
                Err(error)
            }
        }
    }

    fn overlay_launch_request(
        &self,
        request: Option<DaemonToggleRequest>,
        activation_token: Option<String>,
    ) -> OverlayLaunchRequest {
        OverlayLaunchRequest::new(
            request,
            activation_token,
            self.initial_mode.as_deref(),
            self.initial_named_session_file.as_deref(),
            self.freeze_on_show,
        )
    }

    pub(super) fn queue_overlay_launch(
        &mut self,
        request: Option<DaemonToggleRequest>,
        activation_token: Option<String>,
    ) {
        self.pending_launch = Some(self.overlay_launch_request(request, activation_token));
    }

    pub(super) fn replace_pending_launch_request(&mut self, request: DaemonToggleRequest) {
        let token = self
            .pending_launch
            .take()
            .and_then(OverlayLaunchRequest::into_activation_token);
        self.queue_overlay_launch(Some(request), token);
    }

    fn take_pending_launch(&mut self) -> OverlayLaunchRequest {
        self.pending_launch
            .take()
            .unwrap_or_else(|| self.overlay_launch_request(None, None))
    }

    fn retain_launch_token(&mut self, request: OverlayLaunchRequest) {
        self.pending_launch = request
            .into_activation_token()
            .map(|token| self.overlay_launch_request(None, Some(token)));
    }

    pub(super) fn discard_pending_launch_options(&mut self) {
        if let Some(request) = self.pending_launch.take() {
            self.retain_launch_token(request);
        }
    }

    /// Internal runners never back off; child starts use the lifecycle policy.
    pub(super) fn overlay_start_backoff(&mut self) -> Option<Duration> {
        if self.backend_runner.is_some() {
            return None;
        }
        self.overlay.spawn_backoff_remaining()
    }

    /// Hide overlay (destroy layer surface, return to hidden state).
    pub(super) fn hide_overlay(&mut self) -> Result<()> {
        if self.overlay.state() == OverlayState::Hidden {
            debug!("Overlay already hidden");
            return Ok(());
        }

        if self.backend_runner.is_some() {
            debug!("Internal backend runner hidden");
            self.discard_pending_launch_options();
            self.overlay.mark_hidden();
            return Ok(());
        }

        let session = self.overlay.hide()?;
        self.remember_reported_session(session);
        self.discard_pending_launch_options();
        Ok(())
    }

    /// Remembers the session a retired overlay last reported, for the next
    /// show to continue. Without a report the daemon keeps what it knew.
    pub(super) fn remember_reported_session(&mut self, session: Option<ReportedSession>) {
        match session {
            None => {}
            Some(ReportedSession::Home) => self.remembered_session_file = None,
            Some(ReportedSession::Named(path)) => self.remembered_session_file = Some(path),
        }
    }

    /// A request for home started an overlay at home. The overlay has no
    /// session change to report, so the daemon forgets its remembered session
    /// itself rather than return to it on the next show.
    fn forget_remembered_session_if_started_at_home(&mut self, request: &OverlayLaunchRequest) {
        if let (Some(requested), Some(home)) = (
            request.explicit_session_file(),
            self.initial_named_session_file.as_deref(),
        ) && session_paths_match(requested, home)
        {
            self.remembered_session_file = None;
        }
    }
}
