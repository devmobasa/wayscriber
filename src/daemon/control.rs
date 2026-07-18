use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(test)]
use crate::env_vars::XDG_RUNTIME_DIR_ENV;
#[cfg(test)]
use crate::paths::daemon_command_dir;
use crate::tray_action::TrayAction;

mod queue;
mod response;
mod runtime;

use queue::write_daemon_toggle_request;
pub(crate) use queue::{clear_daemon_toggle_request_file, take_daemon_toggle_requests};
#[cfg(test)]
pub(crate) use response::read_daemon_toggle_response;
use response::{clear_daemon_toggle_command_files, wait_daemon_toggle_command_response};
#[cfg(test)]
use response::{wait_daemon_toggle_command_response_for, wait_daemon_toggle_response_for};
pub(crate) use response::{write_daemon_toggle_command_error, write_daemon_toggle_command_success};
#[cfg(test)]
use runtime::read_daemon_runtime_file;
pub(crate) use runtime::{clear_daemon_pid_file, write_daemon_pid_file};
use runtime::{clear_stale_daemon_state_if_matches, read_daemon_runtime_info, signal_daemon_pid};

const MAX_DAEMON_TOGGLE_REQUEST_AGE: Duration = Duration::from_secs(5);
// Must exceed the request freshness window plus the overlay graceful stop path
// so a valid hide request does not time out while the daemon is still handling it.
const MAX_DAEMON_TOGGLE_RESPONSE_WAIT: Duration = Duration::from_secs(8);
const DAEMON_TOGGLE_RESPONSE_POLL: Duration = Duration::from_millis(20);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DaemonRuntimeInfo {
    pid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DaemonToggleEnvelope {
    daemon_token: String,
    requested_at_unix_ms: u64,
    #[serde(default)]
    canceled: bool,
    request: DaemonToggleRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DaemonToggleCommand {
    pub(crate) daemon_token: String,
    pub(crate) request: DaemonToggleRequest,
    pub(crate) request_path: PathBuf,
    pub(crate) response_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DaemonToggleCommands {
    pub(crate) commands: Vec<DaemonToggleCommand>,
    pub(crate) saw_command_files: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DaemonToggleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl DaemonToggleResponse {
    fn ok() -> Self {
        Self { error: None }
    }

    fn error(message: String) -> Self {
        Self {
            error: Some(message),
        }
    }

    fn into_result(self) -> Result<()> {
        match self.error {
            Some(message) => Err(anyhow!(message)),
            None => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(crate) struct DaemonToggleRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) mode: Option<String>,
    #[serde(default)]
    pub(crate) freeze: bool,
    #[serde(default)]
    pub(crate) exit_after_capture: bool,
    #[serde(default)]
    pub(crate) no_exit_after_capture: bool,
    #[serde(default)]
    pub(crate) resume_session: bool,
    #[serde(default)]
    pub(crate) no_resume_session: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) session_file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) overlay_action: Option<TrayAction>,
}

impl DaemonToggleRequest {
    pub(crate) fn is_empty(&self) -> bool {
        self.mode.is_none()
            && !self.freeze
            && !self.exit_after_capture
            && !self.no_exit_after_capture
            && !self.resume_session
            && !self.no_resume_session
            && self.session_file.is_none()
            && self.overlay_action.is_none()
    }

    pub(crate) fn session_resume_override(&self) -> Option<bool> {
        if self.resume_session {
            Some(true)
        } else if self.no_resume_session {
            Some(false)
        } else {
            None
        }
    }

    pub(crate) fn normalize_and_validate_session_file(&mut self) -> Result<()> {
        let Some(path) = self.session_file.as_ref() else {
            return Ok(());
        };
        if self.no_resume_session {
            return Err(anyhow!(
                "--session-file conflicts with --no-resume-session because --session-file requires session persistence for this run"
            ));
        }
        if !path.is_absolute() {
            return Err(anyhow!(
                "daemon --session-file request must use an absolute path"
            ));
        }
        let normalized = normalize_daemon_session_file(path)?;
        crate::session::validate_named_session_file_for_foreground(&normalized)?;
        self.session_file = Some(normalized);
        Ok(())
    }
}

fn normalize_daemon_session_file(path: &Path) -> Result<PathBuf> {
    let raw = path
        .to_str()
        .ok_or_else(|| anyhow!("--session-file path must be valid UTF-8"))?;
    Ok(crate::session::normalize_named_session_file_arg(raw))
}

fn current_unix_millis() -> Result<u64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before UNIX_EPOCH")?;
    Ok(duration
        .as_secs()
        .saturating_mul(1000)
        .saturating_add(duration.subsec_millis() as u64))
}

pub(crate) fn generate_daemon_instance_token() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}-{:x}", std::process::id(), now)
}

pub(crate) fn send_daemon_toggle_request(request: &DaemonToggleRequest) -> Result<()> {
    match crate::daemon::protocol_v2::read_runtime_record(&crate::paths::daemon_pid_file())? {
        crate::daemon::protocol_v2::ClassifiedRuntimeRecord::V2(runtime) => {
            let command = crate::daemon::protocol_v2::ClientCommand::publish(
                &crate::daemon::protocol_v2::DaemonRequestV2::from(request),
                &runtime.v2_instance_token,
            )?;
            return finish_v2_command(command.wait()?);
        }
        crate::daemon::protocol_v2::ClassifiedRuntimeRecord::LegacyV1 { .. } => {}
    }

    let runtime = read_daemon_runtime_info()?;
    let mut command = None;

    if let Some(token) = runtime.token.as_deref() {
        command = Some(write_daemon_toggle_request(request, token)?);
    } else if !request.is_empty() {
        return Err(anyhow!(
            "running daemon does not support typed control; restart wayscriber daemon"
        ));
    }

    if let Err(err) = signal_daemon_pid(runtime.pid) {
        clear_stale_daemon_state_if_matches(&runtime);
        if let Some(command) = command.as_ref() {
            clear_daemon_toggle_command_files(command);
        }
        return Err(err);
    }
    if let Some(command) = command {
        wait_daemon_toggle_command_response(&command)?;
    }
    Ok(())
}

fn finish_v2_command(result: crate::daemon::protocol_v2::TerminalCommandResult) -> Result<()> {
    match result {
        crate::daemon::protocol_v2::TerminalCommandResult::Succeeded => Ok(()),
        crate::daemon::protocol_v2::TerminalCommandResult::Canceled => {
            Err(anyhow!("daemon command was canceled before any effect"))
        }
        crate::daemon::protocol_v2::TerminalCommandResult::FailedNoEffect(reason) => {
            Err(anyhow!(reason))
        }
        crate::daemon::protocol_v2::TerminalCommandResult::AdmittedIndeterminate(reason) => {
            Err(anyhow!(
                "daemon command was admitted but its outcome is indeterminate; do not retry: {reason}"
            ))
        }
        crate::daemon::protocol_v2::TerminalCommandResult::CommittedIndeterminate(reason) => {
            Err(anyhow!("daemon command outcome is indeterminate: {reason}"))
        }
    }
}

pub(crate) fn send_daemon_overlay_action(action: TrayAction) -> Result<()> {
    send_daemon_toggle_request(&DaemonToggleRequest {
        overlay_action: Some(action),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests;
