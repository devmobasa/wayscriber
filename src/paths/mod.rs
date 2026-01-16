use std::env;
use std::path::PathBuf;

/// Resolve the user's home directory.
pub fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
}

/// Resolve the XDG config directory, falling back to `~/.config`.
pub fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("XDG_CONFIG_HOME")
        && !dir.is_empty()
    {
        return Some(PathBuf::from(dir));
    }
    home_dir().map(|home| home.join(".config"))
}

/// Resolve the XDG data directory, falling back to `~/.local/share`.
pub fn data_dir() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("XDG_DATA_HOME")
        && !dir.is_empty()
    {
        return Some(PathBuf::from(dir));
    }
    home_dir().map(|home| home.join(".local").join("share"))
}

/// Best-effort pictures directory (XDG), falling back to `~/Pictures`.
pub fn pictures_dir() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("XDG_PICTURES_DIR")
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
    if let Some(dir) = env::var_os("XDG_RUNTIME_DIR")
        && !dir.is_empty()
    {
        return PathBuf::from(dir).join("wayscriber");
    }

    data_dir()
        .unwrap_or_else(|| home_dir().unwrap_or_else(fallback_runtime_root))
        .join("wayscriber")
}

/// Location for transient tray commands.
/// Uses XDG_RUNTIME_DIR when available; falls back to data/home/temp.
pub fn tray_action_file() -> PathBuf {
    runtime_root().join("tray_action")
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

#[cfg(test)]
mod tests;
