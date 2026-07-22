use std::env;
use std::path::PathBuf;

use crate::env_vars::{
    HOME_ENV, USERPROFILE_ENV, XDG_CONFIG_HOME_ENV, XDG_DATA_HOME_ENV, XDG_PICTURES_DIR_ENV,
    XDG_RUNTIME_DIR_ENV,
};

/// Resolve the user's home directory.
pub fn home_dir() -> Option<PathBuf> {
    env::var_os(HOME_ENV)
        .or_else(|| env::var_os(USERPROFILE_ENV))
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
}

/// Resolve the XDG config directory, falling back to `~/.config`.
pub fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = env::var_os(XDG_CONFIG_HOME_ENV)
        && !dir.is_empty()
    {
        return Some(PathBuf::from(dir));
    }
    home_dir().map(|home| home.join(".config"))
}

/// Resolve the XDG data directory, falling back to `~/.local/share`.
pub fn data_dir() -> Option<PathBuf> {
    if let Some(dir) = env::var_os(XDG_DATA_HOME_ENV)
        && !dir.is_empty()
    {
        return Some(PathBuf::from(dir));
    }
    home_dir().map(|home| home.join(".local").join("share"))
}

/// Location for generated, persistent runtime UI preferences. This is kept
/// separate from the authored configuration and drawing-session stores.
pub(crate) fn runtime_ui_state_file() -> PathBuf {
    data_dir()
        .unwrap_or_else(|| home_dir().unwrap_or_else(fallback_runtime_root))
        .join("wayscriber")
        .join("runtime-ui.toml")
}

/// Best-effort pictures directory (XDG), falling back to `~/Pictures`.
pub fn pictures_dir() -> Option<PathBuf> {
    if let Some(dir) = env::var_os(XDG_PICTURES_DIR_ENV)
        && !dir.is_empty()
    {
        return Some(PathBuf::from(dir));
    }
    home_dir().map(|home| home.join("Pictures"))
}

/// Expand a path string that may start with `~/` into an absolute path.
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(stripped);
    }
    PathBuf::from(path)
}

fn fallback_runtime_root() -> PathBuf {
    std::env::temp_dir().join("wayscriber")
}

fn runtime_root() -> PathBuf {
    // Prefer XDG runtime dir for ephemeral files; fall back to data/home/temp for portability.
    #[cfg(unix)]
    if let Some(dir) = env::var_os(XDG_RUNTIME_DIR_ENV)
        && !dir.is_empty()
    {
        return PathBuf::from(dir).join("wayscriber");
    }

    data_dir()
        .unwrap_or_else(|| home_dir().unwrap_or_else(fallback_runtime_root))
        .join("wayscriber")
}

/// Location for transient tray commands.
/// Uses [`XDG_RUNTIME_DIR_ENV`] when available; falls back to data/home/temp.
pub fn tray_action_file() -> PathBuf {
    runtime_root().join("tray_action")
}

/// Location for queued transient tray commands.
/// Uses [`XDG_RUNTIME_DIR_ENV`] when available; falls back to data/home/temp.
pub fn tray_action_dir() -> PathBuf {
    runtime_root().join("tray-actions")
}

/// Location for transient daemon toggle requests.
/// Uses [`XDG_RUNTIME_DIR_ENV`] when available; falls back to data/home/temp.
pub fn daemon_command_file() -> PathBuf {
    runtime_root().join("daemon_command.json")
}

/// Location for queued daemon toggle requests.
/// Uses [`XDG_RUNTIME_DIR_ENV`] when available; falls back to data/home/temp.
pub fn daemon_command_dir() -> PathBuf {
    runtime_root().join("daemon-commands")
}

/// Location for the running daemon PID.
/// Uses [`XDG_RUNTIME_DIR_ENV`] when available; falls back to data/home/temp.
pub fn daemon_pid_file() -> PathBuf {
    runtime_root().join("wayscriber.pid")
}

/// Location for persistent logs.
#[allow(dead_code)]
pub fn log_dir() -> PathBuf {
    data_dir()
        .unwrap_or_else(|| home_dir().unwrap_or_else(fallback_runtime_root))
        .join("wayscriber")
        .join("logs")
}

/// Location for the daemon single-instance lock.
pub fn daemon_lock_file() -> PathBuf {
    runtime_root().join("wayscriber.lock")
}

/// Location for the active-overlay single-instance lock.
pub fn overlay_lock_file() -> PathBuf {
    runtime_root().join("wayscriber-overlay.lock")
}

#[cfg(test)]
mod tests;
