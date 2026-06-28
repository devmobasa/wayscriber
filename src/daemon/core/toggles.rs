use super::super::control::{
    DaemonToggleCommand, DaemonToggleCommands, DaemonToggleRequest,
    write_daemon_toggle_command_error, write_daemon_toggle_command_success,
};
use super::super::types::OverlayState;
use super::{DUPLICATE_SHORTCUT_SUPPRESSION_WINDOW, Daemon};
use crate::tray_action::TrayAction;
use anyhow::{Context, Result};
use log::{info, warn};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::Instant;

impl Daemon {
    pub(super) fn ensure_visible_overlay_can_accept_request(
        &self,
        request: Option<&DaemonToggleRequest>,
    ) -> Result<()> {
        let Some(requested) = request.and_then(|request| request.session_file.as_ref()) else {
            return Ok(());
        };
        if self.overlay_state != OverlayState::Visible {
            return Ok(());
        }
        if self
            .active_named_session_file
            .as_ref()
            .is_some_and(|active| named_session_paths_match(active, requested))
        {
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
        if let Some(action) = request.as_ref().and_then(|request| request.overlay_action) {
            self.pending_activation_token = activation_token;
            self.pending_toggle_request = request.filter(|request| !request.is_empty());
            if self.overlay_state == OverlayState::Hidden
                && matches!(action, TrayAction::LightDrawOff)
            {
                self.pending_activation_token = None;
                self.pending_toggle_request = None;
                return Ok(false);
            }
            let was_hidden = self.overlay_state == OverlayState::Hidden;
            self.dispatch_overlay_action(action, !suppress_overlay_action_signal)?;
            if self.overlay_state == OverlayState::Hidden {
                self.show_overlay()?;
                return Ok(was_hidden);
            } else {
                self.pending_activation_token = None;
                self.pending_toggle_request = None;
            }
            return Ok(false);
        }

        self.pending_activation_token = activation_token;
        self.pending_toggle_request = request.filter(|request| !request.is_empty());
        if let Err(err) = self.toggle_overlay() {
            self.pending_activation_token = None;
            self.pending_toggle_request = None;
            return Err(err);
        }
        if plain_visibility_toggle_requested {
            self.last_plain_visibility_toggle_completed_at = Some(Instant::now());
        }
        Ok(false)
    }

    pub(super) fn process_queued_toggle_command(
        &mut self,
        command: DaemonToggleCommand,
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
                if let Err(err) = write_daemon_toggle_command_success(&command) {
                    warn!("Failed to write daemon toggle response: {}", err);
                }
            }
            Err(err) => {
                let message = format!("{err:#}");
                warn!("Toggle overlay failed: {}", message);
                if let Err(response_err) = write_daemon_toggle_command_error(&command, &message) {
                    warn!(
                        "Failed to write daemon toggle error response: {}",
                        response_err
                    );
                }
            }
        }
    }

    fn dispatch_overlay_action(
        &self,
        action: TrayAction,
        signal_visible_overlay: bool,
    ) -> Result<()> {
        let action_path = crate::tray_action::queue_action(action)?;

        let pid = self.overlay_pid.load(Ordering::Acquire);
        if signal_visible_overlay && self.overlay_state == OverlayState::Visible && pid != 0 {
            #[cfg(unix)]
            {
                let pid = i32::try_from(pid).context("overlay pid does not fit into i32")?;
                // SAFETY: `pid` has been checked to fit the Unix pid range and
                // `SIGUSR2` is a valid signal constant.
                if unsafe { libc::kill(pid, libc::SIGUSR2) } != 0 {
                    warn!(
                        "Failed to signal overlay process {} for action {}: {}",
                        pid,
                        action.as_str(),
                        std::io::Error::last_os_error()
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

    pub(super) fn process_pending_toggles(
        &mut self,
        activation_token: Option<String>,
        signal_toggle_requested: bool,
    ) -> Result<()> {
        let queued_requests = if signal_toggle_requested {
            crate::daemon::take_daemon_toggle_requests(&self.instance_token)?
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
            self.process_queued_toggle_command(command, &mut suppress_overlay_action_signal);
        }
        Ok(())
    }
}

fn named_session_paths_match(left: &Path, right: &Path) -> bool {
    crate::session::catalog::session_paths_match(left, right)
}
