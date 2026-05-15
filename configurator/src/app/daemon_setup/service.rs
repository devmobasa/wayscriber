use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::models::DesktopEnvironment;
use wayscriber::runtime_capabilities::{
    RUNTIME_CAPABILITIES_FLAG, RuntimeCapabilities, parse_runtime_capabilities,
};
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

pub(super) fn detect_managed_daemon_portal_runtime_supported() -> bool {
    managed_daemon_runtime_capabilities()
        .map(|capabilities| capabilities.portal)
        .unwrap_or(false)
}

fn managed_daemon_runtime_capabilities() -> Result<RuntimeCapabilities, String> {
    let binary_path = explicit_wayscriber_binary_path()
        .or_else(|| detect_installed_service_binary_path(command_available("systemctl")))
        .map_or_else(resolve_wayscriber_binary_path, Ok)?;
    query_wayscriber_runtime_capabilities(&binary_path)
}

fn detect_installed_service_binary_path(systemctl_available: bool) -> Option<PathBuf> {
    if systemctl_available
        && let Ok(capture) = run_command(
            "systemctl",
            &[
                "--user",
                "show",
                "--property=ExecStart",
                "--value",
                SERVICE_NAME,
            ],
        )
        && capture.success
        && let Some(path) = parse_systemctl_exec_start_path(&capture.stdout)
    {
        return Some(path);
    }

    detect_service_unit_path(systemctl_available)
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|content| parse_service_exec_start_path(&content))
}

fn parse_systemctl_exec_start_path(output: &str) -> Option<PathBuf> {
    let output = output
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim();
    output
        .split(" ; ")
        .map(str::trim)
        .find_map(|field| field.strip_prefix("path="))
        .and_then(path_from_exec_value)
}

fn parse_service_exec_start_path(content: &str) -> Option<PathBuf> {
    content.lines().find_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let value = line.strip_prefix("ExecStart=")?;
        parse_exec_command_path(value)
    })
}

fn parse_exec_command_path(value: &str) -> Option<PathBuf> {
    let value = value
        .trim_start()
        .trim_start_matches(['-', '@', ':', '+', '!'])
        .trim_start();
    let token = first_exec_token(value)?;
    path_from_exec_value(&token)
}

fn first_exec_token(value: &str) -> Option<String> {
    let first = value.chars().next()?;
    if first != '"' && first != '\'' {
        return value.split_whitespace().next().map(ToString::to_string);
    }

    let quote = first;
    let mut token = String::new();
    let mut escaped = false;
    for ch in value[first.len_utf8()..].chars() {
        if escaped {
            token.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == quote {
            return Some(token);
        } else {
            token.push(ch);
        }
    }
    if escaped {
        token.push('\\');
    }
    Some(token)
}

fn path_from_exec_value(value: &str) -> Option<PathBuf> {
    let value = value.trim();
    if value.is_empty() || value == "-" || value == "(null)" {
        None
    } else {
        Some(PathBuf::from(value))
    }
}

fn query_wayscriber_runtime_capabilities(
    binary_path: &Path,
) -> Result<RuntimeCapabilities, String> {
    let output = Command::new(binary_path)
        .arg(RUNTIME_CAPABILITIES_FLAG)
        .output()
        .map_err(|err| {
            format!(
                "Failed to execute `{}` with args [{}]: {}",
                binary_path.display(),
                RUNTIME_CAPABILITIES_FLAG,
                err
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "`{}` failed with args [{}]\nstdout: {}\nstderr: {}",
            binary_path.display(),
            RUNTIME_CAPABILITIES_FLAG,
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    parse_runtime_capabilities(&String::from_utf8_lossy(&output.stdout))
}

pub(super) fn remove_portal_shortcut_dropin_if_gnome(
    desktop: DesktopEnvironment,
) -> Result<bool, String> {
    if desktop != DesktopEnvironment::Gnome {
        return Ok(false);
    }
    remove_portal_shortcut_dropin()
}

pub(super) fn remove_portal_shortcut_dropin() -> Result<bool, String> {
    let Some(path) = portal_shortcut_dropin_path() else {
        return Ok(false);
    };
    if !path.exists() {
        return Ok(false);
    }
    fs::remove_file(&path).map_err(|err| {
        format!(
            "Failed to remove portal shortcut drop-in {}: {}",
            path.display(),
            err
        )
    })?;
    Ok(true)
}

fn package_service_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/lib/systemd/user").join(SERVICE_NAME),
        PathBuf::from("/etc/systemd/user").join(SERVICE_NAME),
        PathBuf::from("/lib/systemd/user").join(SERVICE_NAME),
    ]
}

pub(super) fn resolve_wayscriber_binary_path() -> Result<PathBuf, String> {
    if let Some(path) = explicit_wayscriber_binary_path() {
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

fn explicit_wayscriber_binary_path() -> Option<PathBuf> {
    env::var_os("WAYSCRIBER_BIN")
        .map(PathBuf::from)
        .filter(|path| path.exists())
}

fn render_user_service_file(binary_path: &Path) -> String {
    render_user_service_unit(binary_path)
}

pub(super) fn escape_systemd_env_value(value: &str) -> String {
    shared_escape_systemd_env_value(value)
}

#[cfg(test)]
mod tests {
    use super::{
        parse_service_exec_start_path, parse_systemctl_exec_start_path, render_user_service_file,
    };
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

    #[test]
    fn parse_systemctl_exec_start_path_reads_path_field() {
        let output =
            "{ path=/usr/bin/wayscriber ; argv[]=/usr/bin/wayscriber --daemon ; ignore_errors=no }";
        assert_eq!(
            parse_systemctl_exec_start_path(output).as_deref(),
            Some(Path::new("/usr/bin/wayscriber"))
        );
    }

    #[test]
    fn parse_service_exec_start_path_reads_quoted_exec_start() {
        let unit = "[Service]\nExecStartPre=/bin/sh -c 'true'\nExecStart=\"/tmp/My Apps/wayscriber\" --daemon\n";
        assert_eq!(
            parse_service_exec_start_path(unit).as_deref(),
            Some(Path::new("/tmp/My Apps/wayscriber"))
        );
    }

    #[test]
    fn parse_service_exec_start_path_reads_unquoted_exec_start() {
        let unit = "[Service]\nExecStart=/usr/bin/wayscriber --daemon\n";
        assert_eq!(
            parse_service_exec_start_path(unit).as_deref(),
            Some(Path::new("/usr/bin/wayscriber"))
        );
    }
}
