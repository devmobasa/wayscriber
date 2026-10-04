//! The session an overlay child is in, reported to the daemon that owns it.
//!
//! A daemon overlay writes `<generation>.target` when its session changes, so
//! the daemon can start the next overlay in that session. Reports live in
//! `daemon-commands/overlay-targets/`, beside the strict v2 tree rather than in
//! it: the v2 layout check and child-proof recovery reject entries they do not
//! know, and an older daemon reads only the plain files directly in
//! `daemon-commands/`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use super::child::{ActiveGeneration, active_generation_from_environment};

const REPORT_SCHEMA: u16 = 1;
/// Room for the longest path Linux accepts, even with every byte escaped.
const MAX_REPORT_BYTES: usize = 32 * 1024;

/// The session an overlay reports being in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReportedSession {
    /// The daemon's home session: its startup session file, or the configured
    /// default session when it has none.
    Home,
    /// Another session file.
    Named(PathBuf),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionTargetRecord {
    schema: u16,
    generation: String,
    pid: u32,
    process_start_ticks: u64,
    /// `None` is home.
    target: Option<String>,
}

fn report_dir() -> PathBuf {
    crate::paths::daemon_command_dir().join("overlay-targets")
}

fn report_path(generation: &str) -> PathBuf {
    report_dir().join(format!("{generation}.target"))
}

/// Reports `session` to the daemon that launched this overlay, replacing any
/// earlier report. Returns whether a report was written: a standalone overlay,
/// or one an older daemon launched, has no daemon that reads reports.
pub(crate) fn publish_session_from_environment(session: &ReportedSession) -> Result<bool> {
    if std::env::var_os(crate::env_vars::OVERLAY_SESSION_REPORTS_ENV)
        .is_none_or(|value| value != "1")
    {
        return Ok(false);
    }
    let generation = std::env::var(crate::env_vars::OVERLAY_CHILD_GENERATION_ENV)
        .context("a daemon overlay reports its session under its generation")?;
    super::wire::validate_id(&generation)?;
    // Only the process that published this generation's identity reports for it.
    if matches!(
        active_generation_from_environment()?,
        ActiveGeneration::Inactive
    ) {
        bail!("this overlay has not published its daemon child identity");
    }

    let target = match session {
        ReportedSession::Home => None,
        ReportedSession::Named(path) => Some(report_path_text(path)?),
    };
    let record = SessionTargetRecord {
        schema: REPORT_SCHEMA,
        generation,
        pid: std::process::id(),
        process_start_ticks: super::linux::current_process_start_ticks()?,
        target,
    };
    let bytes = super::wire::canonical_json(&record, MAX_REPORT_BYTES)?;
    create_report_dir()?;
    crate::durable_io::write_atomic(
        &report_path(&record.generation),
        &bytes,
        crate::durable_io::AtomicWriteOptions::private_runtime_file(),
    )?;

    Ok(true)
}

/// `path` as the absolute UTF-8 text a report carries.
fn report_path_text(path: &Path) -> Result<String> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to resolve a relative session path")?
            .join(path)
    };
    path.into_os_string().into_string().map_err(|path| {
        anyhow!(
            "session path {} is not UTF-8",
            PathBuf::from(path).display()
        )
    })
}

fn create_report_dir() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let directory = report_dir();
    match std::fs::create_dir(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(error).with_context(|| format!("failed to create {}", directory.display()));
        }
    }
    let metadata = std::fs::symlink_metadata(&directory)?;
    if !metadata.is_dir() || !owned_by_this_user(&metadata) {
        bail!("{} is not a private report directory", directory.display());
    }
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}

/// The session the child `generation` last reported, if it reported one under
/// exactly this identity. The identity is the one the daemon owns, captured
/// while the child ran: an exited child no longer has a `/proc` entry to ask.
pub(crate) fn read_session_report(
    generation: &str,
    pid: u32,
    process_start_ticks: u64,
) -> Result<Option<ReportedSession>> {
    super::wire::validate_id(generation)?;
    match std::fs::symlink_metadata(report_dir()) {
        Ok(metadata) if metadata.is_dir() && is_private(&metadata) => {}
        Ok(_) => bail!("the session report directory is not private"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("failed to inspect the session reports"),
    }
    let bytes = match read_private_file(&report_path(generation)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("failed to read the session report"),
    };
    let record: SessionTargetRecord = super::wire::parse_canonical_json(&bytes, MAX_REPORT_BYTES)?;
    if record.schema != REPORT_SCHEMA
        || record.generation != generation
        || record.pid != pid
        || record.process_start_ticks != process_start_ticks
    {
        bail!("the session report belongs to another overlay child");
    }

    match record.target.map(PathBuf::from) {
        None => Ok(Some(ReportedSession::Home)),
        Some(path) if path.is_absolute() => Ok(Some(ReportedSession::Named(path))),
        Some(path) => bail!("reported session {} is not absolute", path.display()),
    }
}

/// Reads the final report of the exited child `generation`, then removes it
/// along with any temporary its writer left. A report that cannot be trusted
/// is logged and ignored, so it never stands in the way of retiring the child.
pub(crate) fn take_final_session_report(
    generation: &str,
    pid: u32,
    process_start_ticks: u64,
) -> Option<ReportedSession> {
    let session =
        read_session_report(generation, pid, process_start_ticks).unwrap_or_else(|error| {
            log::warn!("Ignoring the session report of overlay child {generation}: {error:#}");
            None
        });
    discard_session_report(generation);
    session
}

/// Removes the report of child `generation` and any temporary its writer left.
pub(crate) fn discard_session_report(generation: &str) {
    let report = format!("{generation}.target");
    // The report itself goes by name, however full the directory is.
    let removed = match std::fs::remove_file(report_dir().join(&report)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(anyhow::Error::new(error).context("failed to remove the report"))
        }
        _ => remove_reports(|target| target == report),
    };
    if let Err(error) = removed {
        log::warn!("Failed to remove the session report of overlay child {generation}: {error:#}");
    }
}

/// Removes every report an earlier daemon left behind, restoring nothing from
/// them: the session a daemon remembered ends with that daemon.
pub(crate) fn clear_stale_session_reports() -> Result<()> {
    remove_reports(|target| {
        target
            .strip_suffix(".target")
            .is_some_and(|generation| super::wire::validate_id(generation).is_ok())
    })
}

/// Bounds how many entries one cleanup looks at, so a flooded directory costs
/// a fixed amount of work.
const MAX_CLEANUP_ENTRIES: usize = 256;

/// Removes each report, and each temporary left by a report's writer, whose
/// report name satisfies `is_removed`. Anything else in the directory is not
/// this daemon's to remove.
fn remove_reports(is_removed: impl Fn(&str) -> bool) -> Result<()> {
    let directory = report_dir();
    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to list {}", directory.display()));
        }
    };
    for entry in entries.take(MAX_CLEANUP_ENTRIES) {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let report = crate::durable_io::temp_file_target(&name).unwrap_or(&name);
        if !is_removed(report) {
            continue;
        }
        match std::fs::remove_file(entry.path()) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to remove {}", entry.path().display()));
            }
        }
    }
    Ok(())
}

/// Reads a regular file this user owns and only this user can read or write,
/// without following a symlink.
fn read_private_file(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || !is_private(&metadata) || metadata.len() > MAX_REPORT_BYTES as u64 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "session report is not a private regular file within its size bound",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_REPORT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_REPORT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "session report exceeds its size bound",
        ));
    }
    Ok(bytes)
}

fn is_private(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    owned_by_this_user(metadata) && metadata.mode() & 0o077 == 0
}

fn owned_by_this_user(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    // SAFETY: geteuid has no preconditions and cannot fail.
    metadata.uid() == unsafe { libc::geteuid() }
}

#[cfg(test)]
mod tests;
