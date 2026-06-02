use super::compression::{DEFAULT_MAX_EXPANDED_SESSION_BYTES, compress_bytes, temp_path};
use super::types::{
    BoardFile, BoardPagesSnapshot, BoardSnapshot, CURRENT_VERSION, SessionFile, SessionSnapshot,
};
use crate::session::lock::{lock_exclusive, open_runtime_lock_file, unlock};
use crate::session::options::{CompressionMode, SessionOptions};
use crate::time_utils::now_rfc3339;
use anyhow::{Context, Result, anyhow};
use log::{debug, info, warn};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const NEAR_LIMIT_PERCENT: u64 = 90;
#[allow(dead_code)]
const AUTOSAVE_HISTORY_FALLBACK_DEPTH: usize = 1;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HistoryFallbackStrategy {
    LargestFitting,
    Bounded { max_depth: usize },
}

/// Outcome of a session save after applying configured size fallbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveSnapshotOutcome {
    Full,
    TrimmedHistory { depth: usize },
    VisibleOnly,
    ClearedEmpty,
}

/// Details about a completed session save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveSnapshotReport {
    pub path: PathBuf,
    pub outcome: SaveSnapshotOutcome,
    pub raw_size: usize,
    pub written_size: usize,
    pub max_file_size_bytes: u64,
    pub compressed: bool,
}

impl SaveSnapshotReport {
    pub fn is_near_limit(&self) -> bool {
        is_near_limit(self.written_size as u64, self.max_file_size_bytes)
    }
}

/// Estimated session payload size using the same serialisation, compression, and limits as saves.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotPayloadEstimate {
    pub raw_size: usize,
    pub written_size: usize,
    pub max_file_size_bytes: u64,
    pub compressed: bool,
    pub limit_exceeded: Option<SaveLimitExceeded>,
}

#[allow(dead_code)]
impl SnapshotPayloadEstimate {
    pub fn is_near_limit(&self) -> bool {
        is_near_limit(self.written_size as u64, self.max_file_size_bytes)
    }
}

/// Estimated full and visible-only payloads for a snapshot.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotSaveEstimate {
    pub full: SnapshotPayloadEstimate,
    pub visible_without_history: SnapshotPayloadEstimate,
}

/// The save or restore safety limit exceeded by a prepared payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveLimitExceeded {
    WrittenSize {
        written_size: u64,
        max_file_size: u64,
    },
    ExpandedSize {
        raw_size: u64,
        max_expanded_size: u64,
    },
}

impl SaveLimitExceeded {
    fn description(self) -> String {
        match self {
            Self::WrittenSize {
                written_size,
                max_file_size,
            } => format!(
                "writes as {written_size} bytes and exceeds the configured limit of {max_file_size} bytes"
            ),
            Self::ExpandedSize {
                raw_size,
                max_expanded_size,
            } => format!(
                "would expand to {raw_size} raw bytes, exceeding the load safety limit of {max_expanded_size} bytes"
            ),
        }
    }
}

#[derive(Debug)]
struct SavePayloadTooLarge {
    limit: SaveLimitExceeded,
    written_size: usize,
    raw_size: usize,
    compressed: bool,
}

impl fmt::Display for SavePayloadTooLarge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Session data cannot be saved safely ({}; {} bytes written from {} raw bytes, compression={}); skipping save",
            self.limit.description(),
            self.written_size,
            self.raw_size,
            self.compressed
        )
    }
}

impl std::error::Error for SavePayloadTooLarge {}

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
            remove_session_file(&session_path)?;
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

fn log_near_limit(report: &SaveSnapshotReport) {
    if report.is_near_limit() {
        debug!(
            "Session save size is near the configured limit ({} of {} bytes, threshold={}%)",
            report.written_size, report.max_file_size_bytes, NEAR_LIMIT_PERCENT
        );
    }
}

fn save_recovery_snapshot(
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

struct PayloadCandidate {
    bytes: Vec<u8>,
    raw_size: usize,
    compressed: bool,
}

impl PayloadCandidate {
    fn final_size(&self) -> usize {
        self.bytes.len()
    }

    fn limit_exceeded(
        &self,
        options: &SessionOptions,
        max_expanded_size: u64,
    ) -> Option<SaveLimitExceeded> {
        if self.final_size() as u64 > options.max_file_size_bytes {
            return Some(SaveLimitExceeded::WrittenSize {
                written_size: self.final_size() as u64,
                max_file_size: options.max_file_size_bytes,
            });
        }
        if self.compressed && self.raw_size as u64 > max_expanded_size {
            return Some(SaveLimitExceeded::ExpandedSize {
                raw_size: self.raw_size as u64,
                max_expanded_size,
            });
        }
        None
    }

    fn fits_limit(&self, options: &SessionOptions, max_expanded_size: u64) -> bool {
        self.limit_exceeded(options, max_expanded_size).is_none()
    }

    fn expanded_limit_exceeded(&self, max_expanded_size: u64) -> Option<SaveLimitExceeded> {
        if self.raw_size as u64 > max_expanded_size {
            Some(SaveLimitExceeded::ExpandedSize {
                raw_size: self.raw_size as u64,
                max_expanded_size,
            })
        } else {
            None
        }
    }

    fn fits_expanded_limit(&self, max_expanded_size: u64) -> bool {
        self.expanded_limit_exceeded(max_expanded_size).is_none()
    }
}

struct PreparedPayload {
    payload: Option<PayloadCandidate>,
    outcome: SaveSnapshotOutcome,
    raw_size: usize,
    compressed: bool,
}

impl PreparedPayload {
    fn write(payload: PayloadCandidate, outcome: SaveSnapshotOutcome) -> Self {
        Self {
            raw_size: payload.raw_size,
            compressed: payload.compressed,
            payload: Some(payload),
            outcome,
        }
    }

    fn clear(raw_size: usize, compressed: bool) -> Self {
        Self {
            payload: None,
            outcome: SaveSnapshotOutcome::ClearedEmpty,
            raw_size,
            compressed,
        }
    }
}

fn payload_within_limit(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    last_modified: &str,
    max_expanded_size: u64,
    history_fallback: HistoryFallbackStrategy,
) -> Result<PreparedPayload> {
    if snapshot.is_empty() && snapshot.tool_state.is_none() {
        return Ok(PreparedPayload::clear(0, false));
    }

    let full_started = Instant::now();
    let full_payload = payload_candidate(snapshot, options, last_modified)?;
    log_payload_candidate("full", &full_payload, full_started.elapsed());
    if full_payload.fits_limit(options, max_expanded_size) {
        return Ok(PreparedPayload::write(
            full_payload,
            SaveSnapshotOutcome::Full,
        ));
    }

    let full_raw_size = full_payload.raw_size;
    let full_final_size = full_payload.final_size();
    let full_limit = full_payload
        .limit_exceeded(options, max_expanded_size)
        .expect("full payload should exceed a save/load limit");
    let visible_only = snapshot_without_history(snapshot);
    if visible_only.is_empty() && visible_only.tool_state.is_none() {
        warn!(
            "Full session payload cannot be saved safely ({}; {} bytes written from {} raw bytes, compression={}); dropping undo/redo history leaves no visible session data, clearing saved session",
            full_limit.description(),
            full_final_size,
            full_raw_size,
            full_payload.compressed
        );
        return Ok(PreparedPayload::clear(
            full_raw_size,
            full_payload.compressed,
        ));
    }

    let visible_started = Instant::now();
    let visible_payload = payload_candidate(&visible_only, options, last_modified)?;
    log_payload_candidate("visible-only", &visible_payload, visible_started.elapsed());
    if !visible_payload.fits_limit(options, max_expanded_size) {
        let visible_limit = visible_payload
            .limit_exceeded(options, max_expanded_size)
            .expect("visible payload should exceed a save/load limit");
        return Err(SavePayloadTooLarge {
            limit: visible_limit,
            written_size: visible_payload.final_size(),
            raw_size: visible_payload.raw_size,
            compressed: visible_payload.compressed,
        }
        .into());
    }

    let history_depth = max_history_depth(snapshot);
    if history_depth > 0 {
        let visible_near_limit = is_near_limit(
            visible_payload.final_size() as u64,
            options.max_file_size_bytes,
        );
        let depth_one_started = Instant::now();
        let depth_one_candidate = snapshot_with_history_depth(snapshot, 1);
        let depth_one_payload = payload_candidate(&depth_one_candidate, options, last_modified)?;
        log_payload_candidate(
            "history-depth 1",
            &depth_one_payload,
            depth_one_started.elapsed(),
        );

        if depth_one_payload.fits_limit(options, max_expanded_size) {
            let fitting_history = if history_depth == 1 || visible_near_limit {
                if visible_near_limit && history_depth > 1 {
                    warn!(
                        "Visible-only session payload is already near the configured limit ({} of {} bytes); keeping one history entry and skipping deeper history-depth scan",
                        visible_payload.final_size(),
                        options.max_file_size_bytes
                    );
                }
                Some((1, depth_one_payload))
            } else {
                fitting_history_payload(
                    snapshot,
                    history_depth,
                    options,
                    last_modified,
                    max_expanded_size,
                    history_fallback,
                )?
                .or(Some((1, depth_one_payload)))
            };

            if let Some((depth, payload)) = fitting_history {
                warn!(
                    "Full session payload cannot be saved safely ({}; {} bytes written from {} raw bytes, compression={}); saving recent {} undo/redo history entries per stack ({} bytes written from {} raw bytes, compression={})",
                    full_limit.description(),
                    full_final_size,
                    full_raw_size,
                    full_payload.compressed,
                    depth,
                    payload.final_size(),
                    payload.raw_size,
                    payload.compressed
                );
                return Ok(PreparedPayload::write(
                    payload,
                    SaveSnapshotOutcome::TrimmedHistory { depth },
                ));
            }
        } else {
            let depth_one_limit = depth_one_payload
                .limit_exceeded(options, max_expanded_size)
                .expect("depth-one payload should exceed a save/load limit");
            warn!(
                "Even one persisted history entry cannot be saved safely ({}; {} bytes written from {} raw bytes, compression={}); skipping history-depth scan and saving visible data only",
                depth_one_limit.description(),
                depth_one_payload.final_size(),
                depth_one_payload.raw_size,
                depth_one_payload.compressed
            );
        }
    }

    warn!(
        "Full session payload cannot be saved safely ({}; {} bytes written from {} raw bytes, compression={}); saving visible data without undo/redo history ({} bytes written from {} raw bytes, compression={})",
        full_limit.description(),
        full_final_size,
        full_raw_size,
        full_payload.compressed,
        visible_payload.final_size(),
        visible_payload.raw_size,
        visible_payload.compressed
    );
    Ok(PreparedPayload::write(
        visible_payload,
        SaveSnapshotOutcome::VisibleOnly,
    ))
}

fn fitting_history_payload(
    snapshot: &SessionSnapshot,
    history_depth: usize,
    options: &SessionOptions,
    last_modified: &str,
    max_expanded_size: u64,
    history_fallback: HistoryFallbackStrategy,
) -> Result<Option<(usize, PayloadCandidate)>> {
    match history_fallback {
        HistoryFallbackStrategy::LargestFitting => largest_fitting_history_payload(
            snapshot,
            2,
            history_depth,
            options,
            last_modified,
            max_expanded_size,
        ),
        HistoryFallbackStrategy::Bounded { max_depth } => {
            let max_depth = max_depth.min(history_depth);
            if max_depth < 2 {
                debug!(
                    "Autosave history fallback capped at depth {}; skipping deeper history-depth scan",
                    max_depth
                );
                return Ok(None);
            }

            largest_fitting_history_payload(
                snapshot,
                2,
                max_depth,
                options,
                last_modified,
                max_expanded_size,
            )
        }
    }
}

fn largest_fitting_history_payload(
    snapshot: &SessionSnapshot,
    min_depth: usize,
    max_depth: usize,
    options: &SessionOptions,
    last_modified: &str,
    max_expanded_size: u64,
) -> Result<Option<(usize, PayloadCandidate)>> {
    if min_depth > max_depth {
        return Ok(None);
    }

    let scan_started = Instant::now();
    let mut attempts = 0usize;
    for depth in (min_depth..=max_depth).rev() {
        attempts += 1;
        let candidate_started = Instant::now();
        let candidate = snapshot_with_history_depth(snapshot, depth);
        let payload = payload_candidate(&candidate, options, last_modified)?;
        debug!(
            "Prepared history-depth session payload candidate depth={} in {:?}: written={} bytes, raw={} bytes, compression={}",
            depth,
            candidate_started.elapsed(),
            payload.final_size(),
            payload.raw_size,
            payload.compressed
        );
        if payload.fits_limit(options, max_expanded_size) {
            info!(
                "History trim scan found fitting session payload at depth {} after {} candidate(s) in {:?}: written={} bytes, raw={} bytes, compression={}",
                depth,
                attempts,
                scan_started.elapsed(),
                payload.final_size(),
                payload.raw_size,
                payload.compressed
            );
            return Ok(Some((depth, payload)));
        }
    }

    warn!(
        "History trim scan found no fitting session payload after {} candidate(s) in {:?}",
        attempts,
        scan_started.elapsed()
    );
    Ok(None)
}

fn log_payload_candidate(label: &str, payload: &PayloadCandidate, elapsed: Duration) {
    info!(
        "Prepared {} session payload candidate in {:?}: written={} bytes, raw={} bytes, compression={}",
        label,
        elapsed,
        payload.final_size(),
        payload.raw_size,
        payload.compressed
    );
}

fn payload_candidate(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    last_modified: &str,
) -> Result<PayloadCandidate> {
    let raw_bytes = serialize_payload(snapshot, last_modified)?;
    let raw_size = raw_bytes.len();
    let compressed = should_compress_payload(raw_size, options);
    let bytes = if compressed {
        compress_bytes(&raw_bytes)?
    } else {
        raw_bytes
    };

    Ok(PayloadCandidate {
        bytes,
        raw_size,
        compressed,
    })
}

fn should_compress_payload(raw_size: usize, options: &SessionOptions) -> bool {
    match options.compression {
        CompressionMode::Off => false,
        CompressionMode::On => true,
        CompressionMode::Auto => raw_size as u64 >= options.auto_compress_threshold_bytes,
    }
}

fn serialize_payload(snapshot: &SessionSnapshot, last_modified: &str) -> Result<Vec<u8>> {
    let file_payload = SessionFile {
        version: CURRENT_VERSION,
        last_modified: last_modified.to_string(),
        active_board_id: Some(snapshot.active_board_id.clone()),
        active_mode: None,
        boards: snapshot
            .boards
            .iter()
            .map(|board| BoardFile {
                id: board.id.clone(),
                pages: board.pages.pages.clone(),
                active_page: board.pages.active,
            })
            .collect(),
        transparent: None,
        whiteboard: None,
        blackboard: None,
        transparent_pages: None,
        whiteboard_pages: None,
        blackboard_pages: None,
        transparent_active_page: None,
        whiteboard_active_page: None,
        blackboard_active_page: None,
        tool_state: snapshot.tool_state.clone(),
    };

    serde_json::to_vec_pretty(&file_payload).context("failed to serialise session payload")
}

#[allow(dead_code)]
fn estimate_from_candidate(
    candidate: &PayloadCandidate,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> SnapshotPayloadEstimate {
    SnapshotPayloadEstimate {
        raw_size: candidate.raw_size,
        written_size: candidate.final_size(),
        max_file_size_bytes: options.max_file_size_bytes,
        compressed: candidate.compressed,
        limit_exceeded: candidate.limit_exceeded(options, max_expanded_size),
    }
}

fn is_near_limit(written_size: u64, max_file_size_bytes: u64) -> bool {
    if written_size == 0 {
        return false;
    }
    let threshold = ((max_file_size_bytes as u128) * (NEAR_LIMIT_PERCENT as u128)).div_ceil(100);
    (written_size as u128) >= threshold
}

fn max_history_depth(snapshot: &SessionSnapshot) -> usize {
    snapshot
        .boards
        .iter()
        .flat_map(|board| board.pages.pages.iter())
        .map(|page| page.undo_stack_len().max(page.redo_stack_len()))
        .max()
        .unwrap_or(0)
}

fn snapshot_with_history_depth(snapshot: &SessionSnapshot, depth: usize) -> SessionSnapshot {
    let mut candidate = snapshot.clone();
    for board in &mut candidate.boards {
        for page in &mut board.pages.pages {
            page.clamp_history_depth(depth);
        }
    }
    candidate
}

fn snapshot_without_history(snapshot: &SessionSnapshot) -> SessionSnapshot {
    let mut boards = Vec::with_capacity(snapshot.boards.len());
    for board in &snapshot.boards {
        let pages = BoardPagesSnapshot {
            pages: board
                .pages
                .pages
                .iter()
                .map(|page| page.clone_without_history())
                .collect(),
            active: board.pages.active,
        };
        if pages.has_persistable_data() {
            boards.push(BoardSnapshot {
                id: board.id.clone(),
                pages,
            });
        }
    }

    SessionSnapshot {
        active_board_id: snapshot.active_board_id.clone(),
        boards,
        tool_state: snapshot.tool_state.clone(),
    }
}

fn remove_session_file(session_path: &Path) -> Result<()> {
    if session_path.exists() {
        debug!(
            "Removing session file {} because snapshot is empty",
            session_path.display()
        );
        fs::remove_file(session_path).with_context(|| {
            format!(
                "failed to remove empty session file {}",
                session_path.display()
            )
        })?;
    }
    Ok(())
}

fn remove_recoverable_artifacts_suppressed_by_clear_marker(options: &SessionOptions) -> bool {
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

fn write_backup_recovery_marker(options: &SessionOptions) -> Result<()> {
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

fn write_recovery_recoverable_marker(options: &SessionOptions) -> Result<()> {
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

fn write_clear_marker(options: &SessionOptions) -> Result<()> {
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

fn remove_clear_marker_file(options: &SessionOptions) {
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

fn remove_backup_file(options: &SessionOptions) {
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

fn remove_backup_recovery_marker_file(options: &SessionOptions) {
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

fn remove_recovery_recoverable_marker_file(options: &SessionOptions) {
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

fn remove_recovery_files(options: &SessionOptions) {
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

fn remove_recovery_file(options: &SessionOptions) {
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
