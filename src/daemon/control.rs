use anyhow::{Context, Result, anyhow};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(test)]
use crate::env_vars::XDG_RUNTIME_DIR_ENV;
use crate::paths::{daemon_command_dir, daemon_command_file, daemon_lock_file, daemon_pid_file};
use crate::session::try_lock_exclusive;
use crate::tray_action::TrayAction;

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

fn clear_file(path: &std::path::Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("failed to remove {}", path.display())),
    }
}

fn clear_dir(path: &std::path::Path) -> Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("failed to remove {}", path.display())),
    }
}

fn atomic_temp_path(path: &std::path::Path) -> Result<std::path::PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("{} has no parent directory", path.display()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow!("{} has no file name", path.display()))?
        .to_string_lossy();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Ok(parent.join(format!(
        ".{}.{}.{}.tmp",
        file_name,
        std::process::id(),
        stamp
    )))
}

fn next_daemon_toggle_request_path() -> Result<std::path::PathBuf> {
    let dir = daemon_command_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create runtime directory {}", dir.display()))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Ok(dir.join(format!("{:032x}-{:08x}.json", stamp, std::process::id())))
}

fn daemon_toggle_response_path_for_request(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("{} has no parent directory", path.display()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow!("{} has no file name", path.display()))?;
    Ok(parent.join("responses").join(file_name))
}

fn write_file_atomic(path: &std::path::Path, payload: &[u8]) -> Result<()> {
    let tmp_path = atomic_temp_path(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create runtime directory {}", parent.display()))?;
    }

    fs::write(&tmp_path, payload)
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;

    #[cfg(unix)]
    {
        if let Err(err) = fs::rename(&tmp_path, path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(err).with_context(|| {
                format!(
                    "failed to atomically replace {} via {}",
                    path.display(),
                    tmp_path.display()
                )
            });
        }
    }

    #[cfg(not(unix))]
    {
        let _ = fs::remove_file(path);
        if let Err(err) = fs::rename(&tmp_path, path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(err).with_context(|| {
                format!(
                    "failed to replace {} via {}",
                    path.display(),
                    tmp_path.display()
                )
            });
        }
    }

    Ok(())
}

pub(crate) fn clear_daemon_toggle_request_file() -> Result<()> {
    clear_file(&daemon_command_file())?;
    clear_dir(&daemon_command_dir())
}

fn write_daemon_toggle_request(
    request: &DaemonToggleRequest,
    daemon_token: &str,
) -> Result<DaemonToggleCommand> {
    let envelope = DaemonToggleEnvelope {
        daemon_token: daemon_token.to_string(),
        requested_at_unix_ms: current_unix_millis()?,
        canceled: false,
        request: request.clone(),
    };
    let payload =
        serde_json::to_vec(&envelope).context("failed to serialize daemon toggle request")?;
    let path = next_daemon_toggle_request_path()?;
    write_file_atomic(&path, &payload)?;
    let response_path = daemon_toggle_response_path_for_request(&path)?;
    Ok(DaemonToggleCommand {
        daemon_token: daemon_token.to_string(),
        request: request.clone(),
        request_path: path,
        response_path,
    })
}

pub(crate) fn take_daemon_toggle_requests(expected_token: &str) -> Result<DaemonToggleCommands> {
    let dir = daemon_command_dir();
    let mut paths = match fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.is_file())
            .collect::<Vec<_>>(),
        Err(err) if err.kind() == ErrorKind::NotFound => Vec::new(),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", dir.display()));
        }
    };
    paths.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    let saw_command_files = !paths.is_empty();

    let mut requests = Vec::new();
    for path in paths {
        let payload = match fs::read(&path) {
            Ok(payload) => payload,
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
            Err(err) => {
                warn!("Failed to read {}: {}", path.display(), err);
                continue;
            }
        };

        if let Err(err) = clear_file(&path) {
            warn!("Failed to remove {}: {}", path.display(), err);
            continue;
        }

        let envelope: DaemonToggleEnvelope = match serde_json::from_slice(&payload) {
            Ok(envelope) => envelope,
            Err(err) => {
                warn!(
                    "Ignoring malformed daemon toggle request {}: {}",
                    path.display(),
                    err
                );
                continue;
            }
        };

        if envelope.daemon_token != expected_token {
            warn!("Ignoring daemon toggle request for a different daemon instance");
            continue;
        }

        if envelope.canceled {
            warn!("Ignoring canceled daemon toggle request");
            continue;
        }

        let age_ms = current_unix_millis()?.saturating_sub(envelope.requested_at_unix_ms);
        if Duration::from_millis(age_ms) > MAX_DAEMON_TOGGLE_REQUEST_AGE {
            warn!("Ignoring stale daemon toggle request older than 5s");
            continue;
        }

        requests.push(DaemonToggleCommand {
            daemon_token: envelope.daemon_token,
            request: envelope.request,
            response_path: daemon_toggle_response_path_for_request(&path)?,
            request_path: path,
        });
    }

    Ok(DaemonToggleCommands {
        commands: requests,
        saw_command_files,
    })
}

pub(crate) fn write_daemon_toggle_command_success(command: &DaemonToggleCommand) -> Result<()> {
    write_daemon_toggle_response(&command.response_path, DaemonToggleResponse::ok())
}

pub(crate) fn write_daemon_toggle_command_error(
    command: &DaemonToggleCommand,
    message: &str,
) -> Result<()> {
    write_daemon_toggle_response(
        &command.response_path,
        DaemonToggleResponse::error(message.to_string()),
    )
}

fn write_daemon_toggle_response(path: &Path, response: DaemonToggleResponse) -> Result<()> {
    let payload =
        serde_json::to_vec(&response).context("failed to serialize daemon toggle response")?;
    write_file_atomic(path, &payload)
}

#[cfg(test)]
pub(crate) fn read_daemon_toggle_response(path: &Path) -> Result<()> {
    let payload = fs::read(path)
        .with_context(|| format!("failed to read daemon response {}", path.display()))?;
    let response: DaemonToggleResponse =
        serde_json::from_slice(&payload).context("failed to parse daemon toggle response")?;
    response.into_result()
}

fn wait_daemon_toggle_response_for(path: &Path, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match fs::read(path) {
            Ok(payload) => {
                let response: DaemonToggleResponse = serde_json::from_slice(&payload)
                    .context("failed to parse daemon toggle response")?;
                if let Err(err) = clear_file(path) {
                    warn!(
                        "Failed to remove daemon response {}: {}",
                        path.display(),
                        err
                    );
                }
                return response.into_result();
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                if Instant::now() >= deadline {
                    return Err(anyhow!(
                        "timed out waiting for wayscriber daemon to process toggle request"
                    ));
                }
                thread::sleep(DAEMON_TOGGLE_RESPONSE_POLL);
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to read daemon response {}", path.display()));
            }
        }
    }
}

fn wait_daemon_toggle_command_response(command: &DaemonToggleCommand) -> Result<()> {
    wait_daemon_toggle_command_response_for(command, MAX_DAEMON_TOGGLE_RESPONSE_WAIT)
}

fn wait_daemon_toggle_command_response_for(
    command: &DaemonToggleCommand,
    timeout: Duration,
) -> Result<()> {
    if let Err(err) = wait_daemon_toggle_response_for(&command.response_path, timeout) {
        cancel_daemon_toggle_command(command);
        return Err(err);
    }
    Ok(())
}

fn cancel_daemon_toggle_command(command: &DaemonToggleCommand) {
    let payload = match canceled_daemon_toggle_payload(command) {
        Ok(payload) => payload,
        Err(err) => {
            warn!("Failed to serialize daemon cancellation command: {}", err);
            clear_daemon_toggle_command_files(command);
            return;
        }
    };

    match OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&command.request_path)
    {
        Ok(mut file) => {
            if let Err(err) = file.write_all(&payload) {
                warn!(
                    "Failed to mark daemon request {} canceled after failed toggle wait: {}",
                    command.request_path.display(),
                    err
                );
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => warn!(
            "Failed to open daemon request {} for cancellation after failed toggle wait: {}",
            command.request_path.display(),
            err
        ),
    }

    if let Err(err) = clear_file(&command.response_path) {
        warn!(
            "Failed to remove daemon response {} after failed toggle wait: {}",
            command.response_path.display(),
            err
        );
    }
}

fn canceled_daemon_toggle_payload(command: &DaemonToggleCommand) -> Result<Vec<u8>> {
    let envelope = DaemonToggleEnvelope {
        daemon_token: command.daemon_token.clone(),
        requested_at_unix_ms: current_unix_millis()?,
        canceled: true,
        request: DaemonToggleRequest::default(),
    };
    serde_json::to_vec(&envelope).context("failed to serialize canceled daemon toggle request")
}

fn clear_daemon_toggle_command_files(command: &DaemonToggleCommand) {
    if let Err(err) = clear_file(&command.request_path) {
        warn!(
            "Failed to remove daemon request {} after failed toggle wait: {}",
            command.request_path.display(),
            err
        );
    }
    if let Err(err) = clear_file(&command.response_path) {
        warn!(
            "Failed to remove daemon response {} after failed toggle wait: {}",
            command.response_path.display(),
            err
        );
    }
}

pub(crate) fn write_daemon_pid_file(pid: u32, token: &str) -> Result<()> {
    let path = daemon_pid_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create runtime directory {}", parent.display()))?;
    }
    let payload = serde_json::to_vec(&DaemonRuntimeInfo {
        pid,
        token: Some(token.to_string()),
    })
    .context("failed to serialize daemon pid file")?;
    fs::write(&path, payload).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub(crate) fn clear_daemon_pid_file() -> Result<()> {
    clear_file(&daemon_pid_file())
}

fn try_acquire_daemon_lock() -> Result<Option<File>> {
    let path = daemon_lock_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create runtime directory {}", parent.display()))?;
    }

    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("failed to open daemon lock {}", path.display()))?;

    match try_lock_exclusive(&lock_file) {
        Ok(()) => Ok(Some(lock_file)),
        Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(None),
        Err(err) => Err(err).context("failed to inspect daemon lock"),
    }
}

fn clear_stale_daemon_state() {
    if let Err(err) = clear_daemon_pid_file() {
        warn!("Failed to clear stale daemon pid file: {}", err);
    }
    if let Err(err) = clear_daemon_toggle_request_file() {
        warn!("Failed to clear stale daemon command file: {}", err);
    }
}

fn parse_daemon_runtime_info(raw: &str) -> Result<DaemonRuntimeInfo> {
    if let Ok(info) = serde_json::from_str::<DaemonRuntimeInfo>(raw) {
        return Ok(info);
    }

    let pid = raw
        .trim()
        .parse::<u32>()
        .context("failed to parse daemon pid file")?;
    Ok(DaemonRuntimeInfo { pid, token: None })
}

fn read_daemon_runtime_file() -> Result<DaemonRuntimeInfo> {
    let path = daemon_pid_file();
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    parse_daemon_runtime_info(&raw)
}

fn read_daemon_runtime_file_if_exists() -> Result<Option<DaemonRuntimeInfo>> {
    let path = daemon_pid_file();
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    parse_daemon_runtime_info(&raw).map(Some)
}

fn clear_stale_daemon_state_if_matches(expected: &DaemonRuntimeInfo) {
    let Some(_lock_file) = (match try_acquire_daemon_lock() {
        Ok(lock_file) => lock_file,
        Err(err) => {
            warn!(
                "Failed to inspect daemon lock before stale cleanup: {}",
                err
            );
            return;
        }
    }) else {
        return;
    };

    match read_daemon_runtime_file_if_exists() {
        Ok(Some(current)) if &current == expected => clear_stale_daemon_state(),
        Ok(_) => {}
        Err(err) => warn!("Failed to inspect daemon pid before stale cleanup: {}", err),
    }
}

fn read_daemon_runtime_info() -> Result<DaemonRuntimeInfo> {
    if let Some(_lock_file) = try_acquire_daemon_lock()? {
        clear_stale_daemon_state();
        return Err(anyhow!("wayscriber daemon is not running"));
    }

    read_daemon_runtime_file()
}

fn signal_daemon_pid(pid: u32) -> Result<()> {
    #[cfg(unix)]
    {
        let pid = i32::try_from(pid).context("daemon pid does not fit into i32")?;
        if pid <= 0 {
            return Err(anyhow!("invalid daemon pid {}", pid));
        }

        if unsafe { libc::kill(pid, libc::SIGUSR1) } != 0 {
            return Err(anyhow!(
                "failed to signal wayscriber daemon {}: {}",
                pid,
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    #[cfg(not(unix))]
    {
        Err(anyhow!(
            "daemon control is only supported on Unix platforms"
        ))
    }
}

pub(crate) fn send_daemon_toggle_request(request: &DaemonToggleRequest) -> Result<()> {
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

pub(crate) fn send_daemon_overlay_action(action: TrayAction) -> Result<()> {
    send_daemon_toggle_request(&DaemonToggleRequest {
        overlay_action: Some(action),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn empty_toggle_request_reports_empty() {
        assert!(DaemonToggleRequest::default().is_empty());
    }

    #[test]
    fn overlay_action_request_is_not_empty() {
        let request = DaemonToggleRequest {
            overlay_action: Some(TrayAction::LightDrawToggle),
            ..Default::default()
        };
        assert!(!request.is_empty());
    }

    #[test]
    fn session_file_request_is_not_empty() {
        let request = DaemonToggleRequest {
            session_file: Some(PathBuf::from("/tmp/lecture.wayscriber-session")),
            ..Default::default()
        };
        assert!(!request.is_empty());
    }

    #[test]
    fn toggle_request_reports_session_override() {
        let request = DaemonToggleRequest {
            resume_session: true,
            ..Default::default()
        };
        assert_eq!(request.session_resume_override(), Some(true));

        let request = DaemonToggleRequest {
            no_resume_session: true,
            ..Default::default()
        };
        assert_eq!(request.session_resume_override(), Some(false));
    }

    #[test]
    fn session_file_request_rejects_no_resume_session() {
        let mut request = DaemonToggleRequest {
            no_resume_session: true,
            session_file: Some(PathBuf::from("/tmp/lecture.wayscriber-session")),
            ..Default::default()
        };

        let err = request
            .normalize_and_validate_session_file()
            .expect_err("session file conflicts with disabled resume");

        assert!(
            format!("{err:#}").contains("--session-file conflicts with --no-resume-session"),
            "{err:#}"
        );
    }

    #[test]
    fn session_file_request_rejects_relative_path() {
        let mut request = DaemonToggleRequest {
            session_file: Some(PathBuf::from("lecture.wayscriber-session")),
            ..Default::default()
        };

        let err = request
            .normalize_and_validate_session_file()
            .expect_err("daemon protocol requires anchored paths");

        assert!(
            format!("{err:#}").contains("daemon --session-file request must use an absolute path"),
            "{err:#}"
        );
    }

    #[test]
    fn daemon_toggle_response_round_trips_error_and_is_removed_after_wait() {
        let tmp = crate::test_temp::tempdir().unwrap();
        let command = DaemonToggleCommand {
            daemon_token: "daemon-token".into(),
            request: DaemonToggleRequest::default(),
            request_path: tmp.path().join("request.json"),
            response_path: tmp.path().join("responses").join("request.json"),
        };

        write_daemon_toggle_command_error(&command, "cannot switch target").unwrap();

        let err = wait_daemon_toggle_response_for(
            &command.response_path,
            MAX_DAEMON_TOGGLE_RESPONSE_WAIT,
        )
        .expect_err("error response should surface to caller");
        assert!(
            format!("{err:#}").contains("cannot switch target"),
            "{err:#}"
        );
        assert!(!command.response_path.exists());
    }

    #[test]
    fn daemon_toggle_response_round_trips_success_and_is_removed_after_wait() {
        let tmp = crate::test_temp::tempdir().unwrap();
        let command = DaemonToggleCommand {
            daemon_token: "daemon-token".into(),
            request: DaemonToggleRequest::default(),
            request_path: tmp.path().join("request.json"),
            response_path: tmp.path().join("responses").join("request.json"),
        };

        write_daemon_toggle_command_success(&command).unwrap();

        wait_daemon_toggle_response_for(&command.response_path, MAX_DAEMON_TOGGLE_RESPONSE_WAIT)
            .unwrap();
        assert!(!command.response_path.exists());
    }

    #[test]
    fn daemon_toggle_response_wait_covers_request_age_and_overlay_stop_grace() {
        assert!(
            MAX_DAEMON_TOGGLE_RESPONSE_WAIT
                > MAX_DAEMON_TOGGLE_REQUEST_AGE + Duration::from_secs(2),
            "typed toggle response wait should exceed accepted request age plus overlay stop grace"
        );
    }

    #[test]
    fn daemon_toggle_response_wait_error_marks_existing_request_canceled() {
        let tmp = crate::test_temp::tempdir().unwrap();
        let command = DaemonToggleCommand {
            daemon_token: "daemon-token".into(),
            request: DaemonToggleRequest::default(),
            request_path: tmp.path().join("request.json"),
            response_path: tmp.path().join("responses").join("request.json"),
        };
        fs::write(&command.request_path, b"pending request").unwrap();

        let err = wait_daemon_toggle_command_response_for(&command, Duration::ZERO)
            .expect_err("missing response should time out immediately");

        assert!(
            format!("{err:#}")
                .contains("timed out waiting for wayscriber daemon to process toggle request"),
            "{err:#}"
        );
        let canceled: DaemonToggleEnvelope =
            serde_json::from_slice(&fs::read(&command.request_path).unwrap()).unwrap();
        assert_eq!(canceled.daemon_token, "daemon-token");
        assert!(canceled.canceled);
        assert!(!command.response_path.exists());
    }

    #[test]
    fn daemon_toggle_response_wait_error_does_not_create_missing_cancel_request() {
        let tmp = crate::test_temp::tempdir().unwrap();
        let command = DaemonToggleCommand {
            daemon_token: "daemon-token".into(),
            request: DaemonToggleRequest::default(),
            request_path: tmp.path().join("request.json"),
            response_path: tmp.path().join("responses").join("request.json"),
        };

        let err = wait_daemon_toggle_command_response_for(&command, Duration::ZERO)
            .expect_err("missing response should time out immediately");

        assert!(
            format!("{err:#}")
                .contains("timed out waiting for wayscriber daemon to process toggle request"),
            "{err:#}"
        );
        assert!(!command.request_path.exists());
        assert!(!command.response_path.exists());
    }

    #[test]
    fn daemon_toggle_response_parse_error_marks_existing_request_canceled() {
        let tmp = crate::test_temp::tempdir().unwrap();
        let command = DaemonToggleCommand {
            daemon_token: "daemon-token".into(),
            request: DaemonToggleRequest::default(),
            request_path: tmp.path().join("request.json"),
            response_path: tmp.path().join("responses").join("request.json"),
        };
        fs::write(&command.request_path, b"pending request").unwrap();
        fs::create_dir_all(command.response_path.parent().unwrap()).unwrap();
        fs::write(&command.response_path, b"not json").unwrap();

        let err = wait_daemon_toggle_command_response_for(&command, Duration::ZERO)
            .expect_err("malformed response should preserve parse error");

        assert!(
            format!("{err:#}").contains("failed to parse daemon toggle response"),
            "{err:#}"
        );
        let canceled: DaemonToggleEnvelope =
            serde_json::from_slice(&fs::read(&command.request_path).unwrap()).unwrap();
        assert!(canceled.canceled);
        assert!(!command.response_path.exists());
    }

    #[test]
    fn daemon_pid_file_round_trips_runtime_info() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = crate::test_temp::tempdir().unwrap();
        let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
        unsafe {
            env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
        }

        write_daemon_pid_file(1234, "daemon-token").unwrap();
        let info = read_daemon_runtime_info().unwrap_err();
        assert!(
            info.to_string()
                .contains("wayscriber daemon is not running")
        );

        match prev {
            Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
            None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
        }
    }

    #[test]
    fn stale_cleanup_removes_matching_runtime_while_lock_is_free() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = crate::test_temp::tempdir().unwrap();
        let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
        unsafe {
            env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
        }

        let runtime = DaemonRuntimeInfo {
            pid: 1234,
            token: Some("old-token".into()),
        };
        write_daemon_pid_file(runtime.pid, runtime.token.as_deref().unwrap()).unwrap();
        write_daemon_toggle_request(
            &DaemonToggleRequest {
                freeze: true,
                ..Default::default()
            },
            "old-token",
        )
        .unwrap();

        clear_stale_daemon_state_if_matches(&runtime);

        assert!(!daemon_pid_file().exists());
        assert!(!daemon_command_dir().exists());

        match prev {
            Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
            None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
        }
    }

    #[test]
    fn stale_cleanup_preserves_mismatched_runtime() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = crate::test_temp::tempdir().unwrap();
        let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
        unsafe {
            env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
        }

        let current = DaemonRuntimeInfo {
            pid: 5678,
            token: Some("new-token".into()),
        };
        write_daemon_pid_file(current.pid, current.token.as_deref().unwrap()).unwrap();
        write_daemon_toggle_request(
            &DaemonToggleRequest {
                freeze: true,
                ..Default::default()
            },
            "new-token",
        )
        .unwrap();

        clear_stale_daemon_state_if_matches(&DaemonRuntimeInfo {
            pid: 1234,
            token: Some("old-token".into()),
        });

        assert_eq!(read_daemon_runtime_file().unwrap(), current);
        assert!(daemon_command_dir().exists());

        match prev {
            Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
            None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
        }
    }

    #[test]
    fn take_daemon_toggle_request_round_trips_payload() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = crate::test_temp::tempdir().unwrap();
        let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
        unsafe {
            env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
        }

        let request = DaemonToggleRequest {
            mode: Some("whiteboard".into()),
            freeze: true,
            exit_after_capture: true,
            session_file: Some(PathBuf::from("/tmp/lecture.wayscriber-session")),
            ..Default::default()
        };
        write_daemon_toggle_request(&request, "daemon-token").unwrap();
        let batch = take_daemon_toggle_requests("daemon-token").unwrap();
        let requests = batch
            .commands
            .iter()
            .map(|command| command.request.clone())
            .collect::<Vec<_>>();
        assert_eq!(requests, vec![request]);
        assert!(batch.saw_command_files);
        assert_eq!(batch.commands.len(), 1);
        assert_eq!(batch.commands[0].daemon_token, "daemon-token");
        assert!(
            batch.commands[0]
                .request_path
                .starts_with(daemon_command_dir())
        );
        assert!(
            batch.commands[0]
                .response_path
                .starts_with(daemon_command_dir().join("responses"))
        );
        let batch = take_daemon_toggle_requests("daemon-token").unwrap();
        assert!(!batch.saw_command_files);
        assert!(batch.commands.is_empty());

        match prev {
            Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
            None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
        }
    }

    #[test]
    fn write_daemon_toggle_request_queues_multiple_files_without_leaking_temp_files() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = crate::test_temp::tempdir().unwrap();
        let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
        unsafe {
            env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
        }

        write_daemon_toggle_request(
            &DaemonToggleRequest {
                freeze: true,
                ..Default::default()
            },
            "daemon-token",
        )
        .unwrap();
        write_daemon_toggle_request(
            &DaemonToggleRequest {
                mode: Some("whiteboard".into()),
                ..Default::default()
            },
            "daemon-token",
        )
        .unwrap();

        let command_dir = daemon_command_dir();
        let entries = fs::read_dir(&command_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|name| name.ends_with(".json")));
        assert!(!entries.iter().any(|name| name.ends_with(".tmp")));

        match prev {
            Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
            None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
        }
    }

    #[test]
    fn take_daemon_toggle_request_drains_multiple_payloads_in_order() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = crate::test_temp::tempdir().unwrap();
        let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
        unsafe {
            env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
        }

        let first = DaemonToggleRequest {
            freeze: true,
            ..Default::default()
        };
        let second = DaemonToggleRequest {
            mode: Some("whiteboard".into()),
            ..Default::default()
        };
        write_daemon_toggle_request(&first, "daemon-token").unwrap();
        write_daemon_toggle_request(&second, "daemon-token").unwrap();

        let batch = take_daemon_toggle_requests("daemon-token").unwrap();
        let requests = batch
            .commands
            .into_iter()
            .map(|command| command.request)
            .collect::<Vec<_>>();
        assert_eq!(requests, vec![first, second]);
        assert!(batch.saw_command_files);
        assert!(
            daemon_command_dir()
                .read_dir()
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(true)
        );

        match prev {
            Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
            None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
        }
    }

    #[test]
    fn take_daemon_toggle_request_ignores_mismatched_token() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = crate::test_temp::tempdir().unwrap();
        let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
        unsafe {
            env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
        }

        write_daemon_toggle_request(
            &DaemonToggleRequest {
                freeze: true,
                ..Default::default()
            },
            "other-daemon",
        )
        .unwrap();

        assert!(
            take_daemon_toggle_requests("daemon-token")
                .unwrap()
                .commands
                .is_empty()
        );

        match prev {
            Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
            None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
        }
    }

    #[test]
    fn take_daemon_toggle_request_ignores_stale_payload() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = crate::test_temp::tempdir().unwrap();
        let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
        unsafe {
            env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
        }

        let payload = serde_json::to_vec(&DaemonToggleEnvelope {
            daemon_token: "daemon-token".into(),
            requested_at_unix_ms: current_unix_millis().unwrap() - 60_000,
            canceled: false,
            request: DaemonToggleRequest {
                freeze: true,
                ..Default::default()
            },
        })
        .unwrap();
        fs::create_dir_all(daemon_command_dir()).unwrap();
        fs::write(daemon_command_dir().join("stale.json"), payload).unwrap();

        let batch = take_daemon_toggle_requests("daemon-token").unwrap();
        assert!(batch.saw_command_files);
        assert!(batch.commands.is_empty());

        match prev {
            Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
            None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
        }
    }

    #[test]
    fn take_daemon_toggle_request_ignores_canceled_payload_but_marks_typed_signal() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = crate::test_temp::tempdir().unwrap();
        let prev = env::var_os(XDG_RUNTIME_DIR_ENV);
        unsafe {
            env::set_var(XDG_RUNTIME_DIR_ENV, tmp.path());
        }

        let payload = serde_json::to_vec(&DaemonToggleEnvelope {
            daemon_token: "daemon-token".into(),
            requested_at_unix_ms: current_unix_millis().unwrap(),
            canceled: true,
            request: DaemonToggleRequest {
                freeze: true,
                ..Default::default()
            },
        })
        .unwrap();
        fs::create_dir_all(daemon_command_dir()).unwrap();
        fs::write(daemon_command_dir().join("canceled.json"), payload).unwrap();

        let batch = take_daemon_toggle_requests("daemon-token").unwrap();
        assert!(batch.saw_command_files);
        assert!(batch.commands.is_empty());

        match prev {
            Some(value) => unsafe { env::set_var(XDG_RUNTIME_DIR_ENV, value) },
            None => unsafe { env::remove_var(XDG_RUNTIME_DIR_ENV) },
        }
    }
}
