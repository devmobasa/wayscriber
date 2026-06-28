use super::queue::{clear_file, write_file_atomic};
use super::{
    DAEMON_TOGGLE_RESPONSE_POLL, DaemonToggleCommand, DaemonToggleEnvelope, DaemonToggleRequest,
    DaemonToggleResponse, MAX_DAEMON_TOGGLE_RESPONSE_WAIT, current_unix_millis,
};
use anyhow::{Context, Result, anyhow, bail};
use log::warn;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

pub(crate) fn write_daemon_toggle_command_success(command: &DaemonToggleCommand) -> Result<()> {
    write_daemon_toggle_response(&command.response_path, DaemonToggleResponse::ok())
}

pub(crate) fn write_daemon_toggle_command_error(
    command: &DaemonToggleCommand,
    message: &str,
) -> Result<()> {
    write_daemon_toggle_response(
        &command.response_path,
        DaemonToggleResponse::error(message.to_string()),
    )
}

fn write_daemon_toggle_response(path: &Path, response: DaemonToggleResponse) -> Result<()> {
    let payload =
        serde_json::to_vec(&response).context("failed to serialize daemon toggle response")?;
    write_file_atomic(path, &payload)
}

#[cfg(test)]
pub(crate) fn read_daemon_toggle_response(path: &Path) -> Result<()> {
    let payload = fs::read(path)
        .with_context(|| format!("failed to read daemon response {}", path.display()))?;
    let response: DaemonToggleResponse =
        serde_json::from_slice(&payload).context("failed to parse daemon toggle response")?;
    response.into_result()
}

pub(super) fn wait_daemon_toggle_response_for(path: &Path, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match fs::read(path) {
            Ok(payload) => {
                let response: DaemonToggleResponse = serde_json::from_slice(&payload)
                    .context("failed to parse daemon toggle response")?;
                if let Err(err) = clear_file(path) {
                    warn!(
                        "Failed to remove daemon response {}: {}",
                        path.display(),
                        err
                    );
                }
                return response.into_result();
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                if Instant::now() >= deadline {
                    return Err(anyhow!(
                        "timed out waiting for wayscriber daemon to process toggle request"
                    ));
                }
                thread::sleep(DAEMON_TOGGLE_RESPONSE_POLL);
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to read daemon response {}", path.display()));
            }
        }
    }
}

pub(super) fn wait_daemon_toggle_command_response(command: &DaemonToggleCommand) -> Result<()> {
    wait_daemon_toggle_command_response_for(command, MAX_DAEMON_TOGGLE_RESPONSE_WAIT)
}

pub(super) fn wait_daemon_toggle_command_response_for(
    command: &DaemonToggleCommand,
    timeout: Duration,
) -> Result<()> {
    if let Err(err) = wait_daemon_toggle_response_for(&command.response_path, timeout) {
        cancel_daemon_toggle_command(command);
        return Err(err);
    }
    Ok(())
}

fn cancel_daemon_toggle_command(command: &DaemonToggleCommand) {
    let payload = match canceled_daemon_toggle_payload(command) {
        Ok(payload) => payload,
        Err(err) => {
            warn!("Failed to serialize daemon cancellation command: {}", err);
            clear_daemon_toggle_command_files(command);
            return;
        }
    };

    match write_existing_daemon_request_file(&command.request_path, &payload) {
        Ok(true) => {}
        Ok(false) => return,
        Err(err) => {
            warn!(
                "Failed to mark daemon request {} canceled after failed toggle wait: {}",
                command.request_path.display(),
                err
            );
        }
    }

    if let Err(err) = clear_file(&command.response_path) {
        warn!(
            "Failed to remove daemon response {} after failed toggle wait: {}",
            command.response_path.display(),
            err
        );
    }
}

fn write_existing_daemon_request_file(path: &Path, payload: &[u8]) -> Result<bool> {
    let expected_metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(err).with_context(|| {
                format!(
                    "failed to inspect daemon request {} before cancellation",
                    path.display()
                )
            });
        }
    };

    if expected_metadata.file_type().is_symlink() {
        bail!("daemon request {} is a symlink", path.display());
    }

    if !expected_metadata.is_file() {
        bail!("daemon request {} is not a regular file", path.display());
    }

    let Some(mut file) = open_existing_daemon_request_file(path)? else {
        return Ok(false);
    };

    let opened_metadata = file.metadata().with_context(|| {
        format!(
            "failed to inspect opened daemon request {} before cancellation",
            path.display()
        )
    })?;

    if !same_file_identity(&expected_metadata, &opened_metadata) {
        return Ok(false);
    }

    file.set_len(0)
        .with_context(|| format!("failed to truncate daemon request {}", path.display()))?;
    file.write_all(payload)
        .with_context(|| format!("failed to write daemon request {}", path.display()))?;
    Ok(true)
}

fn open_existing_daemon_request_file(path: &Path) -> Result<Option<File>> {
    let mut options = OpenOptions::new();
    options.write(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);

    match options.open(path) {
        Ok(file) => Ok(Some(file)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| {
            format!(
                "failed to open existing daemon request {} before cancellation",
                path.display()
            )
        }),
    }
}

#[cfg(unix)]
fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file_identity(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    true
}

fn canceled_daemon_toggle_payload(command: &DaemonToggleCommand) -> Result<Vec<u8>> {
    let envelope = DaemonToggleEnvelope {
        daemon_token: command.daemon_token.clone(),
        requested_at_unix_ms: current_unix_millis()?,
        canceled: true,
        request: DaemonToggleRequest::default(),
    };
    serde_json::to_vec(&envelope).context("failed to serialize canceled daemon toggle request")
}

pub(super) fn clear_daemon_toggle_command_files(command: &DaemonToggleCommand) {
    if let Err(err) = clear_file(&command.request_path) {
        warn!(
            "Failed to remove daemon request {} after failed toggle wait: {}",
            command.request_path.display(),
            err
        );
    }
    if let Err(err) = clear_file(&command.response_path) {
        warn!(
            "Failed to remove daemon response {} after failed toggle wait: {}",
            command.response_path.display(),
            err
        );
    }
}
