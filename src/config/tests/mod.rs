mod document;
mod file_io;
mod immutability;
mod load;
mod migration;
#[cfg(feature = "config-schema")]
mod schema;
mod validate;
mod write_lock;
mod write_target;

use super::{Config, ConfigDocument};
use std::path::PathBuf;

/// The only write shape left, spelled out once: reload the document at the
/// active config path, then hand it the caller's edited config so the merge
/// diffs against what is on disk right now. The configurator's explicit Save
/// performs exactly this pair of calls; nothing else in the application does.
fn save_through_document(config: Config) {
    let _ = save_through_document_with_backup(config);
}

/// Same, returning the timestamped `.bak` path the save left beside the file.
fn save_through_document_with_backup(config: Config) -> Option<PathBuf> {
    ConfigDocument::load()
        .expect("load config document before backup save")
        .save_with_backup(config)
        .expect("document save_with_backup should succeed")
        .into_parts()
        .1
}
