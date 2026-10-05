//! What each file of a named session holds.
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::{named_session_artifact_paths, parse_corrupt_copy_name};

/// What a named session's artifact holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionArtifactKind {
    Primary,
    Backup,
    Recovery,
    /// Bytes that could not be read, preserved as a `.corrupt-N` copy of the
    /// primary, the backup or a recovery file. Nothing can restore from it.
    UnreadableCopy,
}

pub fn named_session_artifact_kind(primary: &Path, path: &Path) -> Option<SessionArtifactKind> {
    let artifacts = named_session_artifact_paths(primary);
    let name = path.file_name()?;
    let (source, copy) =
        parse_corrupt_copy_name(name).map_or((name, false), |(source, _)| (source, true));
    let kind = if source == artifacts.primary.file_name()? {
        SessionArtifactKind::Primary
    } else if source == artifacts.backup.file_name()? {
        SessionArtifactKind::Backup
    } else if is_recovery_variant(source, artifacts.recovery.file_name()?) {
        SessionArtifactKind::Recovery
    } else {
        return None;
    };

    Some(if copy {
        SessionArtifactKind::UnreadableCopy
    } else {
        kind
    })
}

/// The recovery file itself or one of its `.`-suffixed variants.
pub(crate) fn is_recovery_variant(name: &OsStr, recovery_name: &OsStr) -> bool {
    let name = name.as_bytes();
    let recovery = recovery_name.as_bytes();
    name == recovery
        || name
            .strip_prefix(recovery)
            .is_some_and(|suffix| suffix.starts_with(b"."))
}
