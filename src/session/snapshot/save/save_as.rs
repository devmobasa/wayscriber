use super::*;

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
        let removed_sidecars = matches!(overwrite, SaveAsOverwrite::ConfirmReplace)
            && !lock_time_artifacts.sidecars.is_empty();
        if matches!(overwrite, SaveAsOverwrite::ConfirmReplace) {
            remove_save_as_sidecars(&lock_time_artifacts.sidecars)?;
        }
        if let Err(err) = fs::rename(&tmp_path, &session_path) {
            let context = if removed_sidecars {
                format!(
                    "partial destructive Save Session As failure for {}: stale sidecars were removed but failed to move temporary file {} into place",
                    session_path.display(),
                    tmp_path.display()
                )
            } else {
                format!(
                    "failed to move temporary Save Session As file {} -> {}",
                    tmp_path.display(),
                    session_path.display()
                )
            };
            return Err(err).with_context(|| context);
        }
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
    let mut paths = vec![
        options.backup_file_path(),
        options.backup_recovery_marker_file_path(),
        options.recovery_file_path(),
        options.recovery_recoverable_marker_file_path(),
        options.clear_marker_file_path(),
    ];

    let recovery_path = options.recovery_file_path();
    let Some(recovery_name) = recovery_path.file_name().and_then(|name| name.to_str()) else {
        dedupe_paths(&mut paths);
        return Ok(paths);
    };
    let Some(parent) = recovery_path.parent() else {
        dedupe_paths(&mut paths);
        return Ok(paths);
    };
    match fs::read_dir(parent) {
        Ok(entries) => {
            let preserved_prefix = format!("{recovery_name}.");
            for entry in entries {
                let entry = entry.with_context(|| {
                    format!(
                        "failed to inspect Save Session As sidecars under {}",
                        parent.display()
                    )
                })?;
                let path = entry.path();
                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                if name == recovery_name || name.starts_with(&preserved_prefix) {
                    paths.push(path);
                }
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err).with_context(|| {
                format!(
                    "failed to scan Save Session As sidecars under {}",
                    parent.display()
                )
            });
        }
    }

    dedupe_paths(&mut paths);
    Ok(paths)
}

fn dedupe_paths(paths: &mut Vec<PathBuf>) {
    let mut deduped = Vec::with_capacity(paths.len());
    for path in paths.drain(..) {
        if !deduped.contains(&path) {
            deduped.push(path);
        }
    }
    *paths = deduped;
}

fn remove_save_as_sidecars(sidecars: &[PathBuf]) -> Result<()> {
    for path in sidecars {
        match fs::remove_file(path) {
            Ok(()) => info!(
                "Removed stale Save Session As sidecar before commit: {}",
                path.display()
            ),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "failed to remove stale Save Session As sidecar {}",
                        path.display()
                    )
                });
            }
        }
    }
    Ok(())
}
