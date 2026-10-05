use super::*;
use crate::durable_io::{
    AtomicWriteOptions, DestinationExpectation, DurableIoError, FileIdentity, OverwriteMode,
    PermissionPolicy, SymlinkPolicy, sync_parent_dir, write_atomic,
    write_atomic_reporting_identity,
};
use std::io::ErrorKind;

/// Keeps unreadable bytes away from rotation backups. Called under the session lock.
pub(super) fn quarantine_corrupt_artifact(
    artifact: &Path,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<PathBuf> {
    let copies = crate::session::artifacts::corrupt_copies_of(artifact)?;
    let named_bytes = if is_named_primary_path(artifact, options) {
        read_artifact_bounded(artifact, max_expanded_size.max(options.max_file_size_bytes)).ok()
    } else {
        None
    };
    if let Some(bytes) = &named_bytes {
        for copy in copies.iter().filter(|copy| copy.regular_file) {
            if preserved_already_holds(&copy.path, bytes) {
                if let Err(error) =
                    crate::session::primary::make_session_artifact_private(&copy.path)
                {
                    warn!(
                        "Diagnostic bytes are preserved at {}, but permissions could not be restricted: {error}",
                        copy.path.display()
                    );
                }
                prune_corrupt_copies(artifact, &copy.path);
                return Ok(copy.path.clone());
            }
        }
    }

    let last_seq = copies.last().map_or(0, |copy| copy.seq);
    for attempt in 1..=4 {
        let seq = last_seq
            .checked_add(attempt)
            .ok_or_else(|| anyhow!("corrupt copy sequence exhausted"))?;
        let target = crate::session::artifacts::corrupt_copy_path(artifact, seq);
        if let Some(bytes) = &named_bytes {
            match write_atomic(
                &target,
                bytes,
                AtomicWriteOptions {
                    overwrite: OverwriteMode::CreateNew,
                    permissions: PermissionPolicy::FixedMode(0o600),
                    symlink: SymlinkPolicy::Reject,
                    sync_file: true,
                    sync_parent: true,
                },
            ) {
                Ok(()) => {
                    prune_corrupt_copies(artifact, &target);
                    return Ok(target);
                }
                Err(
                    DurableIoError::AlreadyExists { .. } | DurableIoError::SymlinkRejected { .. },
                ) => continue,
                Err(err) => warn!("Could not copy unreadable session; moving it aside: {err}"),
            }
        }
        match crate::session::artifacts::rename_artifact_no_replace(artifact, &target) {
            Ok(()) => {
                if let Err(err) = crate::session::primary::make_session_artifact_private(&target) {
                    warn!(
                        "Could not restrict corrupt copy {}: {err}",
                        target.display()
                    );
                }
                if let Err(err) = sync_parent_dir(&target) {
                    warn!("Could not sync corrupt copy {}: {err}", target.display());
                }
                prune_corrupt_copies(artifact, &target);
                return Ok(target);
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err).context("failed to move unreadable artifact aside"),
        }
    }

    Err(anyhow!("could not find a free corrupt-copy name"))
}

fn prune_corrupt_copies(artifact: &Path, keep: &Path) {
    let Ok(copies) = crate::session::artifacts::corrupt_copies_of(artifact) else {
        return;
    };
    let mut count = copies.iter().filter(|copy| copy.regular_file).count();
    for copy in copies
        .into_iter()
        .filter(|copy| copy.regular_file && copy.path != keep)
    {
        if count <= crate::session::artifacts::MAX_CORRUPT_COPIES_PER_ARTIFACT {
            break;
        }
        match fs::remove_file(&copy.path) {
            Ok(()) => count -= 1,
            Err(err) => warn!(
                "Could not prune corrupt copy {}: {err}",
                copy.path.display()
            ),
        }
    }
}

/// Atomically install restored ink before a remembered load can finish or abort.
/// The exclusive lock and a replace conditional on the corrupt file it read
/// keep a concurrent primary intact. Ink larger than a session file may be
/// stays where it was restored from until a save writes the primary.
pub(super) fn restore_named_primary_after_corruption(
    options: &SessionOptions,
    copy: &Path,
    source: RestoredArtifact,
    max_expanded_size: u64,
) -> Result<Option<Box<SessionSnapshot>>> {
    use std::io::{Seek, SeekFrom};
    let lock = open_runtime_lock_file(&options.lock_file_path(), true)?;
    crate::session::lock::lock_exclusive(&lock)?;
    let result = (|| {
        let primary = options.session_file_path();
        let copy_len = session_artifact_metadata(copy, true)?.len();
        let corrupt = match session_artifact_metadata_if_exists(&primary, true)? {
            Some(metadata) if metadata.len() != copy_len => return Ok(None),
            Some(metadata) => {
                let bytes = read_artifact_bounded(&primary, copy_len)?;
                if !preserved_already_holds(copy, &bytes) {
                    return Ok(None);
                }
                Some((FileIdentity::of(&metadata), bytes))
            }
            None => None,
        };
        let source_path = match source {
            RestoredArtifact::Backup => options.backup_file_path(),
            RestoredArtifact::Recovery => options.recovery_file_path(),
        };
        let mut source_file = open_session_artifact_for_read(&source_path, true)?;
        let loaded = payload::load_snapshot_opened_with_expanded_limit(
            &source_path,
            options,
            source_file.try_clone()?,
            max_expanded_size,
            None,
            NewerVersionAction::LeaveUntouched,
        )?
        .ok_or_else(|| anyhow!("restored artifact changed before primary repair"))?;
        source_file.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        (&mut source_file)
            .take(max_expanded_size.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > max_expanded_size {
            return Err(anyhow!("restored artifact exceeds bounded repair limit"));
        }
        if bytes.len() as u64 > options.max_file_size_bytes {
            warn!(
                "Restored {} is larger than a session file may be; {} is written by the next save",
                source_path.display(),
                primary.display()
            );
            return Ok(Some(Box::new(loaded.snapshot)));
        }

        let (overwrite, expected) = match &corrupt {
            Some((identity, contents)) => (
                OverwriteMode::Replace,
                DestinationExpectation::Present {
                    identity: *identity,
                    contents,
                },
            ),
            None => (OverwriteMode::CreateNew, DestinationExpectation::Absent),
        };
        match write_atomic_reporting_identity(
            &primary,
            &bytes,
            AtomicWriteOptions {
                overwrite,
                permissions: PermissionPolicy::FixedMode(0o600),
                symlink: SymlinkPolicy::Reject,
                sync_file: true,
                sync_parent: false,
            },
            Some(expected),
        ) {
            Ok(_) => {}
            Err(DurableIoError::DestinationChanged { .. }) => return Ok(None),
            Err(error) => return Err(error.into()),
        }

        // The restored file is in place; a failed directory sync only makes
        // the rename less durable, so it must not discard the restore.
        if let Err(error) = sync_parent_dir(&primary) {
            warn!(
                "Could not sync the directory of restored {}: {error}",
                primary.display()
            );
        }

        info!(
            "Restored primary atomically; unreadable bytes remain at {}",
            copy.display()
        );
        Ok(Some(Box::new(loaded.snapshot)))
    })();
    if let Err(error) = unlock(&lock) {
        warn!("Could not unlock restored session: {error}");
    }
    result
}

/// Copy a session written by a newer wayscriber to a content-addressed side
/// path, so that saves and rotations from this build can never destroy it.
///
/// The side name carries a digest of the loaded bytes, so a *different*
/// session from the same newer version gets its own copy — a fixed
/// per-version name preserved only the first one and let rotation destroy any
/// later ones. A name already holding exactly these bytes is a completed
/// preservation and is left alone; anything else at that name (a directory, a
/// symlink, a truncated earlier attempt, a digest collision) is stepped over
/// with a counter rather than trusted.
///
/// If no copy can be written (disk full, permissions), the primary itself is
/// renamed to a free side name instead: a rename needs no free space, and
/// leaving the file where rotation reaches it is the one unacceptable
/// outcome. Both copy and move candidates are no-replace. The original stays
/// in place whenever a copy succeeds, so a newer wayscriber finds its session
/// untouched.
pub(super) fn preserve_newer_version_session(
    session_path: &Path,
    bytes: &[u8],
    version: u64,
) -> Result<PathBuf> {
    let digest = content_digest(bytes);
    let base = format!(
        ".v{version}{}{digest:016x}",
        crate::session::artifacts::PRESERVED_SESSION_MARKER
    );
    let mut last_error = None;

    for attempt in 0..MAX_PRESERVE_ATTEMPTS {
        let suffix = if attempt == 0 {
            base.clone()
        } else {
            format!("{base}-{attempt}")
        };
        let preserved_path = crate::session::append_path_suffix(session_path, &suffix);

        if preserved_already_holds(&preserved_path, bytes) {
            crate::session::primary::make_session_artifact_private(&preserved_path)?;
            debug!(
                "Newer-version session {} is already preserved at {}",
                session_path.display(),
                preserved_path.display()
            );
            return Ok(preserved_path);
        }

        match write_atomic(
            &preserved_path,
            bytes,
            AtomicWriteOptions {
                overwrite: OverwriteMode::CreateNew,
                permissions: PermissionPolicy::FixedMode(0o600),
                symlink: SymlinkPolicy::Reject,
                sync_file: true,
                sync_parent: true,
            },
        ) {
            Ok(()) => return Ok(preserved_path),
            // Something else is at that name and did not match our bytes:
            // step over it rather than claim a preservation we did not make.
            Err(DurableIoError::AlreadyExists { .. }) => continue,
            Err(err) => {
                last_error = Some(err);
                break;
            }
        }
    }

    match last_error {
        Some(err) => warn!(
            "Could not copy newer-version session {} ({err}); moving the file itself out of rotation's reach",
            session_path.display()
        ),
        None => warn!(
            "Could not find a free preserved name for newer-version session {}; moving the file itself out of rotation's reach",
            session_path.display()
        ),
    }

    for attempt in 0..MAX_PRESERVE_ATTEMPTS {
        let suffix = if attempt == 0 {
            format!(
                "{base}{}",
                crate::session::artifacts::PRESERVED_SESSION_MOVED_SUFFIX
            )
        } else {
            format!(
                "{base}{}-{attempt}",
                crate::session::artifacts::PRESERVED_SESSION_MOVED_SUFFIX
            )
        };
        let fallback_path = crate::session::append_path_suffix(session_path, &suffix);

        // A previous fallback may already have established the required copy.
        // Trust it only after the same exact-byte verification as copy paths.
        if preserved_already_holds(&fallback_path, bytes) {
            crate::session::primary::make_session_artifact_private(&fallback_path)?;
            return Ok(fallback_path);
        }

        match crate::session::artifacts::rename_artifact_no_replace(session_path, &fallback_path) {
            Ok(()) => {
                crate::session::primary::make_session_artifact_private(&fallback_path)?;
                sync_parent_dir(&fallback_path).with_context(|| {
                    format!(
                        "moved newer-version session to {}, but failed to sync its directory",
                        fallback_path.display()
                    )
                })?;
                return Ok(fallback_path);
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "failed to preserve newer-version session at {}",
                        fallback_path.display()
                    )
                });
            }
        }
    }

    Err(anyhow!(
        "no free fallback preservation name remained for newer-version session {}",
        session_path.display()
    ))
}

/// How many digest-suffixed names to try before falling back to moving the
/// primary. Only a collision or a leftover foreign entry consumes one.
const MAX_PRESERVE_ATTEMPTS: usize = 16;

/// Whether the path is a regular file already holding exactly `bytes` — the
/// only state that counts as a completed preservation.
fn preserved_already_holds(preserved_path: &Path, bytes: &[u8]) -> bool {
    let Ok(metadata) = fs::symlink_metadata(preserved_path) else {
        return false;
    };
    if !metadata.is_file() || metadata.len() != bytes.len() as u64 {
        return false;
    }
    read_artifact_bounded(preserved_path, bytes.len() as u64)
        .is_ok_and(|existing| existing == bytes)
}

/// FNV-1a over the loaded bytes: stable across processes and builds, which
/// the content-addressed side name requires. A collision only skips one extra
/// copy of same-user data; cryptographic strength buys nothing here.
fn content_digest(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn read_artifact_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = open_session_artifact_for_read(path, true)?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(anyhow!(
            "unreadable artifact exceeds preservation read limit"
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restored_backup_cleanup_does_not_remove_a_changed_named_primary() {
        let temp = crate::test_temp::tempdir().unwrap();
        let mut options = SessionOptions::new(temp.path().to_path_buf(), "named");
        options.persist_transparent = true;
        options.set_named_file_target(temp.path().join("board.wayscriber-session"));
        let replacement = saved_session_bytes(temp.path());
        let corrupt_bytes = vec![b'{'; replacement.len()];
        fs::write(options.session_file_path(), &corrupt_bytes).unwrap();
        let corrupt_copy = with_shared_session_lock(&options, || {
            quarantine_corrupt_artifact(&options.session_file_path(), &options, 4096)
        })
        .unwrap();
        // Another cooperating writer can run between preservation and restoration.
        fs::write(options.session_file_path(), &replacement).unwrap();
        fs::write(options.backup_file_path(), b"saved backup").unwrap();

        assert!(
            restore_named_primary_after_corruption(
                &options,
                &corrupt_copy,
                RestoredArtifact::Backup,
                4096
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(fs::read(options.session_file_path()).unwrap(), replacement);
        assert_eq!(
            fs::read(options.backup_file_path()).unwrap(),
            b"saved backup"
        );
        assert_eq!(fs::read(corrupt_copy).unwrap(), corrupt_bytes);
    }

    /// Session bytes as this build writes them.
    fn saved_session_bytes(dir: &Path) -> Vec<u8> {
        let mut scratch = SessionOptions::new(dir.to_path_buf(), "scratch");
        scratch.persist_transparent = true;
        scratch.set_named_file_target(dir.join("scratch.wayscriber-session"));
        crate::session::save_snapshot(&super::super::super::tests::sample_snapshot(), &scratch)
            .unwrap();
        fs::read(scratch.session_file_path()).unwrap()
    }

    fn quarantined_named_session(temp: &Path) -> (SessionOptions, PathBuf, Vec<u8>) {
        let mut options = SessionOptions::new(temp.to_path_buf(), "named");
        options.persist_transparent = true;
        options.set_named_file_target(temp.join("board.wayscriber-session"));
        let corrupt_bytes = vec![b'{'; 40];
        fs::write(options.session_file_path(), &corrupt_bytes).unwrap();
        let corrupt_copy = with_shared_session_lock(&options, || {
            quarantine_corrupt_artifact(&options.session_file_path(), &options, 4096)
        })
        .unwrap();
        (options, corrupt_copy, corrupt_bytes)
    }

    #[test]
    fn restored_ink_too_large_for_a_session_file_is_not_written_into_the_primary() {
        let temp = crate::test_temp::tempdir().unwrap();
        let (mut options, corrupt_copy, corrupt_bytes) = quarantined_named_session(temp.path());
        let backup = saved_session_bytes(temp.path());
        fs::write(options.backup_file_path(), &backup).unwrap();
        options.max_file_size_bytes = backup.len() as u64 - 1;

        let restored = restore_named_primary_after_corruption(
            &options,
            &corrupt_copy,
            RestoredArtifact::Backup,
            4096,
        )
        .unwrap()
        .expect("the restored ink is still loaded");

        assert!(restored.has_board_data());
        assert_eq!(
            fs::read(options.session_file_path()).unwrap(),
            corrupt_bytes
        );
        assert_eq!(fs::read(options.backup_file_path()).unwrap(), backup);
    }

    #[test]
    fn restore_creates_a_primary_that_was_moved_aside() {
        let temp = crate::test_temp::tempdir().unwrap();
        let (options, corrupt_copy, _) = quarantined_named_session(temp.path());
        fs::remove_file(options.session_file_path()).unwrap();
        let backup = saved_session_bytes(temp.path());
        fs::write(options.backup_file_path(), &backup).unwrap();

        let restored = restore_named_primary_after_corruption(
            &options,
            &corrupt_copy,
            RestoredArtifact::Backup,
            4096,
        )
        .unwrap()
        .expect("restored");

        assert!(restored.has_board_data());
        assert_eq!(fs::read(options.session_file_path()).unwrap(), backup);
    }
}
