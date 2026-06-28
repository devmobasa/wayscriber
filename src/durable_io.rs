use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

mod model;

pub use model::{
    AtomicWriteOptions, DurableIoError, DurableIoOperation, OverwriteMode, PermissionPolicy,
    SymlinkPolicy,
};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct Destination {
    original_path: PathBuf,
    final_path: PathBuf,
    followed_target: Option<PathBuf>,
    existing_mode: Option<u32>,
    existing_identity: Option<FileIdentity>,
    existed_at_inspect: bool,
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(not(unix))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity;

pub fn write_text_atomic(
    path: &Path,
    contents: &str,
    options: AtomicWriteOptions,
) -> Result<(), DurableIoError> {
    write_atomic(path, contents.as_bytes(), options)
}

pub fn write_atomic(
    path: &Path,
    bytes: &[u8],
    options: AtomicWriteOptions,
) -> Result<(), DurableIoError> {
    let destination = inspect_destination(path, options)?;
    let parent = destination
        .final_path
        .parent()
        .ok_or_else(|| DurableIoError::MissingParent {
            path: destination.final_path.clone(),
        })?;
    let file_name =
        destination
            .final_path
            .file_name()
            .ok_or_else(|| DurableIoError::MissingParent {
                path: destination.final_path.clone(),
            })?;
    let (temp_path, mut temp_file) = create_temp_file(parent, file_name)?;

    let result = (|| {
        temp_file
            .write_all(bytes)
            .map_err(|source| io_error(DurableIoOperation::WriteTemporary, &temp_path, source))?;
        apply_final_permissions(&temp_path, &destination, options.permissions)?;
        if options.sync_file {
            temp_file.sync_all().map_err(|source| {
                io_error(DurableIoOperation::SyncTemporary, &temp_path, source)
            })?;
        }
        drop(temp_file);
        revalidate_destination(&destination, options)?;
        finalize_temp_file(
            &temp_path,
            &destination.final_path,
            finalize_overwrite_mode(&destination, options),
        )?;
        if options.sync_parent {
            sync_parent_dir(&destination.final_path)?;
        }
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

pub fn sync_parent_dir(path: &Path) -> Result<(), DurableIoError> {
    let parent = path.parent().ok_or_else(|| DurableIoError::MissingParent {
        path: path.to_path_buf(),
    })?;
    sync_dir(parent)
}

fn inspect_destination(
    path: &Path,
    options: AtomicWriteOptions,
) -> Result<Destination, DurableIoError> {
    match options.symlink {
        SymlinkPolicy::FollowExistingTarget => inspect_follow_destination(path),
        SymlinkPolicy::Reject => inspect_reject_destination(path),
    }
}

fn inspect_follow_destination(path: &Path) -> Result<Destination, DurableIoError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target = read_resolved_link(path)?;
            let target_metadata = fs::symlink_metadata(&target).map_err(|source| {
                io_error(DurableIoOperation::InspectDestination, &target, source)
            })?;
            if target_metadata.file_type().is_symlink() {
                return Err(DurableIoError::SymlinkRejected { path: target });
            }
            if !target_metadata.is_file() {
                return Err(DurableIoError::UnsupportedFileType { path: target });
            }
            Ok(Destination {
                original_path: path.to_path_buf(),
                final_path: target.clone(),
                followed_target: Some(target),
                existing_mode: metadata_mode(&target_metadata),
                existing_identity: file_identity(&target_metadata),
                existed_at_inspect: true,
            })
        }
        Ok(metadata) if metadata.is_file() => Ok(Destination {
            original_path: path.to_path_buf(),
            final_path: path.to_path_buf(),
            followed_target: None,
            existing_mode: metadata_mode(&metadata),
            existing_identity: file_identity(&metadata),
            existed_at_inspect: true,
        }),
        Ok(_) => Err(DurableIoError::UnsupportedFileType {
            path: path.to_path_buf(),
        }),
        Err(source) if source.kind() == ErrorKind::NotFound => Ok(Destination {
            original_path: path.to_path_buf(),
            final_path: path.to_path_buf(),
            followed_target: None,
            existing_mode: None,
            existing_identity: None,
            existed_at_inspect: false,
        }),
        Err(source) => Err(io_error(
            DurableIoOperation::InspectDestination,
            path,
            source,
        )),
    }
}

fn inspect_reject_destination(path: &Path) -> Result<Destination, DurableIoError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(DurableIoError::SymlinkRejected {
            path: path.to_path_buf(),
        }),
        Ok(metadata) if metadata.is_file() => Ok(Destination {
            original_path: path.to_path_buf(),
            final_path: path.to_path_buf(),
            followed_target: None,
            existing_mode: metadata_mode(&metadata),
            existing_identity: file_identity(&metadata),
            existed_at_inspect: true,
        }),
        Ok(_) => Err(DurableIoError::UnsupportedFileType {
            path: path.to_path_buf(),
        }),
        Err(source) if source.kind() == ErrorKind::NotFound => Ok(Destination {
            original_path: path.to_path_buf(),
            final_path: path.to_path_buf(),
            followed_target: None,
            existing_mode: None,
            existing_identity: None,
            existed_at_inspect: false,
        }),
        Err(source) => Err(io_error(
            DurableIoOperation::InspectDestination,
            path,
            source,
        )),
    }
}

fn revalidate_destination(
    destination: &Destination,
    options: AtomicWriteOptions,
) -> Result<(), DurableIoError> {
    if let Some(expected) = &destination.followed_target {
        let current = read_resolved_link(&destination.original_path).map_err(|_| {
            DurableIoError::DestinationChanged {
                operation: DurableIoOperation::ReadLink,
                path: destination.original_path.clone(),
            }
        })?;
        if current != *expected {
            return Err(DurableIoError::DestinationChanged {
                operation: DurableIoOperation::ReadLink,
                path: destination.original_path.clone(),
            });
        }
    }

    match options.overwrite {
        OverwriteMode::CreateNew => match fs::symlink_metadata(&destination.final_path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                Err(DurableIoError::SymlinkRejected {
                    path: destination.final_path.clone(),
                })
            }
            Ok(_) => Err(DurableIoError::AlreadyExists {
                path: destination.final_path.clone(),
            }),
            Err(source) if source.kind() == ErrorKind::NotFound => Ok(()),
            Err(source) => Err(io_error(
                DurableIoOperation::InspectDestination,
                &destination.final_path,
                source,
            )),
        },
        OverwriteMode::Replace => revalidate_replace_destination(destination, options.symlink),
    }
}

fn revalidate_replace_destination(
    destination: &Destination,
    symlink: SymlinkPolicy,
) -> Result<(), DurableIoError> {
    match fs::symlink_metadata(&destination.final_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(DurableIoError::SymlinkRejected {
            path: destination.final_path.clone(),
        }),
        Ok(metadata) if !metadata.is_file() => Err(DurableIoError::UnsupportedFileType {
            path: destination.final_path.clone(),
        }),
        Ok(metadata) => {
            if symlink == SymlinkPolicy::Reject && !destination.existed_at_inspect {
                return Err(DurableIoError::DestinationChanged {
                    operation: DurableIoOperation::InspectDestination,
                    path: destination.final_path.clone(),
                });
            }
            if symlink == SymlinkPolicy::Reject
                && let Some(expected) = destination.existing_identity
                && file_identity(&metadata) != Some(expected)
            {
                return Err(DurableIoError::DestinationChanged {
                    operation: DurableIoOperation::InspectDestination,
                    path: destination.final_path.clone(),
                });
            }
            Ok(())
        }
        Err(source) if source.kind() == ErrorKind::NotFound => {
            if destination.existing_identity.is_some() {
                Err(DurableIoError::DestinationChanged {
                    operation: DurableIoOperation::InspectDestination,
                    path: destination.final_path.clone(),
                })
            } else {
                Ok(())
            }
        }
        Err(source) => Err(io_error(
            DurableIoOperation::InspectDestination,
            &destination.final_path,
            source,
        )),
    }
}

fn finalize_overwrite_mode(
    destination: &Destination,
    options: AtomicWriteOptions,
) -> OverwriteMode {
    if options.overwrite == OverwriteMode::Replace
        && options.symlink == SymlinkPolicy::Reject
        && !destination.existed_at_inspect
    {
        OverwriteMode::CreateNew
    } else {
        options.overwrite
    }
}

fn create_temp_file(
    parent: &Path,
    file_name: &std::ffi::OsStr,
) -> Result<(PathBuf, File), DurableIoError> {
    create_temp_file_from_candidates((0..64).map(|_| next_temp_path(parent, file_name)))
}

fn create_temp_file_from_candidates<I>(candidates: I) -> Result<(PathBuf, File), DurableIoError>
where
    I: IntoIterator<Item = PathBuf>,
{
    let mut last_path = None;
    for path in candidates {
        last_path = Some(path.clone());
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(source) if source.kind() == ErrorKind::AlreadyExists => continue,
            Err(source) => return Err(io_error(DurableIoOperation::OpenTemporary, &path, source)),
        }
    }
    Err(DurableIoError::Conflict {
        operation: DurableIoOperation::OpenTemporary,
        path: last_path.unwrap_or_default(),
        reason: "temporary path collision retry budget exhausted".to_string(),
    })
}

fn next_temp_path(parent: &Path, file_name: &std::ffi::OsStr) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mut name = std::ffi::OsString::from(".");
    name.push(file_name);
    name.push(format!(
        ".{}.{}.{}.tmp",
        std::process::id(),
        stamp,
        sequence
    ));
    parent.join(name)
}

fn finalize_temp_file(
    temp_path: &Path,
    final_path: &Path,
    overwrite: OverwriteMode,
) -> Result<(), DurableIoError> {
    let result = match overwrite {
        OverwriteMode::Replace => fs::rename(temp_path, final_path),
        OverwriteMode::CreateNew => rename_no_replace(temp_path, final_path),
    };
    result.map_err(|source| {
        if overwrite == OverwriteMode::CreateNew && source.kind() == ErrorKind::AlreadyExists {
            DurableIoError::AlreadyExists {
                path: final_path.to_path_buf(),
            }
        } else {
            io_error(DurableIoOperation::FinalizeRename, final_path, source)
        }
    })
}

#[cfg(unix)]
fn apply_final_permissions(
    temp_path: &Path,
    destination: &Destination,
    policy: PermissionPolicy,
) -> Result<(), DurableIoError> {
    let mode = match policy {
        PermissionPolicy::FixedMode(mode) => mode,
        PermissionPolicy::PreserveExistingOrMode(mode) => destination.existing_mode.unwrap_or(mode),
    };
    fs::set_permissions(temp_path, fs::Permissions::from_mode(mode))
        .map_err(|source| io_error(DurableIoOperation::SetPermissions, temp_path, source))
}

#[cfg(not(unix))]
fn apply_final_permissions(
    _temp_path: &Path,
    _destination: &Destination,
    _policy: PermissionPolicy,
) -> Result<(), DurableIoError> {
    Ok(())
}

fn read_resolved_link(path: &Path) -> Result<PathBuf, DurableIoError> {
    let target = fs::read_link(path)
        .map_err(|source| io_error(DurableIoOperation::ReadLink, path, source))?;
    Ok(if target.is_absolute() {
        target
    } else {
        path.parent().unwrap_or_else(|| Path::new(".")).join(target)
    })
}

#[cfg(unix)]
fn metadata_mode(metadata: &fs::Metadata) -> Option<u32> {
    Some(metadata.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn metadata_mode(_metadata: &fs::Metadata) -> Option<u32> {
    None
}

#[cfg(unix)]
fn file_identity(metadata: &fs::Metadata) -> Option<FileIdentity> {
    Some(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(not(unix))]
fn file_identity(_metadata: &fs::Metadata) -> Option<FileIdentity> {
    None
}

#[cfg(target_os = "linux")]
pub fn rename_no_replace(source: &Path, target: &Path) -> io::Result<()> {
    let source = path_to_cstring(source)?;
    let target = path_to_cstring(target)?;
    // SAFETY: The C strings are valid, NUL-terminated paths. AT_FDCWD makes both
    // paths relative to the process cwd, matching std::fs::rename path semantics.
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
fn path_to_cstring(path: &Path) -> io::Result<std::ffi::CString> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!("path contains an interior NUL byte: {}", path.display()),
        )
    })
}

#[cfg(not(target_os = "linux"))]
pub fn rename_no_replace(source: &Path, target: &Path) -> io::Result<()> {
    fs::hard_link(source, target)?;
    fs::remove_file(source)
}

#[cfg(unix)]
fn sync_dir(parent: &Path) -> Result<(), DurableIoError> {
    let dir = File::open(parent)
        .map_err(|source| io_error(DurableIoOperation::SyncParent, parent, source))?;
    dir.sync_all()
        .map_err(|source| io_error(DurableIoOperation::SyncParent, parent, source))
}

#[cfg(not(unix))]
fn sync_dir(_parent: &Path) -> Result<(), DurableIoError> {
    Ok(())
}

fn io_error(operation: DurableIoOperation, path: &Path, source: io::Error) -> DurableIoError {
    DurableIoError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests;
