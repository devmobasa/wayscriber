use super::*;

pub(super) fn backup_corrupt_session(session_path: &Path, options: &SessionOptions) -> Result<()> {
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
        return Ok(());
    }
    fs::remove_file(session_path).with_context(|| {
        format!(
            "failed to remove corrupt session {}",
            session_path.display()
        )
    })?;
    Ok(())
}

/// Copy a session artifact written by a newer wayscriber to a versioned side
/// path, so that saves and rotations from this build can never destroy it. The
/// original file is left in place. The first preserved copy wins: once the side
/// file exists it is never overwritten, so repeated loads of the same too-new
/// file are no-ops.
pub(super) fn preserve_newer_version_session(
    session_path: &Path,
    options: &SessionOptions,
    version: u32,
) -> Result<Option<PathBuf>> {
    let preserved_path =
        crate::session::append_path_suffix(session_path, &format!(".v{version}-preserved"));
    if preserved_path.exists() {
        debug!(
            "Newer-version session {} is already preserved at {}",
            session_path.display(),
            preserved_path.display()
        );
        return Ok(None);
    }
    let named_primary = is_named_primary_path(session_path, options);
    let bytes = read_session_artifact_bytes(session_path, named_primary)?;
    match crate::durable_io::write_atomic(
        &preserved_path,
        &bytes,
        crate::durable_io::AtomicWriteOptions {
            overwrite: crate::durable_io::OverwriteMode::CreateNew,
            permissions: crate::durable_io::PermissionPolicy::PreserveExistingOrMode(0o600),
            symlink: crate::durable_io::SymlinkPolicy::Reject,
            sync_file: true,
            sync_parent: true,
        },
    ) {
        Ok(()) => Ok(Some(preserved_path)),
        Err(crate::durable_io::DurableIoError::AlreadyExists { .. }) => Ok(None),
        Err(err) => Err(err).with_context(|| {
            format!(
                "failed to preserve newer-version session at {}",
                preserved_path.display()
            )
        }),
    }
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
