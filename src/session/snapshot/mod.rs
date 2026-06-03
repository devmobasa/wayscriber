mod apply;
mod capture;
mod compression;
mod history;
mod load;
mod save;
mod types;

#[cfg(test)]
mod tests;

pub use apply::apply_snapshot;
pub(crate) use apply::apply_snapshot_replacing_boards;
pub use capture::snapshot_from_input;
pub(crate) use compression::DEFAULT_MAX_EXPANDED_SESSION_BYTES;
pub use load::load_snapshot;
pub(crate) use load::{
    LoadSnapshotOutcome, LoadedSnapshot, load_named_session_candidate, load_snapshot_inner,
    load_snapshot_with_outcome,
};
pub use save::save_snapshot;
pub(crate) use save::{
    SaveAsOverwrite, SaveLimitExceeded, SaveSnapshotOutcome, SaveSnapshotReport,
    SnapshotPayloadEstimate, SnapshotSaveEstimate, estimate_snapshot_payload,
    estimate_snapshot_save, estimate_snapshot_without_history_payload,
    save_snapshot_as_with_report, save_snapshot_autosave_with_report,
    save_snapshot_autosave_with_report_and_clear_boundary, save_snapshot_with_report,
    save_snapshot_with_report_and_clear_boundary,
};
#[allow(unused_imports)]
pub use types::BoardSnapshot;
pub use types::{BoardPagesSnapshot, SessionSnapshot, ToolStateSnapshot};
