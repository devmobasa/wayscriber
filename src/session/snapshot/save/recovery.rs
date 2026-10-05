use super::payload::{log_payload_candidate, snapshot_without_history};
use super::*;
use crate::session::snapshot::generation::{ArtifactStamp, MarkerKind, cleared_by};

pub(super) fn save_recovery_snapshot(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    max_expanded_size: u64,
    stamp: payload::PayloadStamp<'_>,
) -> Result<Option<SaveSnapshotReport>> {
    let Some((payload, outcome)) = recovery_payload(snapshot, options, max_expanded_size, stamp)?
    else {
        return Ok(None);
    };

    let recovery_path = options.recovery_file_path();
    let write_started = Instant::now();
    // Through durable_io: it owns the temp-write/fsync/rename/parent-sync
    // sequence, keeps the artifact at 0600 like the rest of the session's
    // private data, and removes its temporary file on every failure path.
    crate::durable_io::write_atomic(
        &recovery_path,
        &payload.bytes,
        crate::durable_io::AtomicWriteOptions {
            overwrite: crate::durable_io::OverwriteMode::Replace,
            permissions: crate::durable_io::PermissionPolicy::FixedMode(0o600),
            symlink: crate::durable_io::SymlinkPolicy::Reject,
            sync_file: true,
            sync_parent: true,
        },
    )
    .with_context(|| {
        format!(
            "failed to write session recovery file {}",
            recovery_path.display()
        )
    })?;
    info!(
        "Session recovery file replace completed for {}: elapsed={:?}, final_size={} bytes",
        recovery_path.display(),
        write_started.elapsed(),
        payload.final_size()
    );

    Ok(Some(SaveSnapshotReport {
        generation: stamp.generation,
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
    stamp: payload::PayloadStamp<'_>,
) -> Result<Option<(PayloadCandidate, SaveSnapshotOutcome)>> {
    if snapshot.is_empty() && snapshot.tool_state.is_none() {
        return Ok(None);
    }

    let full_started = Instant::now();
    let full_payload = payload_candidate(snapshot, options, stamp)?;
    log_payload_candidate("recovery full", &full_payload, full_started.elapsed());
    let Some(full_limit) = full_payload.expanded_limit_exceeded(max_expanded_size) else {
        return Ok(Some((full_payload, SaveSnapshotOutcome::Full)));
    };
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
    let visible_payload = payload_candidate(&visible_only, options, stamp)?;
    log_payload_candidate(
        "recovery visible-only",
        &visible_payload,
        visible_started.elapsed(),
    );
    if let Some(visible_limit) = visible_payload.expanded_limit_exceeded(max_expanded_size) {
        return Err(anyhow!(
            "Session recovery data cannot be saved safely ({}; {} bytes written from {} raw bytes, compression={}); skipping recovery",
            visible_limit.description(),
            visible_payload.final_size(),
            visible_payload.raw_size,
            visible_payload.compressed
        ));
    }
    Ok(Some((visible_payload, SaveSnapshotOutcome::VisibleOnly)))
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
    !cleared_by(
        ArtifactStamp::without_generation(artifact_metadata),
        ArtifactStamp::without_generation(marker_metadata),
    )
}

/// Writes a marker file through `durable_io`, which owns the whole
/// temp-write/fsync/rename/parent-sync dance and removes its temporary file on
/// every failure path. Markers carry a format-1 generation record, or a legacy
/// timestamp at the counter ceiling. Loaders still use marker presence and
/// modification times, so marker durability remains part of the save contract.
fn write_session_marker(
    marker_path: &Path,
    kind: MarkerKind,
    generation: Option<u64>,
    label: &str,
) -> Result<()> {
    let written = now_rfc3339();
    let content = match generation {
        Some(g) => super::super::generation::marker_record_bytes(kind, g, written)?,
        None => written.into_bytes(),
    };
    crate::durable_io::write_atomic(
        marker_path,
        &content,
        crate::durable_io::AtomicWriteOptions {
            overwrite: crate::durable_io::OverwriteMode::Replace,
            permissions: crate::durable_io::PermissionPolicy::FixedMode(0o600),
            symlink: crate::durable_io::SymlinkPolicy::Reject,
            sync_file: true,
            sync_parent: true,
        },
    )
    .with_context(|| format!("failed to write {label} {}", marker_path.display()))
}

pub(super) fn write_backup_recovery_marker(
    options: &SessionOptions,
    generation: Option<u64>,
) -> Result<()> {
    let marker_path = options.backup_recovery_marker_file_path();
    write_session_marker(
        &marker_path,
        MarkerKind::BackupRecoverable,
        generation,
        "backup recovery marker",
    )?;
    info!(
        "Wrote backup recovery marker {} for contentless non-clear session save",
        marker_path.display()
    );
    Ok(())
}

pub(super) fn write_recovery_recoverable_marker(
    options: &SessionOptions,
    generation: Option<u64>,
) -> Result<()> {
    let marker_path = options.recovery_recoverable_marker_file_path();
    write_session_marker(
        &marker_path,
        MarkerKind::RecoveryRecoverable,
        generation,
        "recovery recoverable marker",
    )?;
    info!(
        "Wrote recovery recoverable marker {} for contentless non-clear session save",
        marker_path.display()
    );
    Ok(())
}

pub(super) fn write_clear_marker(options: &SessionOptions, generation: Option<u64>) -> Result<()> {
    let marker_path = options.clear_marker_file_path();
    write_session_marker(
        &marker_path,
        MarkerKind::Cleared,
        generation,
        "session clear marker",
    )?;
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

pub(super) fn preserve_oversized_recovery(
    error: &mut anyhow::Error,
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    max_expanded_size: u64,
    stamp: payload::PayloadStamp<'_>,
) {
    if error.downcast_ref::<SavePayloadTooLarge>().is_none() {
        return;
    }
    match save_recovery_snapshot(snapshot, options, max_expanded_size, stamp) {
        Ok(Some(report)) => {
            if let Some(limit) = error.downcast_mut::<SavePayloadTooLarge>() {
                limit.recovery_path = Some(report.path.clone());
            }
            warn!(
                "Wrote oversized session recovery artifact to {} ({} bytes written, raw={} bytes, compression={}, outcome={:?})",
                report.path.display(),
                report.written_size,
                report.raw_size,
                report.compressed,
                report.outcome
            );
        }
        Ok(None) => {}
        Err(recovery_err) => warn!(
            "Failed to write oversized session recovery artifact {}: {}",
            options.recovery_file_path().display(),
            recovery_err
        ),
    }
}
