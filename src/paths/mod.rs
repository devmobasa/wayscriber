use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use crate::env_vars::{
    HOME_ENV, USERPROFILE_ENV, XDG_CACHE_HOME_ENV, XDG_CONFIG_HOME_ENV, XDG_DATA_DIRS_ENV,
    XDG_DATA_HOME_ENV, XDG_PICTURES_DIR_ENV, XDG_RUNTIME_DIR_ENV,
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

/// Resolve the XDG cache directory, falling back to `~/.cache`.
pub fn cache_dir() -> Option<PathBuf> {
    if let Some(dir) = env::var_os(XDG_CACHE_HOME_ENV)
        && !dir.is_empty()
    {
        return Some(PathBuf::from(dir));
    }
    home_dir().map(|home| home.join(".cache"))
}

/// Location for the last update-check result. Cache, not data: it is
/// regenerable, and deleting it only costs one extra request.
pub fn update_check_cache_file() -> PathBuf {
    cache_dir()
        .unwrap_or_else(|| home_dir().unwrap_or_else(fallback_runtime_root))
        .join("wayscriber")
        .join("update-check.json")
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

/// The data directories a program following the XDG base directory spec
/// searches, most important first: the data home, then the system data
/// directories.
///
/// Unlike [`data_dir`], this ignores relative values, as the spec requires, so
/// the result does not depend on the current directory.
pub fn searched_data_dirs() -> Vec<PathBuf> {
    searched_data_dirs_from(
        env::var_os(XDG_DATA_HOME_ENV),
        home_dir(),
        env::var_os(XDG_DATA_DIRS_ENV),
    )
}

/// Reads `XDG_DATA_HOME` and `XDG_DATA_DIRS` values as the base directory spec
/// does: an unset, empty, or relative data home means `~/.local/share`.
fn searched_data_dirs_from(
    data_home: Option<OsString>,
    home: Option<PathBuf>,
    data_dirs: Option<OsString>,
) -> Vec<PathBuf> {
    let data_home = data_home
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .or_else(|| home.map(|home| home.join(".local").join("share")))
        .filter(|dir| dir.is_absolute());

    data_home
        .into_iter()
        .chain(system_data_dirs_from(data_dirs))
        .collect()
}

/// Parses an `XDG_DATA_DIRS` value as the base directory spec reads it: an
/// unset or empty value means the defaults, and relative entries are ignored.
fn system_data_dirs_from(value: Option<OsString>) -> Vec<PathBuf> {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return vec![
            PathBuf::from("/usr/local/share"),
            PathBuf::from("/usr/share"),
        ];
    };
    env::split_paths(&value)
        .filter(|dir| dir.is_absolute())
        .collect()
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

/// True when `name` is a single relative file name, not a path.
///
/// Rejects empty names, `.`, `..`, NULs, slashes, and anything `Path::join`
/// would treat as leaving or replacing the parent directory.
pub fn is_single_path_component(name: &str) -> bool {
    if name.is_empty() || name.contains('\0') || name.contains('/') || name.contains('\\') {
        return false;
    }
    let path = std::path::Path::new(name);
    if path.is_absolute() {
        return false;
    }
    let mut components = path.components();
    match components.next() {
        Some(std::path::Component::Normal(os_name)) => {
            components.next().is_none()
                && os_name.to_str().is_some_and(|component| {
                    component == name && component != "." && component != ".."
                })
        }
        _ => false,
    }
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
