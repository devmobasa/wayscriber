use anyhow::{Context, Result, bail};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use wayscriber::systemd_user_service::{
    USER_SERVICE_NAME, render_user_service_unit, user_service_unit_path,
};

#[derive(Debug, Clone)]
pub(crate) struct BackgroundModeSetupSummary {
    pub(crate) service_path: PathBuf,
}

pub(crate) fn setup_background_mode() -> Result<BackgroundModeSetupSummary> {
    let service_path = ensure_user_service_file()?;
    run_systemctl_user(&["daemon-reload"])?;
    run_systemctl_user(&["enable", "--now", USER_SERVICE_NAME])?;
    Ok(BackgroundModeSetupSummary { service_path })
}

fn ensure_user_service_file() -> Result<PathBuf> {
    let service_path =
        user_service_unit_path().context("unable to resolve XDG config directory")?;
    if let Some(parent) = service_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create systemd user directory {}",
                parent.display()
            )
        })?;
    }

    let executable = std::env::current_exe().context("failed to resolve wayscriber executable")?;
    let service_contents = render_user_service_unit(&executable);
    write_if_changed(&service_path, &service_contents)?;
    Ok(service_path)
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
