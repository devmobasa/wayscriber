use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use wayscriber::systemd_user_service::{
    USER_SERVICE_NAME, escape_systemd_env_value as shared_escape_systemd_env_value,
    portal_shortcut_dropin_path as shared_portal_shortcut_dropin_path, render_user_service_unit,
    user_service_unit_path as shared_user_service_unit_path,
};

use super::command::{command_available, find_in_path, run_command, run_command_checked};

pub(super) const SERVICE_NAME: &str = USER_SERVICE_NAME;

pub(super) fn detect_service_unit_path(systemctl_available: bool) -> Option<PathBuf> {
    if systemctl_available {
        let capture = run_command(
            "systemctl",
            &[
                "--user",
                "show",
                "--property=FragmentPath",
                "--value",
                SERVICE_NAME,
            ],
        )
        .ok()?;
        if capture.success {
            let trimmed = capture.stdout.trim();
            if !trimmed.is_empty() && trimmed != "-" {
                return Some(PathBuf::from(trimmed));
            }
        }
    }

    if let Some(path) = user_service_unit_path()
        && path.exists()
    {
        return Some(path);
    }

    package_service_paths()
        .into_iter()
        .find(|path| path.exists())
}

pub(super) fn query_service_enabled() -> bool {
    let capture = match run_command("systemctl", &["--user", "is-enabled", SERVICE_NAME]) {
        Ok(capture) => capture,
        Err(_) => return false,
    };
    if !capture.success {
        return false;
    }
    let value = capture.stdout.trim();
    matches!(value, "enabled" | "enabled-runtime" | "linked")
}

pub(super) fn query_service_active() -> bool {
    let capture = match run_command("systemctl", &["--user", "is-active", SERVICE_NAME]) {
        Ok(capture) => capture,
        Err(_) => return false,
    };
    capture.success && capture.stdout.trim() == "active"
}

pub(super) fn require_systemctl_available() -> Result<(), String> {
    if command_available("systemctl") {
        Ok(())
    } else {
        Err("systemctl is not available in PATH.".to_string())
    }
}

pub(super) fn run_systemctl_user(args: &[&str]) -> Result<(), String> {
    let mut full_args = Vec::with_capacity(args.len() + 1);
    full_args.push("--user");
    full_args.extend_from_slice(args);
    let _ = run_command_checked("systemctl", &full_args)?;
    Ok(())
}

pub(super) fn user_service_unit_path() -> Option<PathBuf> {
    shared_user_service_unit_path()
}

pub(super) fn portal_shortcut_dropin_path() -> Option<PathBuf> {
    shared_portal_shortcut_dropin_path()
}

pub(super) fn install_or_update_user_service() -> Result<PathBuf, String> {
    let binary_path = resolve_wayscriber_binary_path()?;

    let service_path = user_service_unit_path().ok_or_else(|| {
        "Cannot resolve home directory; failed to determine user systemd service path.".to_string()
    })?;
    let service_dir = service_path
        .parent()
        .ok_or_else(|| "Invalid user service path".to_string())?;
    fs::create_dir_all(service_dir).map_err(|err| {
        format!(
            "Failed to create user service directory {}: {}",
            service_dir.display(),
            err
        )
    })?;

    let contents = render_user_service_file(&binary_path);
    fs::write(&service_path, contents).map_err(|err| {
        format!(
            "Failed to write user service file {}: {}",
            service_path.display(),
            err
        )
    })?;

    if command_available("systemctl") {
        run_systemctl_user(&["daemon-reload"])?;
    }

    Ok(service_path)
}

fn package_service_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/lib/systemd/user").join(SERVICE_NAME),
        PathBuf::from("/etc/systemd/user").join(SERVICE_NAME),
        PathBuf::from("/lib/systemd/user").join(SERVICE_NAME),
    ]
}

fn resolve_wayscriber_binary_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("WAYSCRIBER_BIN").map(PathBuf::from)
        && path.exists()
    {
        return Ok(path);
    }

    if let Ok(current_exe) = env::current_exe()
        && let Some(exe_dir) = current_exe.parent()
    {
        let sibling = exe_dir.join("wayscriber");
        if sibling.exists() {
            return Ok(sibling);
        }
    }

    if let Some(path) = find_in_path("wayscriber") {
        return Ok(path);
    }

    Err(
        "Unable to locate `wayscriber` binary. Set WAYSCRIBER_BIN or install `wayscriber` in PATH."
            .to_string(),
    )
}

fn render_user_service_file(binary_path: &Path) -> String {
    render_user_service_unit(binary_path)
}

pub(super) fn escape_systemd_env_value(value: &str) -> String {
    shared_escape_systemd_env_value(value)
}

#[cfg(test)]
mod tests {
    use super::render_user_service_file;
    use std::path::Path;
    use wayscriber::systemd_user_service::{
        portal_shortcut_dropin_path_from_config_root, user_service_unit_path_from_config_root,
    };

    #[test]
    fn service_paths_are_derived_from_xdg_config_root() {
        let root = Path::new("/tmp/xdg-config");
        assert_eq!(
            user_service_unit_path_from_config_root(root),
            Path::new("/tmp/xdg-config/systemd/user/wayscriber.service")
        );
        assert_eq!(
            portal_shortcut_dropin_path_from_config_root(root),
            Path::new("/tmp/xdg-config/systemd/user/wayscriber.service.d/shortcut.conf")
        );
    }

    #[test]
    fn render_user_service_file_quotes_exec_path() {
        let unit = render_user_service_file(Path::new("/tmp/My Apps/wayscriber"));
        assert!(unit.contains("ExecStart=\"/tmp/My Apps/wayscriber\" --daemon"));
    }
}
