use super::{
    DaemonToggleCommand, DaemonToggleCommands, DaemonToggleEnvelope, DaemonToggleRequest,
    MAX_DAEMON_TOGGLE_REQUEST_AGE, current_unix_millis,
};
use crate::durable_io::AtomicWriteOptions;
use crate::paths::{daemon_command_dir, daemon_command_file};
use anyhow::{Context, Result, anyhow};
use log::warn;
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) fn clear_file(path: &std::path::Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("failed to remove {}", path.display())),
    }
}

fn is_legacy_request_name(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    if extension != "json" || stem.len() != 41 {
        return false;
    }
    let bytes = stem.as_bytes();
    bytes[32] == b'-'
        && bytes[..32]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        && bytes[33..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn remove_legacy_entries(dir: &Path) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", dir.display()));
        }
    };

    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read entry in {}", dir.display()))?;
        if !is_legacy_request_name(&entry.file_name()) {
            continue;
        }
        let file_type = entry.file_type().with_context(|| {
            format!(
                "failed to inspect legacy daemon entry {}",
                entry.path().display()
            )
        })?;
        if file_type.is_dir() {
            return Err(anyhow!(
                "legacy daemon entry {} is unexpectedly a directory",
                entry.path().display()
            ));
        }
        clear_file(&entry.path())?;
    }
    Ok(())
}

fn clear_legacy_command_files() -> Result<()> {
    let dir = daemon_command_dir();
    remove_legacy_entries(&dir)?;

    let responses = dir.join("responses");
    match fs::symlink_metadata(&responses) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            remove_legacy_entries(&responses)?;
            match fs::remove_dir(&responses) {
                Ok(()) => {}
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) if err.kind() == ErrorKind::DirectoryNotEmpty => {}
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!("failed to remove empty {}", responses.display())
                    });
                }
            }
        }
        Ok(_) => {
            return Err(anyhow!(
                "daemon response path {} is not a regular directory",
                responses.display()
            ));
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", responses.display()));
        }
    }

    match fs::remove_dir(&dir) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) if err.kind() == ErrorKind::DirectoryNotEmpty => Ok(()),
        Err(err) => Err(err).with_context(|| format!("failed to remove empty {}", dir.display())),
    }
}

fn next_daemon_toggle_request_path() -> Result<std::path::PathBuf> {
    let dir = daemon_command_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create runtime directory {}", dir.display()))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Ok(dir.join(format!("{:032x}-{:08x}.json", stamp, std::process::id())))
}

fn daemon_toggle_response_path_for_request(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("{} has no parent directory", path.display()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow!("{} has no file name", path.display()))?;
    Ok(parent.join("responses").join(file_name))
}

pub(super) fn write_file_atomic(path: &std::path::Path, payload: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create runtime directory {}", parent.display()))?;
    }
    crate::durable_io::write_atomic(path, payload, AtomicWriteOptions::private_runtime_file())
        .with_context(|| format!("failed to write {}", path.display()))
}

pub(crate) fn clear_daemon_toggle_request_file() -> Result<()> {
    clear_file(&daemon_command_file())?;
    clear_legacy_command_files()
}

pub(super) fn write_daemon_toggle_request(
    request: &DaemonToggleRequest,
    daemon_token: &str,
) -> Result<DaemonToggleCommand> {
    let envelope = DaemonToggleEnvelope {
        daemon_token: daemon_token.to_string(),
        requested_at_unix_ms: current_unix_millis()?,
        canceled: false,
        request: request.clone(),
    };
    let payload =
        serde_json::to_vec(&envelope).context("failed to serialize daemon toggle request")?;
    let path = next_daemon_toggle_request_path()?;
    write_file_atomic(&path, &payload)?;
    let response_path = daemon_toggle_response_path_for_request(&path)?;
    Ok(DaemonToggleCommand {
        daemon_token: daemon_token.to_string(),
        request: request.clone(),
        request_path: path,
        response_path,
    })
}

pub(crate) fn take_daemon_toggle_requests(expected_token: &str) -> Result<DaemonToggleCommands> {
    let dir = daemon_command_dir();
    let mut paths = match fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.is_file())
            .collect::<Vec<_>>(),
        Err(err) if err.kind() == ErrorKind::NotFound => Vec::new(),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", dir.display()));
        }
    };
    paths.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    let saw_command_files = !paths.is_empty();

    let mut requests = Vec::new();
    for path in paths {
        let payload = match fs::read(&path) {
            Ok(payload) => payload,
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
            Err(err) => {
                warn!("Failed to read {}: {}", path.display(), err);
                continue;
            }
        };

        if let Err(err) = clear_file(&path) {
            warn!("Failed to remove {}: {}", path.display(), err);
            continue;
        }

        let envelope: DaemonToggleEnvelope = match serde_json::from_slice(&payload) {
            Ok(envelope) => envelope,
            Err(err) => {
                warn!(
                    "Ignoring malformed daemon toggle request {}: {}",
                    path.display(),
                    err
                );
                continue;
            }
        };

        if envelope.daemon_token != expected_token {
            warn!("Ignoring daemon toggle request for a different daemon instance");
            continue;
        }

        if envelope.canceled {
            warn!("Ignoring canceled daemon toggle request");
            continue;
        }

        let age_ms = current_unix_millis()?.saturating_sub(envelope.requested_at_unix_ms);
        if Duration::from_millis(age_ms) > MAX_DAEMON_TOGGLE_REQUEST_AGE {
            warn!("Ignoring stale daemon toggle request older than 5s");
            continue;
        }

        requests.push(DaemonToggleCommand {
            daemon_token: envelope.daemon_token,
            request: envelope.request,
            response_path: daemon_toggle_response_path_for_request(&path)?,
            request_path: path,
        });
    }

    Ok(DaemonToggleCommands {
        commands: requests,
        saw_command_files,
    })
}
