use std::fmt;
use std::fs::{self, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use super::{PathResolutionError, PathResolver};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedRuntimePaths {
    root: PathBuf,
}

impl PreparedRuntimePaths {
    pub fn prepare(resolver: &PathResolver) -> Result<Self, PrepareRuntimePathsError> {
        let root = resolver.runtime_dir()?;
        prepare_private_runtime_directory(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn tray_action_file(&self) -> PathBuf {
        self.root.join("tray_action")
    }

    pub fn tray_action_dir(&self) -> PathBuf {
        self.root.join("tray-actions")
    }

    pub fn daemon_command_file(&self) -> PathBuf {
        self.root.join("daemon_command.json")
    }

    pub fn daemon_command_dir(&self) -> PathBuf {
        self.root.join("daemon-commands")
    }

    pub fn protocol_v2_root(&self) -> PathBuf {
        self.daemon_command_dir().join("v2")
    }

    pub fn daemon_pid_file(&self) -> PathBuf {
        self.root.join("wayscriber.pid")
    }

    pub fn daemon_lock_file(&self) -> PathBuf {
        self.root.join("wayscriber.lock")
    }

    pub fn overlay_lock_file(&self) -> PathBuf {
        self.root.join("wayscriber-overlay.lock")
    }
}

#[derive(Debug)]
pub enum PrepareRuntimePathsError {
    Resolve(PathResolutionError),
    Prepare(RuntimeDirectoryError),
}

impl fmt::Display for PrepareRuntimePathsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resolve(error) => {
                write!(formatter, "failed to resolve runtime identity: {error}")
            }
            Self::Prepare(error) => {
                write!(formatter, "failed to prepare runtime identity: {error}")
            }
        }
    }
}

impl std::error::Error for PrepareRuntimePathsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resolve(error) => Some(error),
            Self::Prepare(error) => Some(error),
        }
    }
}

impl From<PathResolutionError> for PrepareRuntimePathsError {
    fn from(value: PathResolutionError) -> Self {
        Self::Resolve(value)
    }
}

impl From<RuntimeDirectoryError> for PrepareRuntimePathsError {
    fn from(value: RuntimeDirectoryError) -> Self {
        Self::Prepare(value)
    }
}

#[derive(Debug)]
pub enum RuntimeDirectoryError {
    Inspect {
        path: PathBuf,
        source: std::io::Error,
    },
    Create {
        path: PathBuf,
        source: std::io::Error,
    },
    Permissions {
        path: PathBuf,
        source: std::io::Error,
    },
    Symlink {
        path: PathBuf,
    },
    NotDirectory {
        path: PathBuf,
    },
    WrongOwner {
        path: PathBuf,
        expected: u32,
        actual: u32,
    },
    PublicPermissions {
        path: PathBuf,
        mode: u32,
    },
}

impl fmt::Display for RuntimeDirectoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inspect { path, source } => {
                write!(formatter, "failed to inspect {}: {source}", path.display())
            }
            Self::Create { path, source } => {
                write!(formatter, "failed to create {}: {source}", path.display())
            }
            Self::Permissions { path, source } => write!(
                formatter,
                "failed to secure runtime directory {}: {source}",
                path.display()
            ),
            Self::Symlink { path } => {
                write!(
                    formatter,
                    "runtime directory {} is a symlink",
                    path.display()
                )
            }
            Self::NotDirectory { path } => {
                write!(
                    formatter,
                    "runtime path {} is not a directory",
                    path.display()
                )
            }
            Self::WrongOwner {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "runtime directory {} is owned by uid {actual}, expected {expected}",
                path.display()
            ),
            Self::PublicPermissions { path, mode } => write!(
                formatter,
                "runtime directory {} has non-private mode {:o}",
                path.display(),
                mode & 0o777
            ),
        }
    }
}

impl std::error::Error for RuntimeDirectoryError {}

/// Prepare and verify the directory before creating an identity-bearing file.
pub fn prepare_private_runtime_directory(path: &Path) -> Result<(), RuntimeDirectoryError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            verify_identity(path, &metadata)?;
            verify_open_directory(path)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(path)
                .map_err(|source| RuntimeDirectoryError::Create {
                    path: path.to_path_buf(),
                    source,
                })?;
            let metadata =
                fs::symlink_metadata(path).map_err(|source| RuntimeDirectoryError::Inspect {
                    path: path.to_path_buf(),
                    source,
                })?;
            verify_identity(path, &metadata)?;
            verify_open_directory(path)
        }
        Err(source) => Err(RuntimeDirectoryError::Inspect {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn verify_open_directory(path: &Path) -> Result<(), RuntimeDirectoryError> {
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|source| RuntimeDirectoryError::Inspect {
            path: path.to_path_buf(),
            source,
        })?;
    let metadata = directory
        .metadata()
        .map_err(|source| RuntimeDirectoryError::Inspect {
            path: path.to_path_buf(),
            source,
        })?;
    verify_identity(path, &metadata)?;
    if metadata.permissions().mode() & 0o077 != 0 {
        directory
            .set_permissions(fs::Permissions::from_mode(0o700))
            .map_err(|source| RuntimeDirectoryError::Permissions {
                path: path.to_path_buf(),
                source,
            })?;
    }
    let metadata = directory
        .metadata()
        .map_err(|source| RuntimeDirectoryError::Inspect {
            path: path.to_path_buf(),
            source,
        })?;
    verify(path, &metadata)
}

fn verify_identity(path: &Path, metadata: &fs::Metadata) -> Result<(), RuntimeDirectoryError> {
    if metadata.file_type().is_symlink() {
        return Err(RuntimeDirectoryError::Symlink {
            path: path.to_path_buf(),
        });
    }
    if !metadata.is_dir() {
        return Err(RuntimeDirectoryError::NotDirectory {
            path: path.to_path_buf(),
        });
    }
    // SAFETY: geteuid has no preconditions and does not retain a pointer.
    let expected = unsafe { libc::geteuid() };
    if metadata.uid() != expected {
        return Err(RuntimeDirectoryError::WrongOwner {
            path: path.to_path_buf(),
            expected,
            actual: metadata.uid(),
        });
    }
    Ok(())
}

fn verify(path: &Path, metadata: &fs::Metadata) -> Result<(), RuntimeDirectoryError> {
    verify_identity(path, metadata)?;
    let mode = metadata.permissions().mode();
    if mode & 0o077 != 0 {
        return Err(RuntimeDirectoryError::PublicPermissions {
            path: path.to_path_buf(),
            mode,
        });
    }
    Ok(())
}
