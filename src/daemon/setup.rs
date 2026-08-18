use crate::durable_io::{AtomicWriteOptions, OverwriteMode, PermissionPolicy, SymlinkPolicy};
use anyhow::{Context, Result, bail};
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::systemd_user_service::{
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

    crate::durable_io::write_text_atomic(
        path,
        content,
        AtomicWriteOptions {
            overwrite: OverwriteMode::Replace,
            permissions: PermissionPolicy::PreserveExistingOrMode(0o644),
            symlink: SymlinkPolicy::FollowExistingTarget,
            sync_file: true,
            sync_parent: true,
        },
    )
    .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn run_systemctl_user(args: &[&str]) -> Result<()> {
    let arguments = systemctl_user_arguments(args);
    let output = crate::process_broker::current()
        .and_then(|broker| {
            broker.run(
                crate::process_broker::HelperKind::Systemctl,
                OsStr::new("systemctl"),
                &arguments,
                Vec::new(),
                Duration::from_secs(30),
                256 * 1024,
            )
        })
        .with_context(|| format!("failed to execute systemctl --user {}", args.join(" ")))?;

    if output.timed_out {
        bail!("systemctl --user {} timed out", args.join(" "));
    }
    if output.status == 0 {
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

fn systemctl_user_arguments<'a>(args: &'a [&'a str]) -> Vec<&'a OsStr> {
    std::iter::once(OsStr::new("--user"))
        .chain(args.iter().map(|argument| OsStr::new(argument)))
        .collect()
}

fn systemctl_error_detail(stdout: &str, stderr: &str) -> String {
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => "no output from systemctl".to_string(),
        (true, false) => stderr.to_string(),
        (false, true) => stdout.to_string(),
        (false, false) => format!("{stderr} | {stdout}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_setup_systemctl_argv_is_explicit_and_user_scoped() {
        assert_eq!(
            systemctl_user_arguments(&["daemon-reload"]),
            [OsStr::new("--user"), OsStr::new("daemon-reload")]
        );
        assert_eq!(
            systemctl_user_arguments(&["enable", "--now", USER_SERVICE_NAME]),
            [
                OsStr::new("--user"),
                OsStr::new("enable"),
                OsStr::new("--now"),
                OsStr::new("wayscriber.service"),
            ]
        );
    }

    #[test]
    fn systemctl_error_output_prefers_stderr_without_losing_stdout() {
        assert_eq!(
            systemctl_error_detail("stdout detail", "stderr detail"),
            "stderr detail | stdout detail"
        );
        assert_eq!(systemctl_error_detail("", ""), "no output from systemctl");
    }
}
