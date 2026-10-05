use super::*;

// These arguments carry the loader artifact view and its preservation/size policy.
#[allow(clippy::too_many_arguments)]
pub(super) fn load_normal_session_or_empty(
    options: &SessionOptions,
    session_path: &Path,
    session_metadata: Option<fs::Metadata>,
    max_expanded_size: u64,
    clear_marker_metadata: Option<&fs::Metadata>,
    backup_recovery_marker_metadata: Option<&fs::Metadata>,
    recovery_recoverable_marker_metadata: Option<&fs::Metadata>,
    restore_named_primary: bool,
) -> Result<LoadSnapshotOutcome> {
    let Some(primary_metadata) = session_metadata.as_ref() else {
        if let Some(backup) = load_contentful_backup(
            options,
            max_expanded_size,
            None,
            clear_marker_metadata,
            backup_recovery_marker_metadata,
            recovery_recoverable_marker_metadata,
        )? {
            return Ok(LoadSnapshotOutcome::LoadedFromBackup(backup));
        }
        info!(
            "Session file not found at {}; skipping load",
            session_path.display()
        );
        return Ok(LoadSnapshotOutcome::Empty);
    };

    if clear_marker_suppresses_artifact(
        "primary session",
        session_path,
        primary_metadata,
        clear_marker_metadata,
    ) {
        match load_snapshot_path_with_outcome(
            session_path,
            options,
            max_expanded_size,
            true,
            "session",
            CorruptLoadAction::Preserve,
        )? {
            LoadSnapshotOutcome::Loaded(snapshot) if !snapshot.has_board_data() => {
                return Ok(LoadSnapshotOutcome::Loaded(snapshot));
            }
            LoadSnapshotOutcome::Loaded(_) => {}
            LoadSnapshotOutcome::Empty
            | LoadSnapshotOutcome::EmptyAfterCorruption { .. }
            | LoadSnapshotOutcome::NonRegularArtifact { .. }
            | LoadSnapshotOutcome::ExpandedTooLarge { .. } => {}
            LoadSnapshotOutcome::RestoredAfterCorruption { .. }
            | LoadSnapshotOutcome::LoadedFromBackup(_)
            | LoadSnapshotOutcome::LoadedFromRecovery(_) => {}
        }

        if let Some(backup) = load_contentful_backup(
            options,
            max_expanded_size,
            None,
            clear_marker_metadata,
            backup_recovery_marker_metadata,
            recovery_recoverable_marker_metadata,
        )? {
            return Ok(LoadSnapshotOutcome::LoadedFromBackup(backup));
        }
        return Ok(LoadSnapshotOutcome::Empty);
    }

    let outcome = load_snapshot_path_with_outcome(
        session_path,
        options,
        max_expanded_size,
        true,
        "session",
        CorruptLoadAction::Quarantine,
    )?;
    let snapshot = match outcome {
        LoadSnapshotOutcome::Loaded(snapshot) => snapshot,
        LoadSnapshotOutcome::EmptyAfterCorruption { corrupt_copy } => {
            return load_after_corrupt_primary(
                options,
                max_expanded_size,
                clear_marker_metadata,
                backup_recovery_marker_metadata,
                recovery_recoverable_marker_metadata,
                corrupt_copy,
                restore_named_primary,
            );
        }
        other => return Ok(other),
    };

    if snapshot.has_board_data() {
        return Ok(LoadSnapshotOutcome::Loaded(snapshot));
    }

    if let Some(backup) = load_contentful_backup(
        options,
        max_expanded_size,
        Some(primary_metadata),
        clear_marker_metadata,
        backup_recovery_marker_metadata,
        recovery_recoverable_marker_metadata,
    )? {
        return Ok(LoadSnapshotOutcome::LoadedFromBackup(backup));
    }

    if let Some(recovery) = load_contentful_recovery(
        options,
        max_expanded_size,
        clear_marker_metadata,
        recovery_recoverable_marker_metadata,
    )? {
        return Ok(LoadSnapshotOutcome::LoadedFromRecovery(recovery));
    }

    Ok(LoadSnapshotOutcome::Loaded(snapshot))
}

fn load_contentful_backup(
    options: &SessionOptions,
    max_expanded_size: u64,
    primary_metadata: Option<&fs::Metadata>,
    clear_marker_metadata: Option<&fs::Metadata>,
    backup_recovery_marker_metadata: Option<&fs::Metadata>,
    recovery_recoverable_marker_metadata: Option<&fs::Metadata>,
) -> Result<Option<Box<SessionSnapshot>>> {
    let backup_path = options.backup_file_path();
    let Some(backup_metadata) = fs::metadata(&backup_path).ok() else {
        return Ok(None);
    };

    if clear_marker_suppresses_artifact(
        "session backup",
        &backup_path,
        &backup_metadata,
        clear_marker_metadata,
    ) {
        return Ok(None);
    }

    if let Some(primary_metadata) = primary_metadata
        && backup_recovery_marker_metadata.is_none()
        && !backup_is_newer_than_primary(&backup_metadata, primary_metadata)
    {
        info!(
            "Primary session {} contains no board data, but backup {} is not newer; keeping primary session",
            options.session_file_path().display(),
            backup_path.display()
        );
        return Ok(None);
    }

    if primary_metadata.is_some() {
        warn!(
            "Primary session {} contains no board data; checking newer backup {} before accepting the blank session",
            options.session_file_path().display(),
            backup_path.display()
        );
    } else {
        warn!(
            "Checking recoverable backup for primary {} at {}",
            options.session_file_path().display(),
            backup_path.display()
        );
    }

    match load_snapshot_path_with_outcome(
        &backup_path,
        options,
        max_expanded_size,
        true,
        "session backup",
        CorruptLoadAction::Quarantine,
    )? {
        LoadSnapshotOutcome::Loaded(snapshot) if snapshot.has_board_data() => {
            warn!(
                "Restoring board data from backup {} because primary session {} contained no board data",
                backup_path.display(),
                options.session_file_path().display()
            );
            Ok(Some(snapshot))
        }
        LoadSnapshotOutcome::Loaded(_) => {
            info!(
                "Session backup {} also contains no board data; keeping primary session",
                backup_path.display()
            );
            Ok(None)
        }
        LoadSnapshotOutcome::Empty
        | LoadSnapshotOutcome::EmptyAfterCorruption { .. }
        | LoadSnapshotOutcome::NonRegularArtifact { .. } => Ok(None),
        LoadSnapshotOutcome::ExpandedTooLarge { path, .. } => {
            warn!(
                "Session backup {} is too large to restore; keeping primary session",
                path.display()
            );
            Ok(None)
        }
        LoadSnapshotOutcome::RestoredAfterCorruption { .. }
        | LoadSnapshotOutcome::LoadedFromBackup(_)
        | LoadSnapshotOutcome::LoadedFromRecovery(_) => load_contentful_recovery(
            options,
            max_expanded_size,
            clear_marker_metadata,
            recovery_recoverable_marker_metadata,
        ),
    }
}

fn load_contentful_recovery(
    options: &SessionOptions,
    max_expanded_size: u64,
    clear_marker_metadata: Option<&fs::Metadata>,
    recovery_recoverable_marker_metadata: Option<&fs::Metadata>,
) -> Result<Option<Box<SessionSnapshot>>> {
    if recovery_recoverable_marker_metadata.is_none() {
        return Ok(None);
    }

    let recovery_path = options.recovery_file_path();
    let Some(recovery_metadata) = fs::metadata(&recovery_path).ok() else {
        return Ok(None);
    };
    if clear_marker_suppresses_artifact(
        "session recovery",
        &recovery_path,
        &recovery_metadata,
        clear_marker_metadata,
    ) {
        return Ok(None);
    }

    match load_snapshot_path_with_outcome(
        &recovery_path,
        options,
        max_expanded_size,
        false,
        "session recovery",
        CorruptLoadAction::Quarantine,
    )? {
        LoadSnapshotOutcome::Loaded(snapshot) if snapshot.has_board_data() => Ok(Some(snapshot)),
        LoadSnapshotOutcome::Loaded(_)
        | LoadSnapshotOutcome::Empty
        | LoadSnapshotOutcome::EmptyAfterCorruption { .. }
        | LoadSnapshotOutcome::NonRegularArtifact { .. } => Ok(None),
        LoadSnapshotOutcome::ExpandedTooLarge { path, .. } => {
            preserve_unloadable_recovery(&path, "too-large");
            Ok(None)
        }
        LoadSnapshotOutcome::RestoredAfterCorruption { .. }
        | LoadSnapshotOutcome::LoadedFromBackup(_)
        | LoadSnapshotOutcome::LoadedFromRecovery(_) => Ok(None),
    }
}

/// An unreadable primary is not an intentional blank: try older usable artifacts.
fn load_after_corrupt_primary(
    options: &SessionOptions,
    max_expanded_size: u64,
    clear_marker: Option<&fs::Metadata>,
    backup_marker: Option<&fs::Metadata>,
    recovery_marker: Option<&fs::Metadata>,
    corrupt_copy: PathBuf,
    restore_named_primary: bool,
) -> Result<LoadSnapshotOutcome> {
    let restored = match load_contentful_backup(
        options,
        max_expanded_size,
        None,
        clear_marker,
        backup_marker,
        recovery_marker,
    )? {
        Some(snapshot) => Some((snapshot, RestoredArtifact::Backup)),
        None => {
            load_contentful_recovery(options, max_expanded_size, clear_marker, recovery_marker)?
                .map(|snapshot| (snapshot, RestoredArtifact::Recovery))
        }
    };
    let Some((mut snapshot, source)) = restored else {
        return Ok(LoadSnapshotOutcome::EmptyAfterCorruption { corrupt_copy });
    };

    if options.is_named_file() && restore_named_primary {
        snapshot = corrupt::restore_named_primary_after_corruption(
            options,
            &corrupt_copy,
            source,
            max_expanded_size,
        )?
        .ok_or_else(|| {
            anyhow!("named primary changed during restoration; retry the load without replacing it")
        })?;
    }
    Ok(LoadSnapshotOutcome::RestoredAfterCorruption {
        snapshot,
        source,
        corrupt_copy,
    })
}
