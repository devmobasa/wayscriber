use super::compression::{DEFAULT_MAX_EXPANDED_SESSION_BYTES, compress_bytes, temp_path};
use super::types::{
    BoardFile, BoardPagesSnapshot, BoardSnapshot, CURRENT_VERSION, SessionFile, SessionSnapshot,
};
use crate::session::lock::{lock_exclusive, open_runtime_lock_file, unlock};
use crate::session::options::{CompressionMode, SessionOptions};
use crate::time_utils::now_rfc3339;
use anyhow::{Context, Result, anyhow};
use log::{debug, info, warn};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[allow(dead_code)]
const AUTOSAVE_HISTORY_FALLBACK_DEPTH: usize = 1;

mod payload;
mod recovery;
mod save_as;

use payload::{
    PayloadCandidate, estimate_from_candidate, payload_candidate, payload_within_limit,
    snapshot_without_history,
};
use recovery::{
    remove_backup_file, remove_backup_recovery_marker_file, remove_clear_marker_file,
    remove_recoverable_artifacts_suppressed_by_clear_marker, remove_recovery_file,
    remove_recovery_files, remove_recovery_recoverable_marker_file,
    remove_session_file_after_clear_marker, save_recovery_snapshot, write_backup_recovery_marker,
    write_clear_marker, write_recovery_recoverable_marker,
};
pub(crate) use save_as::{save_snapshot_as_requires_overwrite, save_snapshot_as_with_report};

mod model;

use model::{HistoryFallbackStrategy, SavePayloadTooLarge, log_near_limit};
pub use model::{
    SaveAsOverwrite, SaveLimitExceeded, SaveSnapshotOutcome, SaveSnapshotReport,
    SnapshotPayloadEstimate, SnapshotSaveEstimate,
};

/// Persist the provided snapshot to disk according to the configured options.
#[allow(dead_code)]
pub fn save_snapshot(snapshot: &SessionSnapshot, options: &SessionOptions) -> Result<()> {
    save_snapshot_with_report(snapshot, options).map(|_| ())
}

/// Persist the provided snapshot and report what was written.
#[allow(dead_code)]
pub(crate) fn save_snapshot_with_report(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
) -> Result<Option<SaveSnapshotReport>> {
    save_snapshot_with_report_and_clear_boundary(snapshot, options, false)
}

pub(crate) fn save_snapshot_with_report_and_clear_boundary(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    contentless_clear_boundary: bool,
) -> Result<Option<SaveSnapshotReport>> {
    save_snapshot_with_expanded_limit_and_strategy(
        snapshot,
        options,
        DEFAULT_MAX_EXPANDED_SESSION_BYTES,
        HistoryFallbackStrategy::LargestFitting,
        contentless_clear_boundary,
    )
}

#[allow(dead_code)]
pub(crate) fn save_snapshot_autosave_with_report(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
) -> Result<Option<SaveSnapshotReport>> {
    save_snapshot_autosave_with_report_and_clear_boundary(snapshot, options, false)
}

pub(crate) fn save_snapshot_autosave_with_report_and_clear_boundary(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    contentless_clear_boundary: bool,
) -> Result<Option<SaveSnapshotReport>> {
    save_snapshot_with_expanded_limit_and_strategy(
        snapshot,
        options,
        DEFAULT_MAX_EXPANDED_SESSION_BYTES,
        HistoryFallbackStrategy::Bounded {
            max_depth: AUTOSAVE_HISTORY_FALLBACK_DEPTH,
        },
        contentless_clear_boundary,
    )
}

#[allow(dead_code)]
pub(crate) fn estimate_snapshot_save(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
) -> Result<SnapshotSaveEstimate> {
    estimate_snapshot_save_with_expanded_limit(
        snapshot,
        options,
        DEFAULT_MAX_EXPANDED_SESSION_BYTES,
    )
}

#[allow(dead_code)]
pub(super) fn estimate_snapshot_save_with_expanded_limit(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<SnapshotSaveEstimate> {
    let last_modified = now_rfc3339();
    let full = payload_candidate(snapshot, options, &last_modified)?;
    let visible_only = snapshot_without_history(snapshot);
    let visible_without_history = payload_candidate(&visible_only, options, &last_modified)?;

    Ok(SnapshotSaveEstimate {
        full: estimate_from_candidate(&full, options, max_expanded_size),
        visible_without_history: estimate_from_candidate(
            &visible_without_history,
            options,
            max_expanded_size,
        ),
    })
}

#[allow(dead_code)]
pub(crate) fn estimate_snapshot_payload(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
) -> Result<SnapshotPayloadEstimate> {
    estimate_snapshot_payload_with_expanded_limit(
        snapshot,
        options,
        DEFAULT_MAX_EXPANDED_SESSION_BYTES,
    )
}

#[allow(dead_code)]
pub(crate) fn estimate_snapshot_without_history_payload(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
) -> Result<SnapshotPayloadEstimate> {
    let visible_only = snapshot_without_history(snapshot);
    estimate_snapshot_payload(&visible_only, options)
}

#[allow(dead_code)]
pub(super) fn estimate_snapshot_payload_with_expanded_limit(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<SnapshotPayloadEstimate> {
    let last_modified = now_rfc3339();
    let candidate = payload_candidate(snapshot, options, &last_modified)?;
    Ok(estimate_from_candidate(
        &candidate,
        options,
        max_expanded_size,
    ))
}

#[allow(dead_code)]
pub(super) fn save_snapshot_with_expanded_limit(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<Option<SaveSnapshotReport>> {
    save_snapshot_with_expanded_limit_and_strategy(
        snapshot,
        options,
        max_expanded_size,
        HistoryFallbackStrategy::LargestFitting,
        false,
    )
}

fn save_snapshot_with_expanded_limit_and_strategy(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    max_expanded_size: u64,
    history_fallback: HistoryFallbackStrategy,
    contentless_clear_boundary: bool,
) -> Result<Option<SaveSnapshotReport>> {
    if !options.any_enabled() && !options.persist_history && snapshot.tool_state.is_none() {
        debug!("Session persistence disabled for all boards; skipping save");
        return Ok(None);
    }

    prepare_session_parent_for_save(options)?;

    let lock_path = options.lock_file_path();
    let lock_file = open_runtime_lock_file(&lock_path, options.is_named_file())
        .with_context(|| format!("failed to open session lock file {}", lock_path.display()))?;
    let lock_started = Instant::now();
    lock_exclusive(&lock_file)
        .with_context(|| format!("failed to lock session file {}", lock_path.display()))?;
    info!(
        "Acquired session save lock {} in {:?}",
        lock_path.display(),
        lock_started.elapsed()
    );

    let save_started = Instant::now();
    let result = save_snapshot_inner(
        snapshot,
        options,
        max_expanded_size,
        history_fallback,
        contentless_clear_boundary,
    );
    match &result {
        Ok(Some(report)) => info!(
            "Session save pipeline finished for {} in {:?}: outcome={:?}, written={} bytes, raw={} bytes, compression={}",
            report.path.display(),
            save_started.elapsed(),
            report.outcome,
            report.written_size,
            report.raw_size,
            report.compressed
        ),
        Ok(None) => info!(
            "Session save pipeline finished in {:?}: no file write needed",
            save_started.elapsed()
        ),
        Err(err) => warn!(
            "Session save pipeline failed for {} after {:?}: {}",
            options.session_file_path().display(),
            save_started.elapsed(),
            err
        ),
    }

    if let Err(err) = unlock(&lock_file) {
        warn!(
            "failed to unlock session file {}: {}",
            lock_path.display(),
            err
        );
    }

    if matches!(&result, Ok(Some(_))) {
        crate::session::catalog::record_named_session_saved(options);
    }

    result
}

fn prepare_session_parent_for_save(options: &SessionOptions) -> Result<()> {
    if options.is_named_file() {
        crate::session::validate_named_session_file_for_foreground(&options.session_file_path())?;
        return Ok(());
    }

    fs::create_dir_all(&options.base_dir).with_context(|| {
        format!(
            "failed to create session directory {}",
            options.base_dir.display()
        )
    })?;
    Ok(())
}

fn save_snapshot_inner(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    max_expanded_size: u64,
    history_fallback: HistoryFallbackStrategy,
    contentless_clear_boundary: bool,
) -> Result<Option<SaveSnapshotReport>> {
    let session_path = options.session_file_path();
    let backup_path = options.backup_file_path();
    let backup_recovery_marker_path = options.backup_recovery_marker_file_path();
    let recovery_path = options.recovery_file_path();
    let recovery_recoverable_marker_path = options.recovery_recoverable_marker_file_path();
    let had_session_file = session_path.exists();
    let snapshot_has_board_data = snapshot.has_board_data();
    let contentless_clear_boundary = !snapshot_has_board_data && contentless_clear_boundary;
    let last_modified = now_rfc3339();

    let prepare_started = Instant::now();
    let prepared = match payload_within_limit(
        snapshot,
        options,
        &last_modified,
        max_expanded_size,
        history_fallback,
    ) {
        Ok(prepared) => prepared,
        Err(err) => {
            if session_path.exists() {
                warn!(
                    "Session save failed before replacing {}; existing session file is unchanged",
                    session_path.display()
                );
            }
            if err.downcast_ref::<SavePayloadTooLarge>().is_some()
                && matches!(history_fallback, HistoryFallbackStrategy::LargestFitting)
            {
                match save_recovery_snapshot(snapshot, options, max_expanded_size, &last_modified) {
                    Ok(Some(report)) => warn!(
                        "Wrote oversized session recovery artifact to {} ({} bytes written, raw={} bytes, compression={}, outcome={:?})",
                        report.path.display(),
                        report.written_size,
                        report.raw_size,
                        report.compressed,
                        report.outcome
                    ),
                    Ok(None) => {}
                    Err(recovery_err) => warn!(
                        "Failed to write oversized session recovery artifact {}: {}",
                        options.recovery_file_path().display(),
                        recovery_err
                    ),
                }
            }
            return Err(err);
        }
    };
    let prepared_written_size = prepared
        .payload
        .as_ref()
        .map_or(0, PayloadCandidate::final_size);
    info!(
        "Prepared session payload for {} in {:?}: outcome={:?}, written={} bytes, raw={} bytes, compression={}",
        session_path.display(),
        prepare_started.elapsed(),
        prepared.outcome,
        prepared_written_size,
        prepared.raw_size,
        prepared.compressed
    );

    let Some(payload) = prepared.payload else {
        let report = SaveSnapshotReport {
            path: session_path.clone(),
            outcome: prepared.outcome,
            raw_size: prepared.raw_size,
            written_size: 0,
            max_file_size_bytes: options.max_file_size_bytes,
            compressed: prepared.compressed,
        };
        if matches!(prepared.outcome, SaveSnapshotOutcome::ClearedEmpty) {
            write_clear_marker(options)?;
            remove_session_file_after_clear_marker(&session_path);
            remove_backup_file(options);
            remove_backup_recovery_marker_file(options);
            remove_recovery_files(options);
            remove_recovery_recoverable_marker_file(options);
        }
        log_near_limit(&report);
        return Ok(Some(report));
    };
    let PayloadCandidate {
        bytes: payload_bytes,
        raw_size,
        compressed,
    } = payload;
    let final_size = payload_bytes.len();

    let tmp_path = temp_path(&session_path)?;
    let write_started = Instant::now();
    {
        let mut tmp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .with_context(|| {
                format!(
                    "failed to open temporary session file {}",
                    tmp_path.display()
                )
            })?;
        tmp_file
            .write_all(&payload_bytes)
            .context("failed to write session payload")?;
        tmp_file
            .sync_all()
            .context("failed to sync temporary session file")?;
    }
    let write_elapsed = write_started.elapsed();

    if contentless_clear_boundary {
        write_clear_marker(options)?;
    }

    let replace_started = Instant::now();
    let preserves_recoverable_backup =
        !snapshot_has_board_data && !contentless_clear_boundary && backup_path.exists();
    let preserves_recoverable_recovery = !snapshot_has_board_data
        && !contentless_clear_boundary
        && recovery_path.exists()
        && (!had_session_file || recovery_recoverable_marker_path.exists());
    let preserve_existing_backup = preserves_recoverable_backup
        && backup_recovery_marker_path.exists()
        && session_path.exists();
    let mut should_mark_backup_recoverable = false;
    let should_mark_recovery_recoverable = preserves_recoverable_recovery;
    if session_path.exists() {
        if preserve_existing_backup {
            fs::remove_file(&session_path).with_context(|| {
                format!(
                    "failed to remove previous contentless session file {} while preserving backup {}",
                    session_path.display(),
                    backup_path.display()
                )
            })?;
            should_mark_backup_recoverable = true;
        } else if options.backup_retention > 0 {
            if backup_path.exists() {
                fs::remove_file(&backup_path).ok();
            }
            fs::rename(&session_path, &backup_path).with_context(|| {
                format!(
                    "failed to rotate previous session file {} -> {}",
                    session_path.display(),
                    backup_path.display()
                )
            })?;
            if !snapshot_has_board_data && !contentless_clear_boundary {
                should_mark_backup_recoverable = true;
            }
        } else {
            fs::remove_file(&session_path).ok();
        }
    } else if preserves_recoverable_backup {
        should_mark_backup_recoverable = true;
    }

    if should_mark_backup_recoverable {
        write_backup_recovery_marker(options)?;
    }
    if should_mark_recovery_recoverable {
        write_recovery_recoverable_marker(options)?;
    }

    fs::rename(&tmp_path, &session_path).with_context(|| {
        format!(
            "failed to move temporary session file {} -> {}",
            tmp_path.display(),
            session_path.display()
        )
    })?;
    let replace_elapsed = replace_started.elapsed();
    info!(
        "Session file replace completed for {}: write_and_sync={:?}, rotate_and_rename={:?}, final_size={} bytes",
        session_path.display(),
        write_elapsed,
        replace_elapsed,
        final_size
    );

    info!(
        "Session saved to {} ({} bytes written, raw={} bytes, compression={}, outcome={:?})",
        session_path.display(),
        final_size,
        raw_size,
        compressed,
        prepared.outcome
    );

    let report = SaveSnapshotReport {
        path: session_path,
        outcome: prepared.outcome,
        raw_size,
        written_size: final_size,
        max_file_size_bytes: options.max_file_size_bytes,
        compressed,
    };
    log_near_limit(&report);
    if snapshot_has_board_data {
        if remove_recoverable_artifacts_suppressed_by_clear_marker(options) {
            remove_clear_marker_file(options);
        } else {
            warn!(
                "Keeping session clear marker {} because a stale recoverable artifact could not be removed",
                options.clear_marker_file_path().display()
            );
        }
        remove_backup_recovery_marker_file(options);
        remove_recovery_recoverable_marker_file(options);
        remove_recovery_file(options);
    } else if contentless_clear_boundary {
        remove_backup_file(options);
        remove_backup_recovery_marker_file(options);
        remove_recovery_files(options);
        remove_recovery_recoverable_marker_file(options);
    } else if had_session_file && !preserves_recoverable_recovery {
        remove_recovery_file(options);
        remove_recovery_recoverable_marker_file(options);
    }
    Ok(Some(report))
}
