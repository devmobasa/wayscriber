use super::*;

pub(super) fn log_named_candidate_outcome(session_path: &Path, outcome: &LoadSnapshotOutcome) {
    let (source, snapshot) = match outcome {
        LoadSnapshotOutcome::Loaded(snapshot) => ("primary", snapshot),
        LoadSnapshotOutcome::LoadedFromBackup(snapshot) => ("backup", snapshot),
        LoadSnapshotOutcome::LoadedFromRecovery(snapshot) => ("recovery", snapshot),
        LoadSnapshotOutcome::Empty
        | LoadSnapshotOutcome::NonRegularArtifact { .. }
        | LoadSnapshotOutcome::ExpandedTooLarge { .. } => return,
    };
    info!(
        "Loaded named session candidate {} from {} (boards={}, active_board={}, tool_state={})",
        source,
        session_path.display(),
        snapshot.boards.len(),
        snapshot.active_board_id,
        snapshot.tool_state.is_some()
    );
}

#[allow(dead_code)]
pub(super) fn load_named_candidate_with_fallbacks(
    session_path: &Path,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<LoadSnapshotOutcome> {
    let primary_metadata = session_artifact_metadata(session_path, true)?;
    let recovery_path = options.recovery_file_path();
    let recovery_metadata = named_candidate_artifact_metadata_if_exists(&recovery_path)?;
    let clear_marker_metadata = clear_marker_metadata(options);
    let backup_recovery_marker_metadata =
        recoverable_backup_marker_metadata(options, clear_marker_metadata.as_ref());
    let recovery_recoverable_marker_metadata =
        recoverable_recovery_marker_metadata(options, clear_marker_metadata.as_ref());

    if let Some(recovery_metadata) = recovery_metadata.as_ref()
        && should_prefer_recovery(recovery_metadata, Some(&primary_metadata))
        && !clear_marker_suppresses_artifact(
            "session recovery",
            &recovery_path,
            recovery_metadata,
            clear_marker_metadata.as_ref(),
        )
    {
        match load_named_candidate_sidecar_artifact(
            "session recovery",
            &recovery_path,
            options,
            max_expanded_size,
            CandidateSizeLimit::ExpandedOnly,
        )? {
            LoadSnapshotOutcome::Loaded(snapshot) => {
                return Ok(LoadSnapshotOutcome::LoadedFromRecovery(snapshot));
            }
            LoadSnapshotOutcome::ExpandedTooLarge {
                path,
                max_expanded_size,
            } => {
                warn!(
                    "Named session candidate recovery {} is newer but expands beyond the {} byte safety limit; aborting open to preserve the recovery artifact",
                    path.display(),
                    max_expanded_size
                );
                return Ok(LoadSnapshotOutcome::ExpandedTooLarge {
                    path,
                    max_expanded_size,
                });
            }
            LoadSnapshotOutcome::Empty | LoadSnapshotOutcome::NonRegularArtifact { .. } => {}
            LoadSnapshotOutcome::LoadedFromBackup(_)
            | LoadSnapshotOutcome::LoadedFromRecovery(_) => {}
        }
    }

    if clear_marker_suppresses_artifact(
        "primary session",
        session_path,
        &primary_metadata,
        clear_marker_metadata.as_ref(),
    ) {
        if let Some(primary_snapshot) =
            load_named_candidate_suppressed_primary(session_path, options, max_expanded_size)?
        {
            return Ok(LoadSnapshotOutcome::Loaded(primary_snapshot));
        }

        if let Some(backup) = load_named_candidate_contentful_backup(
            options,
            max_expanded_size,
            None,
            clear_marker_metadata.as_ref(),
            backup_recovery_marker_metadata.as_ref(),
        )? {
            return Ok(LoadSnapshotOutcome::LoadedFromBackup(backup));
        }

        return Ok(LoadSnapshotOutcome::Empty);
    }

    let primary_outcome = load_named_candidate_artifact(
        session_path,
        options,
        max_expanded_size,
        CandidateSizeLimit::Configured,
    )?;
    let LoadSnapshotOutcome::Loaded(primary_snapshot) = primary_outcome else {
        return Ok(primary_outcome);
    };
    if primary_snapshot.has_board_data() {
        return Ok(LoadSnapshotOutcome::Loaded(primary_snapshot));
    }

    if let Some(backup) = load_named_candidate_contentful_backup(
        options,
        max_expanded_size,
        Some(&primary_metadata),
        clear_marker_metadata.as_ref(),
        backup_recovery_marker_metadata.as_ref(),
    )? {
        return Ok(LoadSnapshotOutcome::LoadedFromBackup(backup));
    }

    if let Some(recovery) = load_named_candidate_contentful_recovery(
        options,
        max_expanded_size,
        clear_marker_metadata.as_ref(),
        recovery_recoverable_marker_metadata.as_ref(),
    )? {
        return Ok(LoadSnapshotOutcome::LoadedFromRecovery(recovery));
    }

    Ok(LoadSnapshotOutcome::Loaded(primary_snapshot))
}

#[derive(Clone, Copy)]
enum CandidateSizeLimit {
    Configured,
    ExpandedOnly,
}

fn load_named_candidate_artifact(
    session_path: &Path,
    options: &SessionOptions,
    max_expanded_size: u64,
    size_limit: CandidateSizeLimit,
) -> Result<LoadSnapshotOutcome> {
    let file = open_session_artifact_for_read(session_path, true)?;
    let metadata = file.metadata().with_context(|| {
        format!(
            "failed to inspect opened session file {}",
            session_path.display()
        )
    })?;
    let max_encoded_size = match size_limit {
        CandidateSizeLimit::Configured => {
            if metadata.len() > options.max_file_size_bytes {
                return Err(anyhow!(
                    "session file {} is {} bytes which exceeds the configured limit of {} bytes",
                    session_path.display(),
                    metadata.len(),
                    options.max_file_size_bytes
                ));
            }
            Some(options.max_file_size_bytes)
        }
        CandidateSizeLimit::ExpandedOnly => {
            if metadata.len() > max_expanded_size {
                return Ok(LoadSnapshotOutcome::ExpandedTooLarge {
                    path: session_path.to_path_buf(),
                    max_expanded_size,
                });
            }
            None
        }
    };

    match load_snapshot_opened_with_expanded_limit(
        session_path,
        options,
        file,
        max_expanded_size,
        max_encoded_size,
    ) {
        Ok(Some(loaded)) => Ok(LoadSnapshotOutcome::Loaded(Box::new(loaded.snapshot))),
        Ok(None) => Ok(LoadSnapshotOutcome::Empty),
        Err(err) if err.downcast_ref::<ExpandedSessionTooLarge>().is_some() => {
            Ok(LoadSnapshotOutcome::ExpandedTooLarge {
                path: session_path.to_path_buf(),
                max_expanded_size,
            })
        }
        Err(err) if is_non_regular_session_artifact(&err) => {
            Ok(LoadSnapshotOutcome::NonRegularArtifact {
                path: session_path.to_path_buf(),
            })
        }
        Err(err) => Err(err),
    }
}

fn load_named_candidate_sidecar_artifact(
    label: &str,
    session_path: &Path,
    options: &SessionOptions,
    max_expanded_size: u64,
    size_limit: CandidateSizeLimit,
) -> Result<LoadSnapshotOutcome> {
    match load_named_candidate_artifact(session_path, options, max_expanded_size, size_limit) {
        Ok(outcome) => Ok(outcome),
        Err(err) => {
            warn!(
                "Ignoring unreadable named session candidate {} {}; falling back without mutating it: {:#}",
                label,
                session_path.display(),
                err
            );
            Ok(LoadSnapshotOutcome::Empty)
        }
    }
}

fn load_named_candidate_suppressed_primary(
    session_path: &Path,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<Option<Box<SessionSnapshot>>> {
    match load_named_candidate_artifact(
        session_path,
        options,
        max_expanded_size,
        CandidateSizeLimit::Configured,
    ) {
        Ok(LoadSnapshotOutcome::Loaded(snapshot)) if !snapshot.has_board_data() => {
            Ok(Some(snapshot))
        }
        Ok(LoadSnapshotOutcome::Loaded(_)) => Ok(None),
        Ok(
            LoadSnapshotOutcome::Empty
            | LoadSnapshotOutcome::NonRegularArtifact { .. }
            | LoadSnapshotOutcome::ExpandedTooLarge { .. },
        ) => Ok(None),
        Ok(
            LoadSnapshotOutcome::LoadedFromBackup(_) | LoadSnapshotOutcome::LoadedFromRecovery(_),
        ) => Ok(None),
        Err(err) => {
            warn!(
                "Ignoring unreadable named session candidate primary {} because it is older than the clear marker: {:#}",
                session_path.display(),
                err
            );
            Ok(None)
        }
    }
}

fn named_candidate_artifact_metadata_if_exists(path: &Path) -> Result<Option<fs::Metadata>> {
    match session_artifact_metadata_if_exists(path, true) {
        Ok(metadata) => Ok(metadata),
        Err(err) if is_non_regular_session_artifact(&err) => {
            warn!(
                "Ignoring non-regular named session candidate sidecar {}: {}",
                path.display(),
                err
            );
            Ok(None)
        }
        Err(err) => Err(err),
    }
}

fn load_named_candidate_contentful_backup(
    options: &SessionOptions,
    max_expanded_size: u64,
    primary_metadata: Option<&fs::Metadata>,
    clear_marker_metadata: Option<&fs::Metadata>,
    backup_recovery_marker_metadata: Option<&fs::Metadata>,
) -> Result<Option<Box<SessionSnapshot>>> {
    let backup_path = options.backup_file_path();
    let Some(backup_metadata) = named_candidate_artifact_metadata_if_exists(&backup_path)? else {
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
        return Ok(None);
    }

    match load_named_candidate_sidecar_artifact(
        "session backup",
        &backup_path,
        options,
        max_expanded_size,
        CandidateSizeLimit::Configured,
    )? {
        LoadSnapshotOutcome::Loaded(snapshot) if snapshot.has_board_data() => Ok(Some(snapshot)),
        LoadSnapshotOutcome::Loaded(_) | LoadSnapshotOutcome::Empty => Ok(None),
        LoadSnapshotOutcome::NonRegularArtifact { .. } => Ok(None),
        LoadSnapshotOutcome::ExpandedTooLarge { path, .. } => Err(anyhow!(
            "named session backup is too large to open without mutating candidate artifacts: {}",
            path.display()
        )),
        LoadSnapshotOutcome::LoadedFromBackup(_) | LoadSnapshotOutcome::LoadedFromRecovery(_) => {
            Ok(None)
        }
    }
}

fn load_named_candidate_contentful_recovery(
    options: &SessionOptions,
    max_expanded_size: u64,
    clear_marker_metadata: Option<&fs::Metadata>,
    recovery_recoverable_marker_metadata: Option<&fs::Metadata>,
) -> Result<Option<Box<SessionSnapshot>>> {
    if recovery_recoverable_marker_metadata.is_none() {
        return Ok(None);
    }

    let recovery_path = options.recovery_file_path();
    let Some(recovery_metadata) = named_candidate_artifact_metadata_if_exists(&recovery_path)?
    else {
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

    match load_named_candidate_sidecar_artifact(
        "session recovery",
        &recovery_path,
        options,
        max_expanded_size,
        CandidateSizeLimit::ExpandedOnly,
    )? {
        LoadSnapshotOutcome::Loaded(snapshot) if snapshot.has_board_data() => Ok(Some(snapshot)),
        LoadSnapshotOutcome::Loaded(_) | LoadSnapshotOutcome::Empty => Ok(None),
        LoadSnapshotOutcome::NonRegularArtifact { .. } => Ok(None),
        LoadSnapshotOutcome::ExpandedTooLarge { path, .. } => Err(anyhow!(
            "named session recovery is too large to open without mutating candidate artifacts: {}",
            path.display()
        )),
        LoadSnapshotOutcome::LoadedFromBackup(_) | LoadSnapshotOutcome::LoadedFromRecovery(_) => {
            Ok(None)
        }
    }
}

#[allow(dead_code)]
fn load_named_candidate_primary(
    session_path: &Path,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<Option<LoadedSnapshot>> {
    let file = open_session_artifact_for_read(session_path, true)?;
    let metadata = file.metadata().with_context(|| {
        format!(
            "failed to inspect opened session file {}",
            session_path.display()
        )
    })?;
    if metadata.len() > options.max_file_size_bytes {
        return Err(anyhow!(
            "session file {} is {} bytes which exceeds the configured limit of {} bytes",
            session_path.display(),
            metadata.len(),
            options.max_file_size_bytes
        ));
    }
    load_snapshot_opened_with_expanded_limit(
        session_path,
        options,
        file,
        max_expanded_size,
        Some(options.max_file_size_bytes),
    )
}
