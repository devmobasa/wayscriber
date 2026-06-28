use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurableIoOperation {
    InspectDestination,
    ReadLink,
    OpenTemporary,
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
