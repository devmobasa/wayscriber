//! Session persistence (save/restore) support.
//!
//! Converts in-memory drawing state into a serialised representation, writes it
//! to disk with locking, optional compression, and backup rotation, and restores
//! the state on startup when requested.

mod lock;
mod options;
mod snapshot;
mod storage;

pub use lock::try_lock_exclusive;
#[allow(unused_imports)]
pub use options::{
    CompressionMode, DEFAULT_AUTO_COMPRESS_THRESHOLD_BYTES, SessionOptions, options_from_config,
};
#[allow(unused_imports)]
pub use snapshot::{
    BoardPagesSnapshot, BoardSnapshot, SessionSnapshot, ToolStateSnapshot, apply_snapshot,
    load_snapshot, save_snapshot, snapshot_from_input,
};
#[allow(unused_imports)]
pub(crate) use snapshot::{
    DEFAULT_MAX_EXPANDED_SESSION_BYTES, SaveLimitExceeded, SaveSnapshotOutcome, SaveSnapshotReport,
    SnapshotPayloadEstimate, SnapshotSaveEstimate, estimate_snapshot_payload,
    estimate_snapshot_save, estimate_snapshot_without_history_payload,
    save_snapshot_autosave_with_report, save_snapshot_with_report,
};
#[allow(unused_imports)]
pub(crate) use snapshot::{LoadSnapshotOutcome, load_snapshot_with_outcome};
#[allow(unused_imports)]
pub use storage::{ClearOutcome, FrameCounts, SessionInspection, clear_session, inspect_session};

#[cfg(test)]
mod tests;
