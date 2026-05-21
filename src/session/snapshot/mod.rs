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
pub use capture::snapshot_from_input;
pub use load::load_snapshot;
pub(crate) use load::{LoadSnapshotOutcome, load_snapshot_inner, load_snapshot_with_outcome};
pub use save::save_snapshot;
pub(crate) use save::{
    SaveLimitExceeded, SaveSnapshotOutcome, SaveSnapshotReport, SnapshotPayloadEstimate,
    SnapshotSaveEstimate, estimate_snapshot_payload, estimate_snapshot_save,
    estimate_snapshot_without_history_payload, save_snapshot_with_report,
};
#[allow(unused_imports)]
pub use types::BoardSnapshot;
pub use types::{BoardPagesSnapshot, SessionSnapshot, ToolStateSnapshot};
