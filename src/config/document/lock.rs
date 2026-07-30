//! Cross-process exclusion for the config file's write window.
//!
//! A save is two steps that have to look like one. `ensure_source_unchanged`
//! compares the bytes the document loaded against the bytes on disk, and the
//! write that follows replaces the file by rename. Without exclusion the
//! configurator's Save and an overlay editor — separate processes — can both
//! pass the comparison before either renames, and the second rename then throws
//! the first edit away: both writers report success, and both `.bak` copies hold
//! the same pre-edit source, so the lost edit is not even recoverable.
//!
//! One advisory lock closes that window. It is taken around the whole
//! check-copy-rename sequence, so the writer that loses the race sees the file
//! change and reports it — which is exactly the situation the editors'
//! reload-and-reapply retry already recovers from.
//!
//! The lock is a sibling file (`config.toml.lock`) rather than the config file
//! itself, because an atomic replace gives the config a new inode: a lock held
//! on the old one would be invisible to the next writer, which opens the new
//! one. It is created on the first write and left in place; it carries no
//! contents and is never read.

use anyhow::{Context, Result, anyhow};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long a save waits for another writer's window to close.
///
/// The guarded window is a byte comparison, a file copy, and a rename —
/// milliseconds on any working filesystem. The bound is wide enough that a busy
/// disk never turns a save into a refusal, and short enough that a writer that
/// died holding the lock cannot wedge the caller forever.
pub(super) const CONFIG_WRITE_LOCK_TIMEOUT: Duration = Duration::from_secs(5);

/// How long to wait between attempts while another writer holds the lock.
///
/// Polling rather than a blocking `flock`, so the wait has a deadline the caller
/// can report instead of hanging on a writer that will never release.
const LOCK_RETRY: Duration = Duration::from_millis(2);

/// Another writer held the config file's write lock for longer than the save was
/// willing to wait.
///
/// Reported as itself rather than as a save failure because the file is intact
/// and the cause is another editor, not the configuration: retrying the gesture
/// once the other window closes is all it takes.
#[derive(Debug)]
pub struct ConfigWriteLockTimeout {
    pub path: PathBuf,
    pub waited: Duration,
}

impl std::fmt::Display for ConfigWriteLockTimeout {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "another editor is writing {} (waited {:?} for the config write lock)",
            self.path.display(),
            self.waited
        )
    }
}

impl std::error::Error for ConfigWriteLockTimeout {}

/// The lock file guarding one config file's writes.
///
/// Derived from the write's real destination — the end of the symlink chain, not
/// the path the caller happened to name — so two processes reaching the same
/// file through different links still contend for the same lock.
pub(super) fn config_write_lock_path(destination: &Path) -> Result<PathBuf> {
    let name = destination.file_name().ok_or_else(|| {
        anyhow!(
            "Config destination {} has no file name to lock",
            destination.display()
        )
    })?;
    let mut lock_name = name.to_os_string();
    lock_name.push(".lock");
    Ok(destination.with_file_name(lock_name))
}

/// An acquired write lock, released when it is dropped.
#[derive(Debug)]
pub(super) struct ConfigWriteLock {
    /// `None` only where the platform has no advisory locking, in which case
    /// nothing was acquired and there is nothing to release.
    file: Option<File>,
}

impl Drop for ConfigWriteLock {
    fn drop(&mut self) {
        if let Some(file) = self.file.as_ref() {
            // Closing the descriptor would release it anyway; unlocking first
            // says so at the point where the window ends. A failure here leaves
            // the close to do it, so there is nothing for the caller to act on.
            let _ = crate::session::unlock(file);
        }
    }
}

/// Takes the write lock for `destination`, waiting up to `timeout`.
pub(super) fn acquire_config_write_lock(
    destination: &Path,
    timeout: Duration,
) -> Result<ConfigWriteLock> {
    let path = config_write_lock_path(destination)?;
    acquire_at(&path, timeout)
}

#[cfg(unix)]
pub(super) fn acquire_at(path: &Path, timeout: Duration) -> Result<ConfigWriteLock> {
    use std::io::ErrorKind;

    let file = open_lock_file(path)?;
    let started = Instant::now();
    loop {
        match crate::session::try_lock_exclusive(&file) {
            Ok(()) => return Ok(ConfigWriteLock { file: Some(file) }),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                let waited = started.elapsed();
                if waited >= timeout {
                    return Err(anyhow!(ConfigWriteLockTimeout {
                        path: path.to_path_buf(),
                        waited,
                    }));
                }
                std::thread::sleep(LOCK_RETRY.min(timeout - waited));
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to lock the config file through {}", path.display())
                });
            }
        }
    }
}

/// Best effort where the tree has no advisory locking to mine.
///
/// The same shape the session lock helpers use for this case: the operation
/// proceeds rather than failing, because refusing every config write on such a
/// platform would be a worse outcome than the race the lock exists to close.
#[cfg(not(unix))]
pub(super) fn acquire_at(_path: &Path, _timeout: Duration) -> Result<ConfigWriteLock> {
    Ok(ConfigWriteLock { file: None })
}

#[cfg(unix)]
fn open_lock_file(path: &Path) -> Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    // The file is a rendezvous, never a payload: it must not follow a symlink
    // into something else, and it is no more readable than the config it guards.
    options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    options
        .open(path)
        .with_context(|| format!("Failed to open the config write lock at {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lock somebody else is holding is waited for, then reported — never
    /// silently skipped, which would put the caller back inside the race.
    #[test]
    #[cfg(unix)]
    fn a_held_lock_is_reported_as_a_timeout_rather_than_taken() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let destination = temp.path().join("config.toml");
        let path = config_write_lock_path(&destination).expect("a lock path");

        let held = acquire_at(&path, Duration::from_millis(50)).expect("the first holder");

        let error = acquire_at(&path, Duration::from_millis(50))
            .expect_err("a second holder must not get in");

        let timeout = error
            .downcast_ref::<ConfigWriteLockTimeout>()
            .unwrap_or_else(|| panic!("the caller must be able to name this: {error:#}"));
        assert_eq!(timeout.path, path);
        assert!(
            timeout.waited >= Duration::from_millis(50),
            "the wait must be the one that was asked for, not an instant refusal"
        );

        // And releasing it lets the next writer straight in.
        drop(held);
        acquire_at(&path, Duration::from_millis(50)).expect("the released lock is free");
    }

    /// The lock is named after the file it guards, in that file's directory, so
    /// two processes editing the same config contend rather than each taking a
    /// lock of their own.
    #[test]
    fn the_lock_sits_beside_the_file_it_guards() {
        let path =
            config_write_lock_path(Path::new("/home/someone/.config/wayscriber/config.toml"))
                .expect("a lock path");
        assert_eq!(
            path,
            Path::new("/home/someone/.config/wayscriber/config.toml.lock")
        );
    }
}
