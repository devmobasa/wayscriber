use anyhow::{Context, Result, anyhow};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::session::lock::{lock_exclusive, open_runtime_lock_file, unlock};
use crate::session::primary::open_session_artifact_for_read;

use super::{
    SessionArtifactPaths, named_session_artifact_paths, named_session_non_lock_artifact_paths,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedSessionMoveOutcome {
    pub source: PathBuf,
    pub target: PathBuf,
    pub moved_artifacts: usize,
    pub moved_artifact_paths: Vec<NamedSessionMovedArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedSessionMovedArtifact {
    pub source: PathBuf,
    pub target: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MoveArtifact {
    source: PathBuf,
    target: PathBuf,
    required: bool,
}

struct MoveLocks {
    first_path: PathBuf,
    first: File,
    second_path: PathBuf,
    second: File,
}

impl Drop for MoveLocks {
    fn drop(&mut self) {
        if let Err(err) = unlock(&self.second) {
            log::warn!(
                "failed to unlock named session move lock {}: {}",
                self.second_path.display(),
                err
            );
        }
        if let Err(err) = unlock(&self.first) {
            log::warn!(
                "failed to unlock named session move lock {}: {}",
                self.first_path.display(),
                err
            );
        }
    }
}

#[allow(dead_code)]
pub fn move_named_session_non_lock_artifacts(
    source: &Path,
    target: &Path,
) -> Result<NamedSessionMoveOutcome> {
    crate::session::validate_named_session_file_for_info(source)?;
    crate::session::validate_named_session_file_for_foreground(target)?;
    if crate::session::catalog::session_paths_match(source, target) {
        return Err(anyhow!(
            "Move Session target must be different from the source: {}",
            target.display()
        ));
    }

    open_session_artifact_for_read(source, true)
        .with_context(|| format!("failed to open move source {}", source.display()))
        .map(drop)?;
    ensure_target_has_no_artifacts(target)?;

    let source_artifacts = named_session_artifact_paths(source);
    let target_artifacts = named_session_artifact_paths(target);
    let _locks = acquire_move_locks(&source_artifacts.lock, &target_artifacts.lock)?;

    open_session_artifact_for_read(source, true)
        .with_context(|| format!("failed to open locked move source {}", source.display()))
        .map(drop)?;
    ensure_target_has_no_artifacts(target)?;

    let artifacts = collect_movable_artifacts(&source_artifacts, &target_artifacts)?;
    rename_artifacts_with_rollback(&artifacts)?;
    sync_move_parent_dirs(source, target);

    Ok(NamedSessionMoveOutcome {
        source: source.to_path_buf(),
        target: target.to_path_buf(),
        moved_artifacts: artifacts.len(),
        moved_artifact_paths: artifacts
            .iter()
            .map(NamedSessionMovedArtifact::from)
            .collect(),
    })
}

#[allow(dead_code)]
pub fn rollback_named_session_non_lock_artifacts_move(
    outcome: &NamedSessionMoveOutcome,
) -> Result<()> {
    let source_artifacts = named_session_artifact_paths(&outcome.source);
    let target_artifacts = named_session_artifact_paths(&outcome.target);
    let _locks = acquire_move_locks(&source_artifacts.lock, &target_artifacts.lock)?;
    let moved = outcome
        .moved_artifact_paths
        .iter()
        .map(MoveArtifact::from)
        .collect::<Vec<_>>();
    rollback_moved_artifacts(&moved)?;
    sync_move_parent_dirs(&outcome.source, &outcome.target);
    Ok(())
}

impl From<&MoveArtifact> for NamedSessionMovedArtifact {
    fn from(artifact: &MoveArtifact) -> Self {
        Self {
            source: artifact.source.clone(),
            target: artifact.target.clone(),
        }
    }
}

impl From<&NamedSessionMovedArtifact> for MoveArtifact {
    fn from(artifact: &NamedSessionMovedArtifact) -> Self {
        Self {
            source: artifact.source.clone(),
            target: artifact.target.clone(),
            required: false,
        }
    }
}

fn acquire_move_locks(source_lock: &Path, target_lock: &Path) -> Result<MoveLocks> {
    let (first_path, second_path) = if source_lock.as_os_str() <= target_lock.as_os_str() {
        (source_lock, target_lock)
    } else {
        (target_lock, source_lock)
    };

    let first = open_and_lock_named_lock(first_path)?;
    let second = open_and_lock_named_lock(second_path)?;
    Ok(MoveLocks {
        first_path: first_path.to_path_buf(),
        first,
        second_path: second_path.to_path_buf(),
        second,
    })
}

fn open_and_lock_named_lock(path: &Path) -> Result<File> {
    let file = open_runtime_lock_file(path, true)
        .with_context(|| format!("failed to open named session move lock {}", path.display()))?;
    lock_exclusive(&file)
        .with_context(|| format!("failed to lock named session move lock {}", path.display()))?;
    Ok(file)
}

fn collect_movable_artifacts(
    source_artifacts: &SessionArtifactPaths,
    target_artifacts: &SessionArtifactPaths,
) -> Result<Vec<MoveArtifact>> {
    let source_primary = source_artifacts.primary.as_path();
    let mut artifacts = Vec::new();
    for source in named_session_non_lock_artifact_paths(source_primary)? {
        let Some(target) =
            target_path_for_source_artifact(&source, source_artifacts, target_artifacts)
        else {
            continue;
        };
        let required = source == source_artifacts.primary;
        if movable_artifact_exists(&source, required)? {
            artifacts.push(MoveArtifact {
                source,
                target,
                required,
            });
        }
    }

    if !artifacts.iter().any(|artifact| artifact.required) {
        return Err(anyhow!(
            "Move Session source primary does not exist: {}",
            source_primary.display()
        ));
    }
    dedupe_move_artifacts(&mut artifacts);
    Ok(artifacts)
}

fn target_path_for_source_artifact(
    source: &Path,
    source_artifacts: &SessionArtifactPaths,
    target_artifacts: &SessionArtifactPaths,
) -> Option<PathBuf> {
    for (candidate_source, candidate_target) in [
        (&source_artifacts.primary, &target_artifacts.primary),
        (&source_artifacts.backup, &target_artifacts.backup),
        (
            &source_artifacts.backup_recovery_marker,
            &target_artifacts.backup_recovery_marker,
        ),
        (&source_artifacts.recovery, &target_artifacts.recovery),
        (
            &source_artifacts.recovery_recoverable_marker,
            &target_artifacts.recovery_recoverable_marker,
        ),
        (
            &source_artifacts.clear_marker,
            &target_artifacts.clear_marker,
        ),
    ] {
        if source == candidate_source {
            return Some(candidate_target.clone());
        }
    }

    target_recovery_variant_path(
        source,
        &source_artifacts.recovery,
        &target_artifacts.recovery,
    )
}

fn target_recovery_variant_path(
    source: &Path,
    source_recovery: &Path,
    target_recovery: &Path,
) -> Option<PathBuf> {
    if source.parent() != source_recovery.parent() {
        return None;
    }
    let source_name = source.file_name()?.to_str()?;
    let source_recovery_name = source_recovery.file_name()?.to_str()?;
    if !source_name.starts_with(&format!("{source_recovery_name}.")) {
        return None;
    }

    let target_recovery_name = target_recovery.file_name()?.to_str()?;
    let suffix = &source_name[source_recovery_name.len()..];
    let mut target_name = OsString::from(target_recovery_name);
    target_name.push(suffix);
    Some(match target_recovery.parent() {
        Some(parent) => parent.join(target_name),
        None => PathBuf::from(target_name),
    })
}

fn movable_artifact_exists(path: &Path, required: bool) -> Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound && !required => return Ok(false),
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return Err(anyhow!(
                "required session artifact is missing: {}",
                path.display()
            ));
        }
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to inspect session artifact {}", path.display()));
        }
    };

    if metadata.is_file() {
        return Ok(true);
    }
    if metadata.is_dir() && !required {
        return Ok(false);
    }
    let kind = if metadata.file_type().is_symlink() {
        "symlink"
    } else if metadata.is_dir() {
        "directory"
    } else {
        "special file"
    };
    Err(anyhow!(
        "Move Session source artifact is a {kind}, not a regular file: {}",
        path.display()
    ))
}

fn ensure_target_has_no_artifacts(target: &Path) -> Result<()> {
    for path in named_session_non_lock_artifact_paths(target)? {
        if artifact_exists(&path)? {
            return Err(anyhow!(
                "Move Session target already has session artifacts; choose a new path: {}",
                target.display()
            ));
        }
    }
    Ok(())
}

fn artifact_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err)
            .with_context(|| format!("failed to inspect session artifact {}", path.display())),
    }
}

fn rename_artifacts_with_rollback(artifacts: &[MoveArtifact]) -> Result<()> {
    let mut moved = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        match rename_artifact_no_replace(&artifact.source, &artifact.target) {
            Ok(()) => moved.push(artifact.clone()),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                rollback_moved_artifacts(&moved)?;
                return Err(anyhow!(
                    "Move Session target appeared during move: {}",
                    artifact.target.display()
                ));
            }
            Err(err) => {
                let rollback = rollback_moved_artifacts(&moved);
                return match rollback {
                    Ok(()) => Err(err).with_context(|| {
                        format!(
                            "failed to move session artifact {} -> {}; rolled back moved artifacts",
                            artifact.source.display(),
                            artifact.target.display()
                        )
                    }),
                    Err(rollback_err) => Err(err).with_context(|| {
                        format!(
                            "partial Move Session failure: failed to move session artifact {} -> {}, and rollback also failed: {rollback_err:#}",
                            artifact.source.display(),
                            artifact.target.display()
                        )
                    }),
                };
            }
        }
    }
    Ok(())
}

fn rollback_moved_artifacts(moved: &[MoveArtifact]) -> Result<()> {
    for artifact in moved.iter().rev() {
        rename_artifact_no_replace(&artifact.target, &artifact.source).with_context(|| {
            format!(
                "failed to roll back moved session artifact {} -> {}",
                artifact.target.display(),
                artifact.source.display()
            )
        })?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn rename_artifact_no_replace(source: &Path, target: &Path) -> io::Result<()> {
    let source = path_to_cstring(source)?;
    let target = path_to_cstring(target)?;
    // SAFETY: The C strings are valid, NUL-terminated paths. AT_FDCWD makes both paths
    // relative to the current process, matching std::fs::rename path semantics.
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
fn rename_artifact_no_replace(source: &Path, target: &Path) -> io::Result<()> {
    match fs::symlink_metadata(target) {
        Ok(_) => return Err(io::Error::from(ErrorKind::AlreadyExists)),
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    fs::rename(source, target)
}

fn sync_move_parent_dirs(source: &Path, target: &Path) {
    let mut parents = Vec::new();
    for path in [source, target] {
        if let Some(parent) = path.parent()
            && !parents.contains(&parent)
        {
            parents.push(parent);
        }
    }

    for parent in parents {
        if let Err(err) = sync_parent_dir(parent) {
            log::warn!(
                "Moved named session artifacts involving {}, but syncing the directory failed: {}",
                parent.display(),
                err
            );
        }
    }
}

#[cfg(unix)]
fn sync_parent_dir(parent: &Path) -> Result<()> {
    let dir = File::open(parent)
        .with_context(|| format!("failed to open session move directory {}", parent.display()))?;
    dir.sync_all()
        .with_context(|| format!("failed to sync session move directory {}", parent.display()))
}

#[cfg(not(unix))]
fn sync_parent_dir(_parent: &Path) -> Result<()> {
    Ok(())
}

fn dedupe_move_artifacts(artifacts: &mut Vec<MoveArtifact>) {
    let mut deduped = Vec::with_capacity(artifacts.len());
    for artifact in artifacts.drain(..) {
        if !deduped
            .iter()
            .any(|existing: &MoveArtifact| existing.source == artifact.source)
        {
            deduped.push(artifact);
        }
    }
    *artifacts = deduped;
}

#[cfg(test)]
mod tests;
