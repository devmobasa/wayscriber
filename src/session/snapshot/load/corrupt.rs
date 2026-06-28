use super::*;

pub(super) fn backup_corrupt_session(session_path: &Path, options: &SessionOptions) -> Result<()> {
    let named_primary = is_named_primary_path(session_path, options);
    let bytes = read_corrupt_session_bytes(session_path, named_primary)?;
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

fn read_corrupt_session_bytes(session_path: &Path, no_follow: bool) -> Result<Vec<u8>> {
    if !no_follow {
        return fs::read(session_path)
            .with_context(|| format!("failed to read corrupt session {}", session_path.display()));
    }

    let mut file = open_session_artifact_for_read(session_path, true)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .with_context(|| format!("failed to read corrupt session {}", session_path.display()))?;
    Ok(bytes)
}
