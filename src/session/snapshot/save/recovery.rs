use super::payload::{log_payload_candidate, snapshot_without_history};
use super::*;

pub(super) fn save_recovery_snapshot(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    max_expanded_size: u64,
    last_modified: &str,
) -> Result<Option<SaveSnapshotReport>> {
    let Some((payload, outcome)) =
        recovery_payload(snapshot, options, max_expanded_size, last_modified)?
    else {
        return Ok(None);
    };

    let recovery_path = options.recovery_file_path();
    let tmp_path = temp_path(&recovery_path)?;
    let write_started = Instant::now();
    {
        let mut tmp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .with_context(|| {
                format!(
                    "failed to open temporary recovery file {}",
                    tmp_path.display()
                )
            })?;
        tmp_file
            .write_all(&payload.bytes)
            .context("failed to write session recovery payload")?;
        tmp_file
            .sync_all()
            .context("failed to sync temporary recovery file")?;
    }
    let write_elapsed = write_started.elapsed();

    let replace_started = Instant::now();
    fs::rename(&tmp_path, &recovery_path).with_context(|| {
        format!(
            "failed to move temporary recovery file {} -> {}",
            tmp_path.display(),
            recovery_path.display()
        )
    })?;
    info!(
        "Session recovery file replace completed for {}: write_and_sync={:?}, rename={:?}, final_size={} bytes",
        recovery_path.display(),
        write_elapsed,
        replace_started.elapsed(),
        payload.final_size()
    );

    Ok(Some(SaveSnapshotReport {
        path: recovery_path,
        outcome,
        raw_size: payload.raw_size,
        written_size: payload.final_size(),
        max_file_size_bytes: options.max_file_size_bytes,
        compressed: payload.compressed,
    }))
}

fn recovery_payload(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    max_expanded_size: u64,
    last_modified: &str,
) -> Result<Option<(PayloadCandidate, SaveSnapshotOutcome)>> {
    if snapshot.is_empty() && snapshot.tool_state.is_none() {
        return Ok(None);
    }

    let full_started = Instant::now();
    let full_payload = payload_candidate(snapshot, options, last_modified)?;
    log_payload_candidate("recovery full", &full_payload, full_started.elapsed());
    if full_payload.fits_expanded_limit(max_expanded_size) {
        return Ok(Some((full_payload, SaveSnapshotOutcome::Full)));
    }

    let full_limit = full_payload
        .expanded_limit_exceeded(max_expanded_size)
        .expect("full recovery payload should exceed expanded limit");
    warn!(
        "Full session recovery payload cannot be saved safely ({}; {} bytes written from {} raw bytes, compression={}); trying visible data without undo/redo history",
        full_limit.description(),
        full_payload.final_size(),
        full_payload.raw_size,
        full_payload.compressed
    );

    let visible_only = snapshot_without_history(snapshot);
    if visible_only.is_empty() && visible_only.tool_state.is_none() {
        return Ok(None);
    }
    let visible_started = Instant::now();
    let visible_payload = payload_candidate(&visible_only, options, last_modified)?;
    log_payload_candidate(
        "recovery visible-only",
        &visible_payload,
        visible_started.elapsed(),
    );
    if visible_payload.fits_expanded_limit(max_expanded_size) {
        return Ok(Some((visible_payload, SaveSnapshotOutcome::VisibleOnly)));
    }

    let visible_limit = visible_payload
        .expanded_limit_exceeded(max_expanded_size)
        .expect("visible recovery payload should exceed expanded limit");
    Err(anyhow!(
        "Session recovery data cannot be saved safely ({}; {} bytes written from {} raw bytes, compression={}); skipping recovery",
        visible_limit.description(),
        visible_payload.final_size(),
        visible_payload.raw_size,
        visible_payload.compressed
    ))
}

pub(super) fn remove_session_file_after_clear_marker(session_path: &Path) {
    match fs::remove_file(session_path) {
        Ok(()) => debug!(
            "Removed session file {} after writing clear marker",
            session_path.display()
        ),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => warn!(
            "Clear marker was written, but failed to remove stale session file {}: {}",
            session_path.display(),
            err
        ),
    }
}

pub(super) fn remove_recoverable_artifacts_suppressed_by_clear_marker(
    options: &SessionOptions,
) -> bool {
    let marker_path = options.clear_marker_file_path();
    let Ok(marker_metadata) = fs::metadata(&marker_path) else {
        return true;
    };

    let backup_removed = remove_recoverable_artifact_suppressed_by_clear_marker(
        &options.backup_file_path(),
        "session backup",
        &marker_metadata,
    );
    let recovery_removed = remove_recoverable_artifact_suppressed_by_clear_marker(
        &options.recovery_file_path(),
        "session recovery",
        &marker_metadata,
    );
    backup_removed && recovery_removed
}

fn remove_recoverable_artifact_suppressed_by_clear_marker(
    path: &Path,
    label: &str,
    marker_metadata: &fs::Metadata,
) -> bool {
    let Ok(artifact_metadata) = fs::metadata(path) else {
        return true;
    };
    if artifact_is_newer_than_marker(&artifact_metadata, marker_metadata) {
        return true;
    }

    match fs::remove_file(path) {
        Ok(()) => {
            info!(
                "Removed stale {} {} before removing session clear marker",
                label,
                path.display()
            );
            true
        }
        Err(err) => {
            warn!(
                "Failed to remove stale {} {} before removing session clear marker: {}",
                label,
                path.display(),
                err
            );
            false
        }
    }
}

fn artifact_is_newer_than_marker(
    artifact_metadata: &fs::Metadata,
    marker_metadata: &fs::Metadata,
) -> bool {
    match (artifact_metadata.modified(), marker_metadata.modified()) {
        (Ok(artifact_modified), Ok(marker_modified)) => artifact_modified > marker_modified,
        _ => false,
    }
}

pub(super) fn write_backup_recovery_marker(options: &SessionOptions) -> Result<()> {
    let marker_path = options.backup_recovery_marker_file_path();
    let tmp_path = temp_path(&marker_path)?;
    {
        let mut tmp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .with_context(|| {
                format!(
                    "failed to open temporary backup recovery marker {}",
                    tmp_path.display()
                )
            })?;
        tmp_file
            .write_all(now_rfc3339().as_bytes())
            .context("failed to write backup recovery marker")?;
        tmp_file
            .sync_all()
            .context("failed to sync backup recovery marker")?;
    }
    fs::rename(&tmp_path, &marker_path).with_context(|| {
        format!(
            "failed to move temporary backup recovery marker {} -> {}",
            tmp_path.display(),
            marker_path.display()
        )
    })?;
    info!(
        "Wrote backup recovery marker {} for contentless non-clear session save",
        marker_path.display()
    );
    Ok(())
}

pub(super) fn write_recovery_recoverable_marker(options: &SessionOptions) -> Result<()> {
    let marker_path = options.recovery_recoverable_marker_file_path();
    let tmp_path = temp_path(&marker_path)?;
    {
        let mut tmp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .with_context(|| {
                format!(
                    "failed to open temporary recovery recoverable marker {}",
                    tmp_path.display()
                )
            })?;
        tmp_file
            .write_all(now_rfc3339().as_bytes())
            .context("failed to write recovery recoverable marker")?;
        tmp_file
            .sync_all()
            .context("failed to sync recovery recoverable marker")?;
    }
    fs::rename(&tmp_path, &marker_path).with_context(|| {
        format!(
            "failed to move temporary recovery recoverable marker {} -> {}",
            tmp_path.display(),
            marker_path.display()
        )
    })?;
    info!(
        "Wrote recovery recoverable marker {} for contentless non-clear session save",
        marker_path.display()
    );
    Ok(())
}

pub(super) fn write_clear_marker(options: &SessionOptions) -> Result<()> {
    let marker_path = options.clear_marker_file_path();
    let tmp_path = temp_path(&marker_path)?;
    {
        let mut tmp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .with_context(|| {
                format!(
                    "failed to open temporary session clear marker {}",
                    tmp_path.display()
                )
            })?;
        tmp_file
            .write_all(now_rfc3339().as_bytes())
            .context("failed to write session clear marker")?;
        tmp_file
            .sync_all()
            .context("failed to sync session clear marker")?;
    }
    fs::rename(&tmp_path, &marker_path).with_context(|| {
        format!(
            "failed to move temporary session clear marker {} -> {}",
            tmp_path.display(),
            marker_path.display()
        )
    })?;
    info!(
        "Wrote session clear marker {} for empty saved session",
        marker_path.display()
    );
    Ok(())
}

pub(super) fn remove_clear_marker_file(options: &SessionOptions) {
    let marker_path = options.clear_marker_file_path();
    if !marker_path.exists() {
        return;
    }
    match fs::remove_file(&marker_path) {
        Ok(()) => info!(
            "Removed session clear marker after successful contentful save: {}",
            marker_path.display()
        ),
        Err(err) => warn!(
            "Failed to remove session clear marker {} after successful contentful save: {}",
            marker_path.display(),
            err
        ),
    }
}

pub(super) fn remove_backup_file(options: &SessionOptions) {
    let backup_path = options.backup_file_path();
    if !backup_path.exists() {
        return;
    }
    match fs::remove_file(&backup_path) {
        Ok(()) => info!(
            "Removed session backup after intentional empty clear: {}",
            backup_path.display()
        ),
        Err(err) => warn!(
            "Failed to remove session backup {} after intentional empty clear: {}",
            backup_path.display(),
            err
        ),
    }
}

pub(super) fn remove_backup_recovery_marker_file(options: &SessionOptions) {
    let marker_path = options.backup_recovery_marker_file_path();
    if !marker_path.exists() {
        return;
    }
    match fs::remove_file(&marker_path) {
        Ok(()) => info!("Removed backup recovery marker: {}", marker_path.display()),
        Err(err) => warn!(
            "Failed to remove backup recovery marker {}: {}",
            marker_path.display(),
            err
        ),
    }
}

pub(super) fn remove_recovery_recoverable_marker_file(options: &SessionOptions) {
    let marker_path = options.recovery_recoverable_marker_file_path();
    if !marker_path.exists() {
        return;
    }
    match fs::remove_file(&marker_path) {
        Ok(()) => info!(
            "Removed recovery recoverable marker: {}",
            marker_path.display()
        ),
        Err(err) => warn!(
            "Failed to remove recovery recoverable marker {}: {}",
            marker_path.display(),
            err
        ),
    }
}

pub(super) fn remove_recovery_files(options: &SessionOptions) {
    let recovery_path = options.recovery_file_path();
    let Some(recovery_name) = recovery_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
    else {
        remove_recovery_file(options);
        return;
    };
    let Some(parent) = recovery_path.parent() else {
        remove_recovery_file(options);
        return;
    };

    let mut removed_any = false;
    match fs::read_dir(parent) {
        Ok(entries) => {
            for entry in entries {
                let Ok(entry) = entry else {
                    continue;
                };
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                if name != recovery_name && !name.starts_with(&format!("{recovery_name}.")) {
                    continue;
                }
                match fs::remove_file(&path) {
                    Ok(()) => {
                        removed_any = true;
                        info!(
                            "Removed session recovery artifact after intentional empty clear: {}",
                            path.display()
                        );
                    }
                    Err(err) => warn!(
                        "Failed to remove session recovery artifact {} after intentional empty clear: {}",
                        path.display(),
                        err
                    ),
                }
            }
        }
        Err(err) => warn!(
            "Failed to scan session recovery artifacts under {} after intentional empty clear: {}",
            parent.display(),
            err
        ),
    }

    if !removed_any {
        debug!(
            "No session recovery artifact present after intentional empty clear: {}",
            recovery_path.display()
        );
    }
}

pub(super) fn remove_recovery_file(options: &SessionOptions) {
    let recovery_path = options.recovery_file_path();
    if !recovery_path.exists() {
        return;
    }
    match fs::remove_file(&recovery_path) {
        Ok(()) => info!(
            "Removed session recovery artifact after successful normal save: {}",
            recovery_path.display()
        ),
        Err(err) => warn!(
            "Failed to remove session recovery artifact {} after successful normal save: {}",
            recovery_path.display(),
            err
        ),
    }
}
