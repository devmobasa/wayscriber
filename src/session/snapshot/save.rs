use super::compression::{DEFAULT_MAX_EXPANDED_SESSION_BYTES, compress_bytes, temp_path};
use super::types::{
    BoardFile, BoardPagesSnapshot, BoardSnapshot, CURRENT_VERSION, SessionFile, SessionSnapshot,
};
use crate::session::lock::{lock_exclusive, unlock};
use crate::session::options::{CompressionMode, SessionOptions};
use crate::time_utils::now_rfc3339;
use anyhow::{Context, Result, anyhow};
use log::{debug, info, warn};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const NEAR_LIMIT_PERCENT: u64 = 90;

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
        if self.written_size == 0 {
            return false;
        }
        let threshold =
            ((self.max_file_size_bytes as u128) * (NEAR_LIMIT_PERCENT as u128)).div_ceil(100);
        (self.written_size as u128) >= threshold
    }
}

/// Persist the provided snapshot to disk according to the configured options.
pub fn save_snapshot(snapshot: &SessionSnapshot, options: &SessionOptions) -> Result<()> {
    save_snapshot_with_report(snapshot, options).map(|_| ())
}

/// Persist the provided snapshot and report what was written.
pub(crate) fn save_snapshot_with_report(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
) -> Result<Option<SaveSnapshotReport>> {
    save_snapshot_with_expanded_limit(snapshot, options, DEFAULT_MAX_EXPANDED_SESSION_BYTES)
}

pub(super) fn save_snapshot_with_expanded_limit(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<Option<SaveSnapshotReport>> {
    if !options.any_enabled() && !options.persist_history && snapshot.tool_state.is_none() {
        debug!("Session persistence disabled for all boards; skipping save");
        return Ok(None);
    }

    fs::create_dir_all(&options.base_dir).with_context(|| {
        format!(
            "failed to create session directory {}",
            options.base_dir.display()
        )
    })?;

    let lock_path = options.lock_file_path();
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open session lock file {}", lock_path.display()))?;
    lock_exclusive(&lock_file)
        .with_context(|| format!("failed to lock session file {}", lock_path.display()))?;

    let result = save_snapshot_inner(snapshot, options, max_expanded_size);

    if let Err(err) = unlock(&lock_file) {
        warn!(
            "failed to unlock session file {}: {}",
            lock_path.display(),
            err
        );
    }

    result
}

fn save_snapshot_inner(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<Option<SaveSnapshotReport>> {
    let session_path = options.session_file_path();
    let backup_path = options.backup_file_path();
    let last_modified = now_rfc3339();

    let prepared = match payload_within_limit(snapshot, options, &last_modified, max_expanded_size)
    {
        Ok(prepared) => prepared,
        Err(err) => {
            if session_path.exists() {
                warn!(
                    "Session save failed before replacing {}; existing session file is unchanged",
                    session_path.display()
                );
            }
            return Err(err);
        }
    };

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
            remove_session_file(&session_path)?;
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

    if session_path.exists() {
        if options.backup_retention > 0 {
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
        } else {
            fs::remove_file(&session_path).ok();
        }
    }

    fs::rename(&tmp_path, &session_path).with_context(|| {
        format!(
            "failed to move temporary session file {} -> {}",
            tmp_path.display(),
            session_path.display()
        )
    })?;

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
    ) -> Option<PayloadLimitExceeded> {
        if self.final_size() as u64 > options.max_file_size_bytes {
            return Some(PayloadLimitExceeded::WrittenSize {
                written_size: self.final_size() as u64,
                max_file_size: options.max_file_size_bytes,
            });
        }
        if self.compressed && self.raw_size as u64 > max_expanded_size {
            return Some(PayloadLimitExceeded::ExpandedSize {
                raw_size: self.raw_size as u64,
                max_expanded_size,
            });
        }
        None
    }

    fn fits_limit(&self, options: &SessionOptions, max_expanded_size: u64) -> bool {
        self.limit_exceeded(options, max_expanded_size).is_none()
    }
}

#[derive(Debug, Clone, Copy)]
enum PayloadLimitExceeded {
    WrittenSize {
        written_size: u64,
        max_file_size: u64,
    },
    ExpandedSize {
        raw_size: u64,
        max_expanded_size: u64,
    },
}

impl PayloadLimitExceeded {
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
) -> Result<PreparedPayload> {
    if snapshot.is_empty() && snapshot.tool_state.is_none() {
        return Ok(PreparedPayload::clear(0, false));
    }

    let full_payload = payload_candidate(snapshot, options, last_modified)?;
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
    let history_depth = max_history_depth(snapshot);
    if history_depth > 0
        && let Some((depth, payload)) = largest_fitting_history_payload(
            snapshot,
            history_depth,
            options,
            last_modified,
            max_expanded_size,
        )?
    {
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

    let visible_payload = payload_candidate(&visible_only, options, last_modified)?;
    if visible_payload.fits_limit(options, max_expanded_size) {
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
        return Ok(PreparedPayload::write(
            visible_payload,
            SaveSnapshotOutcome::VisibleOnly,
        ));
    }

    let visible_limit = visible_payload
        .limit_exceeded(options, max_expanded_size)
        .expect("visible payload should exceed a save/load limit");
    Err(anyhow!(
        "Session data cannot be saved safely ({}; {} bytes written from {} raw bytes, compression={}); skipping save",
        visible_limit.description(),
        visible_payload.final_size(),
        visible_payload.raw_size,
        visible_payload.compressed
    ))
}

fn largest_fitting_history_payload(
    snapshot: &SessionSnapshot,
    max_depth: usize,
    options: &SessionOptions,
    last_modified: &str,
    max_expanded_size: u64,
) -> Result<Option<(usize, PayloadCandidate)>> {
    for depth in (1..=max_depth).rev() {
        let candidate = snapshot_with_history_depth(snapshot, depth);
        let payload = payload_candidate(&candidate, options, last_modified)?;
        if payload.fits_limit(options, max_expanded_size) {
            return Ok(Some((depth, payload)));
        }
    }

    Ok(None)
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
