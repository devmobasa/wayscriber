mod document;
mod file_io;
mod immutability;
mod load;
mod migration;
#[cfg(feature = "config-schema")]
mod schema;
mod validate;

use super::{Config, ConfigDocument};
use std::path::PathBuf;

/// The runtime write shape, spelled out once: reload the document at the active
/// config path, then hand it the caller's edited config so the merge diffs
/// against what is on disk right now. The background `ConfigWriter`, the tray
/// toggle, and the configurator all persist through this same pair of calls.
fn save_through_document(config: Config) {
    ConfigDocument::load()
        .expect("load config document before save")
        .save(config)
        .expect("document save should succeed");
}

/// Same, with the timestamped `.bak` the configurator asks for. Returns the
/// backup path when one was created.
fn save_through_document_with_backup(config: Config) -> Option<PathBuf> {
    ConfigDocument::load()
        .expect("load config document before backup save")
        .save_with_backup(config)
        .expect("document save_with_backup should succeed")
        .into_parts()
        .1
}
