use super::*;
use std::os::unix::fs::OpenOptionsExt;

mod transaction;

use transaction::commit_save_as_target;

#[allow(dead_code)]
pub(crate) fn save_snapshot_as_with_report(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    overwrite: SaveAsOverwrite,
) -> Result<SaveSnapshotReport> {
    if !options.is_named_file() {
        return Err(anyhow!(
            "Save Session As requires a named session file target"
        ));
    }
    if !options.any_enabled() && !options.persist_history && snapshot.tool_state.is_none() {
        return Err(anyhow!(
            "Save Session As has no enabled session data to write"
        ));
    }

    let session_path = options.session_file_path();
    crate::session::validate_named_session_file_for_foreground(&session_path)?;
    let initial_artifacts = collect_save_as_artifacts(options)?;
    ensure_save_as_overwrite_allowed(&initial_artifacts, overwrite, &session_path)?;

    let last_modified = now_rfc3339();
    let prepare_started = Instant::now();
    let prepared = payload_within_limit(
        snapshot,
        options,
        &last_modified,
        DEFAULT_MAX_EXPANDED_SESSION_BYTES,
        HistoryFallbackStrategy::LargestFitting,
    )?;
    let Some(payload) = prepared.payload else {
        return Err(anyhow!(
            "Save Session As produced no primary session payload for {}",
            session_path.display()
        ));
    };
    let PayloadCandidate {
        bytes: payload_bytes,
        raw_size,
        compressed,
    } = payload;
    let final_size = payload_bytes.len();
    info!(
        "Prepared Save Session As payload for {} in {:?}: outcome={:?}, written={} bytes, raw={} bytes, compression={}",
        session_path.display(),
        prepare_started.elapsed(),
        prepared.outcome,
        final_size,
        raw_size,
        compressed
    );

    let tmp_path = temp_path(&session_path)?;
    let write_started = Instant::now();
    let mut tmp_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        // Same 0600 as the configured save: a named session holds the same
        // drawings, and the umask default made them world-readable.
        .mode(0o600)
        .open(&tmp_path)
        .with_context(|| {
            format!(
                "failed to open temporary Save Session As file {}",
                tmp_path.display()
            )
        })?;
    let write_result = (|| {
        tmp_file
            .write_all(&payload_bytes)
            .context("failed to write Save Session As payload")?;
        tmp_file
            .sync_all()
            .context("failed to sync temporary Save Session As file")?;
        Ok::<(), anyhow::Error>(())
    })();
    drop(tmp_file);
    if let Err(err) = write_result {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }
    let write_elapsed = write_started.elapsed();

    let lock_path = options.lock_file_path();
    let lock_file = match open_runtime_lock_file(&lock_path, true) {
        Ok(file) => file,
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            return Err(err).with_context(|| {
                format!(
                    "failed to open Save Session As lock file {}",
                    lock_path.display()
                )
            });
        }
    };
    let lock_started = Instant::now();
    if let Err(err) = lock_exclusive(&lock_file) {
        let _ = fs::remove_file(&tmp_path);
        return Err(err).with_context(|| {
            format!(
                "failed to lock Save Session As target {}",
                lock_path.display()
            )
        });
    }
    info!(
        "Acquired Save Session As lock {} in {:?}",
        lock_path.display(),
        lock_started.elapsed()
    );

    let commit_started = Instant::now();
    let commit_result = (|| {
        crate::session::validate_named_session_file_for_foreground(&session_path)?;
        let lock_time_artifacts = collect_save_as_artifacts(options)?;
        ensure_save_as_overwrite_allowed(&lock_time_artifacts, overwrite, &session_path)?;
        let sidecars = matches!(overwrite, SaveAsOverwrite::ConfirmReplace)
            .then_some(lock_time_artifacts.sidecars.as_slice())
            .unwrap_or_default();
        commit_save_as_target(&tmp_path, &session_path, sidecars)?;
        Ok::<(), anyhow::Error>(())
    })();

    if let Err(err) = unlock(&lock_file) {
        warn!(
            "failed to unlock Save Session As target {}: {}",
            lock_path.display(),
            err
        );
    }

    if let Err(err) = commit_result {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }

    let report = SaveSnapshotReport {
        path: session_path,
        outcome: prepared.outcome,
        raw_size,
        written_size: final_size,
        max_file_size_bytes: options.max_file_size_bytes,
        compressed,
    };
    log_near_limit(&report);
    info!(
        "Save Session As committed to {}: write_and_sync={:?}, cleanup_and_rename={:?}, final_size={} bytes",
        report.path.display(),
        write_elapsed,
        commit_started.elapsed(),
        final_size
    );
    Ok(report)
}

#[allow(dead_code)]
pub(crate) fn save_snapshot_as_requires_overwrite(options: &SessionOptions) -> Result<bool> {
    if !options.is_named_file() {
        return Err(anyhow!(
            "Save Session As requires a named session file target"
        ));
    }

    let session_path = options.session_file_path();
    crate::session::validate_named_session_file_for_foreground(&session_path)?;
    Ok(collect_save_as_artifacts(options)?.has_any())
}

struct SaveAsArtifactSet {
    primary_exists: bool,
    sidecars: Vec<PathBuf>,
}

impl SaveAsArtifactSet {
    fn has_any(&self) -> bool {
        self.primary_exists || !self.sidecars.is_empty()
    }
}

fn collect_save_as_artifacts(options: &SessionOptions) -> Result<SaveAsArtifactSet> {
    let session_path = options.session_file_path();
    let primary_exists = artifact_path_exists(&session_path)?;
    let mut sidecars = Vec::new();
    for path in save_as_non_lock_sidecar_paths(options)? {
        if artifact_path_exists(&path)? {
            sidecars.push(path);
        }
    }
    Ok(SaveAsArtifactSet {
        primary_exists,
        sidecars,
    })
}

fn ensure_save_as_overwrite_allowed(
    artifacts: &SaveAsArtifactSet,
    overwrite: SaveAsOverwrite,
    session_path: &Path,
) -> Result<()> {
    if artifacts.has_any() && matches!(overwrite, SaveAsOverwrite::Deny) {
        return Err(anyhow!(
            "Save Session As target already has session artifacts; overwrite confirmation required for {}",
            session_path.display()
        ));
    }
    Ok(())
}

fn artifact_path_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err)
            .with_context(|| format!("failed to inspect session artifact {}", path.display())),
    }
}

fn save_as_non_lock_sidecar_paths(options: &SessionOptions) -> Result<Vec<PathBuf>> {
    let session_path = options.session_file_path();
    let mut paths = crate::session::named_session_non_lock_artifact_paths(&session_path)?;
    paths.retain(|path| path != &session_path);
    Ok(paths)
}
