use super::DaemonRuntimeInfo;
use super::queue::{clear_daemon_toggle_request_file, clear_file, write_file_atomic};
use crate::paths::{daemon_lock_file, daemon_pid_file};
use crate::session::try_lock_exclusive;
use anyhow::{Context, Result, anyhow};
use log::warn;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::ErrorKind;

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
    write_file_atomic(&path, &payload)?;
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

pub(super) fn read_daemon_runtime_file() -> Result<DaemonRuntimeInfo> {
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

pub(super) fn clear_stale_daemon_state_if_matches(expected: &DaemonRuntimeInfo) {
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

pub(super) fn read_daemon_runtime_info() -> Result<DaemonRuntimeInfo> {
    if let Some(_lock_file) = try_acquire_daemon_lock()? {
        clear_stale_daemon_state();
        return Err(anyhow!("wayscriber daemon is not running"));
    }

    read_daemon_runtime_file()
}

pub(super) fn signal_daemon_pid(pid: u32) -> Result<()> {
    #[cfg(unix)]
    {
        let pid = i32::try_from(pid).context("daemon pid does not fit into i32")?;
        if pid <= 0 {
            return Err(anyhow!("invalid daemon pid {}", pid));
        }

        // SAFETY: `pid` has been checked to be a positive Unix process id and
        // `SIGUSR1` is a valid signal constant.
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
