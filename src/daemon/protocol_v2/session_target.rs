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
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let directory = report_dir();
    match std::fs::create_dir(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(error).with_context(|| format!("failed to create {}", directory.display()));
        }
    }
    let metadata = std::fs::symlink_metadata(&directory)?;
    // SAFETY: geteuid has no preconditions and cannot fail.
    let owner = unsafe { libc::geteuid() };
    if !metadata.is_dir() || metadata.uid() != owner {
        bail!("{} is not a private report directory", directory.display());
    }
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(test)]
mod tests;
