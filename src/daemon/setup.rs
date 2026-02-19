use anyhow::{Context, Result, bail};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::paths::config_dir;

const SERVICE_UNIT_NAME: &str = "wayscriber.service";

#[derive(Debug, Clone)]
pub(crate) struct BackgroundModeSetupSummary {
    pub(crate) service_path: PathBuf,
}

pub(crate) fn setup_background_mode() -> Result<BackgroundModeSetupSummary> {
    let service_path = ensure_user_service_file()?;
    run_systemctl_user(&["daemon-reload"])?;
    run_systemctl_user(&["enable", "--now", SERVICE_UNIT_NAME])?;
    Ok(BackgroundModeSetupSummary { service_path })
}

fn ensure_user_service_file() -> Result<PathBuf> {
    let service_path = user_service_path()?;
    if let Some(parent) = service_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create systemd user directory {}",
                parent.display()
            )
        })?;
    }

    let executable = std::env::current_exe().context("failed to resolve wayscriber executable")?;
    let service_contents = render_user_service(&executable);
    write_if_changed(&service_path, &service_contents)?;
    Ok(service_path)
}

fn user_service_path() -> Result<PathBuf> {
    let config_root = config_dir().context("unable to resolve XDG config directory")?;
    Ok(config_root
        .join("systemd")
        .join("user")
        .join(SERVICE_UNIT_NAME))
}

fn write_if_changed(path: &Path, content: &str) -> Result<()> {
    match fs::read_to_string(path) {
        Ok(existing) if existing == content => return Ok(()),
        Ok(_) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", path.display()));
        }
    }

    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn run_systemctl_user(args: &[&str]) -> Result<()> {
    let output = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .with_context(|| format!("failed to execute systemctl --user {}", args.join(" ")))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = systemctl_error_detail(&stdout, &stderr);

    bail!(
        "systemctl --user {} failed (status {}): {}",
        args.join(" "),
        output.status,
        detail
    );
}

fn systemctl_error_detail(stdout: &str, stderr: &str) -> String {
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => "no output from systemctl".to_string(),
        (true, false) => stderr.to_string(),
        (false, true) => stdout.to_string(),
        (false, false) => format!("{stderr} | {stdout}"),
    }
}

fn render_user_service(executable: &Path) -> String {
    let quoted_exec = quote_systemd_exec(executable);
    let binary_dir = executable
        .parent()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "/usr/bin".to_string());
    let escaped_path_env =
        escape_systemd_env_value(&format!("{binary_dir}:/usr/local/bin:/usr/bin:/bin"));
    format!(
        "[Unit]
Description=Wayscriber - Screen annotation tool for Wayland
Documentation=https://wayscriber.com
PartOf=graphical-session.target
After=graphical-session.target

[Service]
Type=simple
ExecStartPre=/bin/sh -c '[ -n \"$WAYLAND_DISPLAY\" ] && [ -S \"$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY\" ]'
ExecStart={quoted_exec} --daemon
Restart=on-failure
RestartSec=5
RestartPreventExitStatus=75
SuccessExitStatus=75
Environment=\"PATH={escaped_path_env}\"

[Install]
WantedBy=graphical-session.target
"
    )
}

fn quote_systemd_exec(path: &Path) -> String {
    let escaped = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn escape_systemd_env_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::{quote_systemd_exec, render_user_service};
    use std::path::Path;

    #[test]
    fn systemd_exec_path_is_quoted_for_whitespace() {
        let quoted = quote_systemd_exec(Path::new("/tmp/My App/wayscriber"));
        assert_eq!(quoted, "\"/tmp/My App/wayscriber\"");
    }

    #[test]
    fn render_user_service_uses_daemon_exec_start() {
        let service = render_user_service(Path::new("/usr/bin/wayscriber"));
        assert!(service.contains("ExecStart=\"/usr/bin/wayscriber\" --daemon"));
        assert!(service.contains("Documentation=https://wayscriber.com"));
        assert!(service.contains("Environment=\"PATH=/usr/bin:/usr/local/bin:/usr/bin:/bin\""));
        assert!(service.contains("WantedBy=graphical-session.target"));
    }

    #[test]
    fn render_user_service_quotes_execstart_path_with_spaces() {
        let service = render_user_service(Path::new("/tmp/My App/wayscriber"));
        assert!(service.contains("ExecStart=\"/tmp/My App/wayscriber\" --daemon"));
    }
}
