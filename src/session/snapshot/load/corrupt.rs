use super::*;
use std::io::ErrorKind;

/// Preserves an unreadable session's bytes and reports where they went, so
/// the caller can tell the user rather than leaving the loss to the log.
pub(super) fn backup_corrupt_session(
    session_path: &Path,
    options: &SessionOptions,
) -> Result<PathBuf> {
    let named_primary = is_named_primary_path(session_path, options);
    let bytes = read_session_artifact_bytes(session_path, named_primary)?;
    let primary_path = options.session_file_path();
    let backup_path = if session_path == primary_path.as_path() {
        options.backup_file_path()
    } else {
        options.corrupt_artifact_backup_file_path(session_path)
    };
    crate::durable_io::write_atomic(
        &backup_path,
        &bytes,
        crate::durable_io::AtomicWriteOptions {
            overwrite: crate::durable_io::OverwriteMode::Replace,
            permissions: crate::durable_io::PermissionPolicy::PreserveExistingOrMode(0o600),
            symlink: crate::durable_io::SymlinkPolicy::Reject,
            sync_file: true,
            sync_parent: true,
        },
    )
    .with_context(|| format!("failed to write session backup {}", backup_path.display()))?;
    if named_primary {
        debug!(
            "Backed up corrupt named session primary {} to {}; leaving the selected primary in place",
            session_path.display(),
            backup_path.display()
        );
        return Ok(backup_path);
    }
    fs::remove_file(session_path).with_context(|| {
        format!(
            "failed to remove corrupt session {}",
            session_path.display()
        )
    })?;
    Ok(backup_path)
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
            debug!(
                "Newer-version session {} is already preserved at {}",
                session_path.display(),
                preserved_path.display()
            );
            return Ok(preserved_path);
        }

        match crate::durable_io::write_atomic(
            &preserved_path,
            bytes,
            crate::durable_io::AtomicWriteOptions {
                overwrite: crate::durable_io::OverwriteMode::CreateNew,
                permissions: crate::durable_io::PermissionPolicy::PreserveExistingOrMode(0o600),
                symlink: crate::durable_io::SymlinkPolicy::Reject,
                sync_file: true,
                sync_parent: true,
            },
        ) {
            Ok(()) => return Ok(preserved_path),
            // Something else is at that name and did not match our bytes:
            // step over it rather than claim a preservation we did not make.
            Err(crate::durable_io::DurableIoError::AlreadyExists { .. }) => continue,
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
            return Ok(fallback_path);
        }

        match crate::session::artifacts::rename_artifact_no_replace(session_path, &fallback_path) {
            Ok(()) => {
                crate::durable_io::sync_parent_dir(&fallback_path).with_context(|| {
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
    fs::read(preserved_path).is_ok_and(|existing| existing == bytes)
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

fn read_session_artifact_bytes(session_path: &Path, no_follow: bool) -> Result<Vec<u8>> {
    if !no_follow {
        return fs::read(session_path)
            .with_context(|| format!("failed to read session {}", session_path.display()));
    }

    let mut file = open_session_artifact_for_read(session_path, true)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .with_context(|| format!("failed to read session {}", session_path.display()))?;
    Ok(bytes)
}
