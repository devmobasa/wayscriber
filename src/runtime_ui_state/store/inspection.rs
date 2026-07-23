use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::sync::Arc;

use super::{MAX_RUNTIME_UI_FILE_BYTES, RuntimeUiStateInspection, fs::PinnedPath};
use crate::runtime_ui_state::{
    RuntimeStateFileIdentity, RuntimeStateInspectionError, RuntimeStatePathIdentity,
    RuntimeStateResolvedParent, RuntimeStateSourceObservation, RuntimeStateSourceRevision,
    decode_runtime_ui_file,
};

pub(super) fn inspect_path(
    path: &Path,
) -> Result<RuntimeUiStateInspection, RuntimeStateInspectionError> {
    let Some(parent) = resolve_parent(path)? else {
        return Ok(RuntimeUiStateInspection::missing(
            RuntimeStateSourceRevision::missing(RuntimeStatePathIdentity::direct(path)),
        ));
    };
    let resolved_path = parent.resolved_path.join(file_name(path)?);
    let path_identity = parent.path_identity(path);
    let mut file = match open_regular_nofollow(&resolved_path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            verify_parent(path, &parent)?;
            return Ok(RuntimeUiStateInspection::missing(
                RuntimeStateSourceRevision::missing(path_identity),
            ));
        }
        Err(error) => return Err(inspection_error(&resolved_path, error)),
    };
    let before = file
        .metadata()
        .map_err(|error| inspection_error(&resolved_path, error))?;
    if !before.file_type().is_file() {
        return Err(RuntimeStateInspectionError::new(format!(
            "runtime-state path is not a regular file: {}",
            path.display()
        )));
    }
    if before.len() > MAX_RUNTIME_UI_FILE_BYTES {
        return Err(RuntimeStateInspectionError::new(format!(
            "runtime-state file exceeds the {} byte limit",
            MAX_RUNTIME_UI_FILE_BYTES
        )));
    }

    let bytes = read_bounded(&resolved_path, &mut file)?;
    if bytes.len() as u64 > MAX_RUNTIME_UI_FILE_BYTES {
        return Err(RuntimeStateInspectionError::new(format!(
            "runtime-state file exceeds the {} byte limit",
            MAX_RUNTIME_UI_FILE_BYTES
        )));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| inspection_error(&resolved_path, error))?;
    let verification_bytes = read_bounded(&resolved_path, &mut file)?;
    let after = file
        .metadata()
        .map_err(|error| inspection_error(&resolved_path, error))?;
    let active = fs::symlink_metadata(&resolved_path)
        .map_err(|error| inspection_error(&resolved_path, error))?;
    if active.file_type().is_symlink() || !active.file_type().is_file() {
        return Err(RuntimeStateInspectionError::new(format!(
            "runtime-state path changed type during inspection: {}",
            path.display()
        )));
    }
    let before_fingerprint = file_fingerprint(&before);
    if verification_bytes != bytes
        || file_fingerprint(&after) != before_fingerprint
        || file_fingerprint(&active) != before_fingerprint
    {
        return Err(RuntimeStateInspectionError::new(format!(
            "runtime-state path changed during inspection: {}",
            path.display()
        )));
    }
    verify_parent(path, &parent)?;

    let revision = RuntimeStateSourceRevision::present_observed(
        path_identity,
        Arc::<[u8]>::from(bytes.into_boxed_slice()),
        before_fingerprint.identity,
    );
    let decoded = decode_runtime_ui_file(revision.bytes().expect("present revision"));
    Ok(RuntimeUiStateInspection {
        observation: RuntimeStateSourceObservation {
            revision,
            envelope: decoded.envelope,
        },
        status: decoded.status,
        supported_wire: decoded.supported_wire,
    })
}

pub(super) fn inspect_pinned(
    path: &PinnedPath,
) -> Result<RuntimeUiStateInspection, RuntimeStateInspectionError> {
    let reported_path = path.reported_path().map_err(|error| {
        RuntimeStateInspectionError::new(format!(
            "could not resolve pinned runtime-state path: {error}"
        ))
    })?;
    let mut file = match path.open_read() {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RuntimeUiStateInspection::missing(
                RuntimeStateSourceRevision::missing(RuntimeStatePathIdentity::direct(
                    reported_path,
                )),
            ));
        }
        Err(error) => return Err(inspection_error(&reported_path, error)),
    };
    let before = file
        .metadata()
        .map_err(|error| inspection_error(&reported_path, error))?;
    if !before.file_type().is_file() {
        return Err(RuntimeStateInspectionError::new(format!(
            "runtime-state path is not a regular file: {}",
            reported_path.display()
        )));
    }
    if before.len() > MAX_RUNTIME_UI_FILE_BYTES {
        return Err(RuntimeStateInspectionError::new(format!(
            "runtime-state file exceeds the {} byte limit",
            MAX_RUNTIME_UI_FILE_BYTES
        )));
    }

    let bytes = read_bounded(&reported_path, &mut file)?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| inspection_error(&reported_path, error))?;
    let verification_bytes = read_bounded(&reported_path, &mut file)?;
    let after = file
        .metadata()
        .map_err(|error| inspection_error(&reported_path, error))?;
    let active = path
        .open_read()
        .and_then(|file| file.metadata())
        .map_err(|error| inspection_error(&reported_path, error))?;
    if !active.file_type().is_file() {
        return Err(RuntimeStateInspectionError::new(format!(
            "runtime-state path changed type during inspection: {}",
            reported_path.display()
        )));
    }
    let before_fingerprint = file_fingerprint(&before);
    if verification_bytes != bytes
        || file_fingerprint(&after) != before_fingerprint
        || file_fingerprint(&active) != before_fingerprint
    {
        return Err(RuntimeStateInspectionError::new(format!(
            "runtime-state path changed during inspection: {}",
            reported_path.display()
        )));
    }

    let revision = RuntimeStateSourceRevision::present_observed(
        RuntimeStatePathIdentity::direct(reported_path),
        Arc::<[u8]>::from(bytes.into_boxed_slice()),
        before_fingerprint.identity,
    );
    let decoded = decode_runtime_ui_file(revision.bytes().expect("present revision"));
    Ok(RuntimeUiStateInspection {
        observation: RuntimeStateSourceObservation {
            revision,
            envelope: decoded.envelope,
        },
        status: decoded.status,
        supported_wire: decoded.supported_wire,
    })
}

#[derive(Debug)]
struct ResolvedParent {
    resolved_path: std::path::PathBuf,
    identity: RuntimeStateFileIdentity,
}

impl ResolvedParent {
    fn path_identity(&self, source_path: &Path) -> RuntimeStatePathIdentity {
        RuntimeStatePathIdentity::observed(
            source_path,
            RuntimeStateResolvedParent::new(self.resolved_path.clone(), self.identity),
        )
    }
}

fn resolve_parent(path: &Path) -> Result<Option<ResolvedParent>, RuntimeStateInspectionError> {
    let parent = source_parent(path);
    let resolved_path = match fs::canonicalize(parent) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(inspection_error(parent, error)),
    };
    let metadata =
        fs::metadata(&resolved_path).map_err(|error| inspection_error(&resolved_path, error))?;
    if !metadata.is_dir() {
        return Err(RuntimeStateInspectionError::new(format!(
            "runtime-state parent is not a directory: {}",
            resolved_path.display()
        )));
    }
    Ok(Some(ResolvedParent {
        resolved_path,
        identity: file_identity(&metadata),
    }))
}

fn verify_parent(
    path: &Path,
    expected: &ResolvedParent,
) -> Result<(), RuntimeStateInspectionError> {
    let Some(active) = resolve_parent(path)? else {
        return Err(RuntimeStateInspectionError::new(format!(
            "runtime-state parent disappeared during inspection: {}",
            source_parent(path).display()
        )));
    };
    if active.resolved_path != expected.resolved_path || active.identity != expected.identity {
        return Err(RuntimeStateInspectionError::new(format!(
            "runtime-state parent changed during inspection: {}",
            source_parent(path).display()
        )));
    }
    Ok(())
}

fn source_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn file_name(path: &Path) -> Result<&std::ffi::OsStr, RuntimeStateInspectionError> {
    path.file_name().ok_or_else(|| {
        RuntimeStateInspectionError::new(format!(
            "runtime-state path has no file name: {}",
            path.display()
        ))
    })
}

fn open_regular_nofollow(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK);
    options.open(path)
}

fn file_identity(metadata: &fs::Metadata) -> RuntimeStateFileIdentity {
    RuntimeStateFileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileFingerprint {
    identity: RuntimeStateFileIdentity,
    len: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

fn file_fingerprint(metadata: &fs::Metadata) -> FileFingerprint {
    FileFingerprint {
        identity: file_identity(metadata),
        len: metadata.len(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        changed_seconds: metadata.ctime(),
        changed_nanoseconds: metadata.ctime_nsec(),
    }
}

fn read_bounded(path: &Path, file: &mut File) -> Result<Vec<u8>, RuntimeStateInspectionError> {
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_RUNTIME_UI_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| inspection_error(path, error))?;
    if bytes.len() as u64 > MAX_RUNTIME_UI_FILE_BYTES {
        return Err(RuntimeStateInspectionError::new(format!(
            "runtime-state file exceeds the {} byte limit",
            MAX_RUNTIME_UI_FILE_BYTES
        )));
    }
    Ok(bytes)
}

fn inspection_error(path: &Path, error: io::Error) -> RuntimeStateInspectionError {
    RuntimeStateInspectionError::new(format!(
        "could not inspect runtime-state path {}: {error}",
        path.display()
    ))
}
