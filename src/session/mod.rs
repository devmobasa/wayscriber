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
pub(crate) use options::append_path_suffix;
#[allow(unused_imports)]
pub use options::{
    CompressionMode, DEFAULT_AUTO_COMPRESS_THRESHOLD_BYTES, SessionOptions, SessionTarget,
    normalize_named_session_file_arg, options_from_config, options_from_config_for_named_file,
    validate_named_session_file_for_clear, validate_named_session_file_for_foreground,
    validate_named_session_file_for_info,
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
    save_snapshot_autosave_with_report, save_snapshot_autosave_with_report_and_clear_boundary,
    save_snapshot_with_report, save_snapshot_with_report_and_clear_boundary,
};
#[allow(unused_imports)]
pub(crate) use snapshot::{LoadSnapshotOutcome, load_snapshot_with_outcome};
#[allow(unused_imports)]
pub use storage::{ClearOutcome, FrameCounts, SessionInspection, clear_session, inspect_session};

#[cfg(test)]
mod tests;
