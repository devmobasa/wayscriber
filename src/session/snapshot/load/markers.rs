use super::*;

pub(super) fn backup_is_newer_than_primary(backup: &fs::Metadata, primary: &fs::Metadata) -> bool {
    match (backup.modified(), primary.modified()) {
        (Ok(backup_modified), Ok(primary_modified)) => backup_modified > primary_modified,
        _ => false,
    }
}

pub(super) fn recoverable_backup_marker_metadata(
    options: &SessionOptions,
    clear_marker_metadata: Option<&fs::Metadata>,
) -> Option<fs::Metadata> {
    let marker_path = options.backup_recovery_marker_file_path();
    let marker_metadata =
        session_marker_metadata_if_exists("backup recovery marker", &marker_path, options)?;
    if clear_marker_suppresses_artifact(
        "backup recovery marker",
        &marker_path,
        &marker_metadata,
        clear_marker_metadata,
    ) {
        return None;
    }
    Some(marker_metadata)
}

pub(super) fn recoverable_recovery_marker_metadata(
    options: &SessionOptions,
    clear_marker_metadata: Option<&fs::Metadata>,
) -> Option<fs::Metadata> {
    let marker_path = options.recovery_recoverable_marker_file_path();
    let marker_metadata =
        session_marker_metadata_if_exists("recovery recoverable marker", &marker_path, options)?;
    if clear_marker_suppresses_artifact(
        "recovery recoverable marker",
        &marker_path,
        &marker_metadata,
        clear_marker_metadata,
    ) {
        return None;
    }
    Some(marker_metadata)
}

pub(super) fn clear_marker_metadata(options: &SessionOptions) -> Option<fs::Metadata> {
    let marker_path = options.clear_marker_file_path();
    session_marker_metadata_if_exists("clear marker", &marker_path, options)
}

fn session_marker_metadata_if_exists(
    label: &str,
    path: &Path,
    options: &SessionOptions,
) -> Option<fs::Metadata> {
    match session_artifact_metadata_if_exists(path, options.is_named_file()) {
        Ok(metadata) => metadata,
        Err(err) if is_non_regular_session_artifact(&err) => {
            warn!(
                "Ignoring non-regular session {} {}: {}",
                label,
                path.display(),
                err
            );
            None
        }
        Err(err) => {
            warn!(
                "Ignoring unreadable session {} {}: {:#}",
                label,
                path.display(),
                err
            );
            None
        }
    }
}

pub(super) fn clear_marker_suppresses_artifact(
    label: &str,
    path: &Path,
    artifact_metadata: &fs::Metadata,
    clear_marker_metadata: Option<&fs::Metadata>,
) -> bool {
    let Some(clear_marker_metadata) = clear_marker_metadata else {
        return false;
    };
    let artifact_newer_than_marker = match (
        artifact_metadata.modified(),
        clear_marker_metadata.modified(),
    ) {
        (Ok(artifact_modified), Ok(marker_modified)) => artifact_modified > marker_modified,
        _ => false,
    };
    if artifact_newer_than_marker {
        return false;
    }
    info!(
        "Ignoring {} {} because it is not newer than the session clear marker",
        label,
        path.display()
    );
    true
}

pub(super) fn preserve_unloadable_recovery(path: &Path, reason: &str) {
    if !path.exists() {
        return;
    }
    let preserved_path = preserved_recovery_path(path, reason);
    match fs::rename(path, &preserved_path) {
        Ok(()) => warn!(
            "Preserved unloadable session recovery artifact as {}",
            preserved_path.display()
        ),
        Err(err) => warn!(
            "Failed to preserve unloadable session recovery artifact {}: {}",
            path.display(),
            err
        ),
    }
}

fn preserved_recovery_path(path: &Path, reason: &str) -> PathBuf {
    for index in 0..100 {
        let suffix = if index == 0 {
            format!(".{reason}")
        } else {
            format!(".{reason}.{index}")
        };
        let candidate = crate::session::append_path_suffix(path, &suffix);
        if !candidate.exists() {
            return candidate;
        }
    }

    crate::session::append_path_suffix(path, &format!(".{reason}.100"))
}

pub(super) fn should_prefer_recovery(
    recovery_metadata: &fs::Metadata,
    session_metadata: Option<&fs::Metadata>,
) -> bool {
    let Some(session_metadata) = session_metadata else {
        return true;
    };
    match (recovery_metadata.modified(), session_metadata.modified()) {
        (Ok(recovery_modified), Ok(session_modified)) => recovery_modified >= session_modified,
        _ => true,
    }
}
