//! A one-per-process safety copy of `config.toml` for runtime saves.
//!
//! The configurator and the migration save each leave a `.bak` beside the
//! file. Every other write — the overlay's background [`ConfigWriter`] batches
//! and the tray's session-resume toggle — used to overwrite the user's config
//! with no copy anywhere, which is what made the keybinding wipe in #293
//! unrecoverable. One snapshot per process keeps the cost at a single copy per
//! session and preserves the file as the user last authored it, rather than as
//! the previous runtime write left it.
//!
//! [`ConfigWriter`]: crate::backend::wayland
use crate::time_utils::{format_with_template, now_local};
use anyhow::{Context, Result, bail};
use log::{debug, info, warn};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

/// How many snapshots the directory keeps. Enough to walk back a few sessions
/// without turning the state directory into an archive.
const BACKUP_RETENTION: usize = 5;
const BACKUP_PREFIX: &str = "config-";
const BACKUP_SUFFIX: &str = ".toml";
/// Upper bound on the same-second names one process will try before giving up.
/// Only reached if several processes snapshot within the same second.
const MAX_NAME_ATTEMPTS: usize = 32;
/// Mode a snapshot falls back to when the source's own bits cannot be read.
/// Private, because guessing wide on a file whose permissions are unknown is
/// the one mistake a backup must not make.
const PRIVATE_BACKUP_MODE: u32 = 0o600;
/// The directory holds verbatim copies of a file the user may have locked
/// down, so it is owner-only regardless of what the source allows.
#[cfg(unix)]
const BACKUP_DIR_MODE: u32 = 0o700;

/// The snapshot this process still owes, if any.
///
/// Held by whatever owns the process's config writes — the background writer's
/// persist closure in the overlay, the tray struct in the daemon — so the
/// once-per-process guard is ordinary state a test can construct, rather than
/// a global that leaks between test cases.
pub(crate) struct RuntimeConfigBackup {
    directory: PathBuf,
    retention: usize,
    attempted: bool,
}

impl RuntimeConfigBackup {
    pub(crate) fn new() -> Self {
        Self::with_directory(crate::paths::config_backup_dir())
    }

    pub(crate) fn with_directory(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            retention: BACKUP_RETENTION,
            attempted: false,
        }
    }

    /// Copy `source` aside unless this process already did.
    ///
    /// Call it immediately before the write that mutates the file, so a batch
    /// that turns out to change nothing does not spend the process's one
    /// snapshot. Failures are logged and swallowed on purpose: the net exists
    /// to make a bad save recoverable, never to stop a good one.
    pub(crate) fn ensure_snapshot(&mut self, source: &Path) {
        // One attempt per process, successful or not: a directory that cannot
        // be written now will not start working mid-session, and retrying
        // would put a warning in the log on every save.
        if std::mem::replace(&mut self.attempted, true) {
            return;
        }
        match snapshot(source, &self.directory, self.retention) {
            Ok(Some(backup)) => info!(
                "Backed up {} to {} before the first runtime config save",
                source.display(),
                backup.display()
            ),
            Ok(None) => debug!(
                "No config at {} to back up before the first runtime save",
                source.display()
            ),
            Err(error) => warn!(
                "Failed to back up {} before the first runtime config save; saving anyway: {error:#}",
                source.display()
            ),
        }
    }
}

/// Returns the snapshot path, or `None` when there was no file to copy.
fn snapshot(source: &Path, directory: &Path, retention: usize) -> Result<Option<PathBuf>> {
    let mut source_file = match File::open(source) {
        Ok(file) => file,
        // A config the user has not written yet has no state worth keeping.
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Failed to open config at {}", source.display()));
        }
    };
    let mut contents = Vec::new();
    source_file
        .read_to_end(&mut contents)
        .with_context(|| format!("Failed to read config from {}", source.display()))?;
    let mode = source_mode(&source_file, source);
    create_backup_dir(directory).with_context(|| {
        format!(
            "Failed to create config backup directory {}",
            directory.display()
        )
    })?;
    let backup = write_unique_snapshot(directory, &contents, mode)?;
    prune(directory, retention);
    Ok(Some(backup))
}

/// The permission bits the snapshot must carry. A `0600` config copied at the
/// default `0666 & umask` would hand its contents to every local user, so the
/// copy inherits the source's bits instead of the process's umask.
#[cfg(unix)]
fn source_mode(source_file: &File, source: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    match source_file.metadata() {
        Ok(metadata) => metadata.permissions().mode() & 0o777,
        Err(error) => {
            warn!(
                "Failed to read permissions of {}; backing it up as {PRIVATE_BACKUP_MODE:o}: {error}",
                source.display()
            );
            PRIVATE_BACKUP_MODE
        }
    }
}

#[cfg(not(unix))]
fn source_mode(_source_file: &File, _source: &Path) -> u32 {
    PRIVATE_BACKUP_MODE
}

/// `mode` applies only to directories this call creates; one that already
/// exists keeps whatever the user gave it.
#[cfg(unix)]
fn create_backup_dir(directory: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(BACKUP_DIR_MODE)
        .create(directory)
}

#[cfg(not(unix))]
fn create_backup_dir(directory: &Path) -> std::io::Result<()> {
    fs::create_dir_all(directory)
}

#[cfg(unix)]
fn create_snapshot_file(path: &Path, mode: u32) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)
}

#[cfg(not(unix))]
fn create_snapshot_file(path: &Path, _mode: u32) -> std::io::Result<fs::File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

/// `OpenOptions::mode` is still filtered by the umask, which can only clear
/// bits — never add them — so the file is never wider than intended while it is
/// being written, and this restores whatever the umask stripped.
#[cfg(unix)]
fn enforce_snapshot_mode(file: &fs::File, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn enforce_snapshot_mode(_file: &fs::File, _mode: u32) -> std::io::Result<()> {
    Ok(())
}

fn write_unique_snapshot(directory: &Path, contents: &[u8], mode: u32) -> Result<PathBuf> {
    let stamp = format_with_template(now_local(), "%Y%m%d-%H%M%S");
    for name in snapshot_names(&stamp) {
        let path = directory.join(name);
        match create_snapshot_file(&path, mode) {
            Ok(mut file) => {
                file.write_all(contents).with_context(|| {
                    format!("Failed to write config backup to {}", path.display())
                })?;
                enforce_snapshot_mode(&file, mode).with_context(|| {
                    format!("Failed to restrict config backup {}", path.display())
                })?;
                return Ok(path);
            }
            // Another process claimed this name inside the same second. Take
            // the next one; overwriting would destroy its snapshot.
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to create config backup at {}", path.display())
                });
            }
        }
    }
    bail!("Config backup directory already holds every candidate name for {stamp}")
}

fn snapshot_names(stamp: &str) -> impl Iterator<Item = String> {
    let pid = std::process::id();
    let stamp = stamp.to_string();
    (0..MAX_NAME_ATTEMPTS).map(move |attempt| match attempt {
        0 => format!("{BACKUP_PREFIX}{stamp}{BACKUP_SUFFIX}"),
        1 => format!("{BACKUP_PREFIX}{stamp}-{pid}{BACKUP_SUFFIX}"),
        _ => format!("{BACKUP_PREFIX}{stamp}-{pid}-{attempt}{BACKUP_SUFFIX}"),
    })
}

/// Drop the oldest snapshots past `retention`.
///
/// Pruning is housekeeping, not part of the guarantee: a directory that cannot
/// be listed still got its copy, so problems are logged and the save continues.
fn prune(directory: &Path, retention: usize) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            warn!(
                "Failed to list config backups in {}: {error}",
                directory.display()
            );
            return;
        }
    };
    let mut names = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(BACKUP_PREFIX) && name.ends_with(BACKUP_SUFFIX))
        .collect::<Vec<_>>();
    // The stamp is fixed width, so plain string order is oldest-first.
    names.sort();
    let Some(excess) = names.len().checked_sub(retention) else {
        return;
    };
    for name in names.into_iter().take(excess) {
        let path = directory.join(name);
        if let Err(error) = fs::remove_file(&path) {
            warn!("Failed to prune config backup {}: {error}", path.display());
        }
    }
}

#[cfg(test)]
mod tests;
