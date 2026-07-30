use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurableIoOperation {
    InspectDestination,
    ReadLink,
    OpenTemporary,
    InspectTemporary,
    WriteTemporary,
    SetPermissions,
    SyncTemporary,
    FinalizeRename,
    SyncParent,
}

#[derive(Debug)]
pub enum DurableIoError {
    Io {
        operation: DurableIoOperation,
        path: PathBuf,
        source: io::Error,
    },
    AlreadyExists {
        path: PathBuf,
    },
    SymlinkRejected {
        path: PathBuf,
    },
    UnsupportedFileType {
        path: PathBuf,
    },
    DestinationChanged {
        operation: DurableIoOperation,
        path: PathBuf,
    },
    Conflict {
        operation: DurableIoOperation,
        path: PathBuf,
        reason: String,
    },
    MissingParent {
        path: PathBuf,
    },
}

impl fmt::Display for DurableIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(f, "{operation:?} failed for {}: {source}", path.display()),
            Self::AlreadyExists { path } => write!(f, "{} already exists", path.display()),
            Self::SymlinkRejected { path } => write!(f, "symlink rejected: {}", path.display()),
            Self::UnsupportedFileType { path } => {
                write!(f, "unsupported file type: {}", path.display())
            }
            Self::DestinationChanged { operation, path } => write!(
                f,
                "destination changed during {operation:?}: {}",
                path.display()
            ),
            Self::Conflict {
                operation,
                path,
                reason,
            } => write!(f, "{operation:?} conflict for {}: {reason}", path.display()),
            Self::MissingParent { path } => {
                write!(f, "{} has no parent directory", path.display())
            }
        }
    }
}

impl std::error::Error for DurableIoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverwriteMode {
    Replace,
    CreateNew,
}

/// Which file a path named, as far as the platform can tell one file from
/// another.
///
/// On Unix that is the device and inode pair, and it is what makes a file
/// renamed away and replaced at the same path a *different* file even when the
/// replacement holds identical bytes. Elsewhere there is nothing to compare, so
/// every file has the same empty identity and a caller that needs the
/// distinction has to carry it another way — the config save compares the bytes
/// it read.
#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(not(unix))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileIdentity;

impl FileIdentity {
    #[cfg(unix)]
    pub fn of(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }

    #[cfg(not(unix))]
    pub fn of(_metadata: &fs::Metadata) -> Self {
        Self
    }
}

/// What the caller last saw at the destination, to be confirmed immediately
/// before the rename.
///
/// The write already refuses a destination that changed since *it* looked, but
/// its own look happens after the caller's — so a file changed between the two
/// is one this writer has no reason to doubt, and the rename replaces bytes
/// nothing ever checked. Identity catches an atomic replacement; exact contents
/// catch an editor that truncates and rewrites the same file in place. Carrying
/// both makes the rename conditional on the complete revision the caller read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinationExpectation<'a> {
    /// The path named nothing when the caller looked. Anything there now was
    /// put there by somebody else, and creating over it would discard it.
    Absent,
    /// It named this regular file with exactly these contents.
    Present {
        identity: FileIdentity,
        contents: &'a [u8],
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionPolicy {
    PreserveExistingOrMode(u32),
    FixedMode(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymlinkPolicy {
    FollowExistingTarget,
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtomicWriteOptions {
    pub overwrite: OverwriteMode,
    pub permissions: PermissionPolicy,
    pub symlink: SymlinkPolicy,
    pub sync_file: bool,
    pub sync_parent: bool,
}

impl AtomicWriteOptions {
    pub const fn private_runtime_file() -> Self {
        Self {
            overwrite: OverwriteMode::Replace,
            permissions: PermissionPolicy::FixedMode(0o600),
            symlink: SymlinkPolicy::Reject,
            sync_file: false,
            sync_parent: false,
        }
    }

    pub const fn user_config_file() -> Self {
        Self {
            overwrite: OverwriteMode::Replace,
            permissions: PermissionPolicy::PreserveExistingOrMode(0o644),
            symlink: SymlinkPolicy::FollowExistingTarget,
            sync_file: true,
            sync_parent: true,
        }
    }
}
