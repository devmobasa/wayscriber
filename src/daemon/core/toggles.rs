use crate::session::catalog;
use crate::{daemon, tray_action};

use super::super::control::{
    DaemonToggleCommand, DaemonToggleCommands, DaemonToggleRequest,
    write_daemon_toggle_command_error, write_daemon_toggle_command_success,
};
use super::super::overlay::overlay_start_backoff_reason;
use super::super::protocol_v2::ReportedSession;
use super::super::types::OverlayState;
use super::{DUPLICATE_SHORTCUT_SUPPRESSION_WINDOW, Daemon};
use crate::tray_action::TrayAction;
use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::mem;
use std::path::Path;
use std::time::Instant;

impl Daemon {
    pub(super) fn ensure_visible_overlay_can_accept_request(
        &self,
        request: Option<&DaemonToggleRequest>,
    ) -> Result<()> {
        let Some(requested) = request.and_then(|request| request.session_file.as_ref()) else {
            return Ok(());
        };
        if self.overlay.state() != OverlayState::Visible {
            return Ok(());
        }
        // The overlay may have switched session since it was launched.
        let active = match self.overlay.reported_session() {
            Some(ReportedSession::Home) => self.initial_named_session_file.clone(),
            Some(ReportedSession::Named(path)) => Some(path),
            None => self
                .overlay
                .active_named_session_file()
                .map(Path::to_path_buf),
        };
        if active.is_some_and(|active| named_session_paths_match(&active, requested)) {
            return Ok(());
        }

        Err(anyhow::anyhow!(
            "cannot switch named session target while overlay is visible; hide the overlay first"
        ))
    }

    pub(super) fn process_single_toggle(
        &mut self,
        request: Option<DaemonToggleRequest>,
        activation_token: Option<String>,
        suppress_overlay_action_signal: bool,
    ) -> Result<bool> {
        let request = request
            .map(|mut request| {
                request.normalize_and_validate_session_file()?;
                Ok::<_, anyhow::Error>(request)
            })
            .transpose()?;
        self.ensure_visible_overlay_can_accept_request(request.as_ref())?;
        let plain_visibility_toggle_requested =
            request.as_ref().is_none_or(DaemonToggleRequest::is_empty);
        if plain_visibility_toggle_requested {
            let now = Instant::now();
            if self
                .last_plain_visibility_toggle_completed_at
                .is_some_and(|previous| {
                    now.saturating_duration_since(previous) < DUPLICATE_SHORTCUT_SUPPRESSION_WINDOW
                })
            {
                info!("Ignoring duplicate plain daemon visibility toggle");
                return Ok(false);
            }
        }
        let action = request.as_ref().and_then(|request| request.overlay_action);
        self.queue_overlay_launch(
            request.filter(|request| !request.is_empty()),
            activation_token,
        );
        if let Some(action) = action {
            if self.overlay.state() == OverlayState::Hidden
                && matches!(action, TrayAction::LightDrawOff)
            {
                self.pending_launch = None;
                return Ok(false);
            }
            let was_hidden = self.overlay.state() == OverlayState::Hidden;
            if was_hidden && let Some(retry_in) = self.overlay_start_backoff() {
                // Queued now, the action would run whenever the overlay next
                // starts, long after the caller was told it had happened.
                self.pending_launch = None;
                return Err(anyhow::anyhow!(overlay_start_backoff_reason(retry_in)));
            }

            self.dispatch_overlay_action(action, !suppress_overlay_action_signal)?;
            if self.overlay.state() == OverlayState::Hidden {
                self.show_overlay()?.require_shown()?;
                return Ok(was_hidden);
            }

            self.pending_launch = None;
            return Ok(false);
        }

        if let Err(err) = self.toggle_overlay() {
            self.pending_launch = None;
            return Err(err);
        }
        if plain_visibility_toggle_requested {
            self.last_plain_visibility_toggle_completed_at = Some(Instant::now());
        }
        Ok(false)
    }

    pub(super) fn process_queued_toggle_command(
        &mut self,
        command: &DaemonToggleCommand,
        suppress_overlay_action_signal: &mut bool,
    ) {
        let result = self.process_single_toggle(
            Some(command.request.clone()),
            None,
            *suppress_overlay_action_signal,
        );
        match result {
            Ok(spawned_overlay) => {
                *suppress_overlay_action_signal |= spawned_overlay;
                if let Err(err) = write_daemon_toggle_command_success(command) {
                    warn!("Failed to write daemon toggle response: {err}");
                }
            }
            Err(err) => {
                let message = format!("{err:#}");
                warn!("Toggle overlay failed: {message}");
                if let Err(response_err) = write_daemon_toggle_command_error(command, &message) {
                    warn!("Failed to write daemon toggle error response: {response_err}");
                }
            }
        }
    }

    fn dispatch_overlay_action(
        &self,
        action: TrayAction,
        signal_visible_overlay: bool,
    ) -> Result<()> {
        let action_path = tray_action::queue_action(action)?;

        if signal_visible_overlay && self.overlay.state() == OverlayState::Visible {
            #[cfg(unix)]
            {
                if let Err(error) = self.overlay.signal(libc::SIGUSR2) {
                    warn!(
                        "Failed to signal overlay process for action {}: {error:#}",
                        action.as_str(),
                    );
                }
            }
            #[cfg(not(unix))]
            {
                warn!("Overlay actions are only supported on Unix platforms");
            }
        }
        log::debug!(
            "Queued overlay action {} at {}",
            action.as_str(),
            action_path.display()
        );

        Ok(())
    }

    /// Asks a light-capable overlay to move its editor instead of hiding.
    ///
    /// The move travels through the action journal and wakes the overlay with
    /// `SIGUSR2`, so the raw `SIGUSR1` a legacy shortcut sends to every
    /// Wayscriber process cannot trigger it a second time.
    pub(in crate::daemon) fn request_light_editor_move(&mut self) -> Result<()> {
        let Some(journal) = self.v2_action_journal.clone() else {
            // Legacy v1 has no ordered action delivery; keep its plain toggle.
            info!("Hiding overlay");
            return self.hide_overlay();
        };

        let action = TrayAction::MoveLightEditor;
        let prepared = journal
            .publish_anonymous(&self.instance_token, action)
            .context("failed to queue light-mode editor move")?;
        if let Err(error) = self.signal_overlay_action_ready(action) {
            let reason = format!("overlay action delivery failed: {error:#}");
            if let Err(abandon_error) = journal.abandon(&prepared, &reason) {
                warn!("Failed to abandon light-mode editor move: {abandon_error:#}");
            }
            return Err(error);
        }

        info!("Moving the light-mode overlay editor");
        self.light_editor_moves.push(prepared);
        Ok(())
    }

    /// A move the previous overlay never claimed must not reach the next one,
    /// which would read it as a toggle and close right after opening.
    pub(in crate::daemon) fn abandon_unclaimed_light_editor_moves(&mut self) {
        let moves = mem::take(&mut self.light_editor_moves);
        let Some(journal) = self.v2_action_journal.as_ref() else {
            return;
        };

        for prepared in moves {
            if let Err(error) = journal.abandon(&prepared, "the overlay exited before claiming it")
            {
                // Claimed, applied, or collected moves need nothing more.
                debug!("Light-mode editor move needs no abandonment: {error:#}");
            }
        }
    }

    pub(super) fn signal_overlay_action_ready(&self, action: TrayAction) -> Result<()> {
        if self.overlay.state() != OverlayState::Visible {
            return Ok(());
        }
        self.overlay.signal(libc::SIGUSR2).with_context(|| {
            format!(
                "failed to notify overlay about committed v2 action {}",
                action.as_str()
            )
        })
    }

    pub(super) fn process_pending_toggles(
        &mut self,
        activation_token: Option<String>,
        signal_toggle_requested: bool,
    ) -> Result<()> {
        let queued_requests = if signal_toggle_requested {
            daemon::take_daemon_toggle_requests(&self.instance_token)?
        } else {
            DaemonToggleCommands {
                commands: Vec::new(),
                saw_command_files: false,
            }
        };

        if !signal_toggle_requested || activation_token.is_some() {
            self.process_single_toggle(None, activation_token, false)?;
        }

        if signal_toggle_requested {
            self.process_signal_toggle_commands(queued_requests)?;
        }

        Ok(())
    }

    pub(super) fn process_signal_toggle_commands(
        &mut self,
        queued_requests: DaemonToggleCommands,
    ) -> Result<()> {
        if queued_requests.commands.is_empty() {
            if queued_requests.saw_command_files {
                return Ok(());
            }
            return self
                .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
                .map(drop);
        }

        let mut suppress_overlay_action_signal = false;
        for command in queued_requests.commands {
            self.process_queued_toggle_command(&command, &mut suppress_overlay_action_signal);
        }
        Ok(())
    }
}

fn named_session_paths_match(left: &Path, right: &Path) -> bool {
    catalog::session_paths_match(left, right)
}
