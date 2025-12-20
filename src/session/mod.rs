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
    SessionSnapshot, ToolStateSnapshot, apply_snapshot, load_snapshot, save_snapshot,
    snapshot_from_input,
};
#[allow(unused_imports)]
pub use storage::{ClearOutcome, FrameCounts, SessionInspection, clear_session, inspect_session};

#[cfg(test)]
mod tests;
