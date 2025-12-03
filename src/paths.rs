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
    if let Some(dir) = env::var_os("XDG_CONFIG_HOME") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    home_dir().map(|home| home.join(".config"))
}

/// Resolve the XDG data directory, falling back to `~/.local/share`.
pub fn data_dir() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("XDG_DATA_HOME") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    home_dir().map(|home| home.join(".local").join("share"))
}

/// Best-effort pictures directory (XDG), falling back to `~/Pictures`.
pub fn pictures_dir() -> Option<PathBuf> {
    if let Some(dir) = env::var_os("XDG_PICTURES_DIR") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    home_dir().map(|home| home.join("Pictures"))
}

/// Expand a path string that may start with `~/` into an absolute path.
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(stripped);
        }
    }
    PathBuf::from(path)
}

fn fallback_runtime_root() -> PathBuf {
    std::env::temp_dir().join("wayscriber")
}

fn runtime_root() -> PathBuf {
    // Prefer XDG runtime dir for ephemeral files; fall back to data/home/temp for portability.
    #[cfg(unix)]
    if let Some(dir) = env::var_os("XDG_RUNTIME_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir).join("wayscriber");
        }
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

/// Location to open when showing logs or runtime artifacts.
pub fn log_dir() -> PathBuf {
    runtime_root().join("logs")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    #[cfg(unix)]
    fn tray_action_prefers_runtime_dir_when_set() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let prev = env::var_os("XDG_RUNTIME_DIR");
        // SAFETY: serialised via ENV_MUTEX
        unsafe {
            env::set_var("XDG_RUNTIME_DIR", tmp.path());
        }

        let path = tray_action_file();
        assert!(path.starts_with(tmp.path()));

        if let Some(prev) = prev {
            unsafe {
                env::set_var("XDG_RUNTIME_DIR", prev);
            }
        } else {
            unsafe {
                env::remove_var("XDG_RUNTIME_DIR");
            }
        }
    }

    #[test]
    fn config_dir_prefers_xdg_config_home_when_set() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let tmp = tempfile::tempdir().unwrap();
        let prev_home = env::var_os("HOME");
        let prev_userprofile = env::var_os("USERPROFILE");
        let prev_xdg = env::var_os("XDG_CONFIG_HOME");

        unsafe {
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::remove_var("HOME");
            env::remove_var("USERPROFILE");
        }

        let dir = config_dir().expect("config_dir should resolve from XDG_CONFIG_HOME");
        assert_eq!(dir, tmp.path());

        match prev_xdg {
            Some(v) => unsafe { env::set_var("XDG_CONFIG_HOME", v) },
            None => unsafe { env::remove_var("XDG_CONFIG_HOME") },
        }
        match prev_home {
            Some(v) => unsafe { env::set_var("HOME", v) },
            None => unsafe { env::remove_var("HOME") },
        }
        match prev_userprofile {
            Some(v) => unsafe { env::set_var("USERPROFILE", v) },
            None => unsafe { env::remove_var("USERPROFILE") },
        }
    }

    #[test]
    fn config_dir_falls_back_to_home_config() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let tmp = tempfile::tempdir().unwrap();
        let prev_home = env::var_os("HOME");
        let prev_userprofile = env::var_os("USERPROFILE");
        let prev_xdg = env::var_os("XDG_CONFIG_HOME");

        unsafe {
            env::set_var("HOME", tmp.path());
            env::remove_var("USERPROFILE");
            env::remove_var("XDG_CONFIG_HOME");
        }

        let dir = config_dir().expect("config_dir should resolve from HOME");
        assert_eq!(dir, tmp.path().join(".config"));

        match prev_xdg {
            Some(v) => unsafe { env::set_var("XDG_CONFIG_HOME", v) },
            None => unsafe { env::remove_var("XDG_CONFIG_HOME") },
        }
        match prev_home {
            Some(v) => unsafe { env::set_var("HOME", v) },
            None => unsafe { env::remove_var("HOME") },
        }
        match prev_userprofile {
            Some(v) => unsafe { env::set_var("USERPROFILE", v) },
            None => unsafe { env::remove_var("USERPROFILE") },
        }
    }

    #[test]
    fn expand_tilde_expands_home_prefix() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let tmp = tempfile::tempdir().unwrap();
        let prev_home = env::var_os("HOME");
        let prev_userprofile = env::var_os("USERPROFILE");

        unsafe {
            env::set_var("HOME", tmp.path());
            env::remove_var("USERPROFILE");
        }

        let expanded = expand_tilde("~/my/config");
        assert_eq!(expanded, tmp.path().join("my/config"));

        match prev_home {
            Some(v) => unsafe { env::set_var("HOME", v) },
            None => unsafe { env::remove_var("HOME") },
        }
        match prev_userprofile {
            Some(v) => unsafe { env::set_var("USERPROFILE", v) },
            None => unsafe { env::remove_var("USERPROFILE") },
        }
    }
}
