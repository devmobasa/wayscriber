use super::payload::is_near_limit;
use log::debug;
use std::fmt;
use std::path::PathBuf;

const NEAR_LIMIT_PERCENT: u64 = 90;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HistoryFallbackStrategy {
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

/// Overwrite policy for runtime Save Session As.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveAsOverwrite {
    /// Reject any existing primary or non-lock sidecar artifacts.
    Deny,
    /// Replace the selected primary and remove selected non-lock sidecars.
    ConfirmReplace,
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
    pub(super) fn description(self) -> String {
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
pub(super) struct SavePayloadTooLarge {
    pub(super) limit: SaveLimitExceeded,
    pub(super) written_size: usize,
    pub(super) raw_size: usize,
    pub(super) compressed: bool,
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

pub(super) fn log_near_limit(report: &SaveSnapshotReport) {
    if report.is_near_limit() {
        debug!(
            "Session save size is near the configured limit ({} of {} bytes, threshold={}%)",
            report.written_size, report.max_file_size_bytes, NEAR_LIMIT_PERCENT
        );
    }
}
