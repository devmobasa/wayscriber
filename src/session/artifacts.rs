use anyhow::{Context, Result, anyhow};
use std::fs;
use std::io::{self, ErrorKind};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use super::options::append_path_suffix;
use super::primary::open_session_artifact_for_read;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionArtifactPaths {
    pub primary: PathBuf,
    pub backup: PathBuf,
    pub backup_recovery_marker: PathBuf,
    pub recovery: PathBuf,
    pub recovery_recoverable_marker: PathBuf,
    pub clear_marker: PathBuf,
    pub lock: PathBuf,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NamedSessionClearOutcome {
    pub removed_primary: bool,
    pub removed_backup: bool,
    pub removed_recovery: bool,
    pub removed_clear_marker: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedSessionDuplicateOutcome {
    pub target: PathBuf,
    pub bytes_copied: u64,
}

impl NamedSessionClearOutcome {
    #[allow(dead_code)]
    pub fn removed_any(self) -> bool {
        self.removed_primary
            || self.removed_backup
            || self.removed_recovery
            || self.removed_clear_marker
    }
}

#[allow(dead_code)]
pub fn named_session_artifact_paths(path: &Path) -> SessionArtifactPaths {
    SessionArtifactPaths {
        primary: path.to_path_buf(),
        backup: append_path_suffix(path, ".bak"),
        backup_recovery_marker: append_path_suffix(path, ".bak.recoverable"),
        recovery: append_path_suffix(path, ".recovery"),
        recovery_recoverable_marker: append_path_suffix(path, ".recovery.recoverable"),
        clear_marker: append_path_suffix(path, ".cleared"),
        lock: append_path_suffix(path, ".lock"),
    }
}

#[allow(dead_code)]
pub fn named_session_non_lock_artifact_paths(path: &Path) -> Result<Vec<PathBuf>> {
    let artifacts = named_session_artifact_paths(path);
    let mut paths = vec![
        artifacts.primary,
        artifacts.backup,
        artifacts.backup_recovery_marker,
        artifacts.recovery.clone(),
        artifacts.recovery_recoverable_marker,
        artifacts.clear_marker,
    ];
    collect_recovery_variants(&artifacts.recovery, &mut paths)?;
    dedupe_paths(&mut paths);
    Ok(paths)
}

#[allow(dead_code)]
pub fn clear_named_session_non_lock_artifacts(path: &Path) -> Result<NamedSessionClearOutcome> {
    crate::session::validate_named_session_file_for_clear(path)?;

    let artifacts = named_session_artifact_paths(path);
    let recovery_artifacts = removable_recovery_artifact_paths(&artifacts.recovery)?;
    let removed_primary = remove_artifact_if_exists(&artifacts.primary)?;
    let removed_clear_marker = remove_artifact_if_exists(&artifacts.clear_marker)?;
    let removed_backup = remove_artifact_if_exists(&artifacts.backup)?;
    let removed_backup_marker = remove_artifact_if_exists(&artifacts.backup_recovery_marker)?;
    let removed_recovery = remove_artifacts(&recovery_artifacts)?;
    let removed_recovery_marker =
        remove_artifact_if_exists(&artifacts.recovery_recoverable_marker)?;

    Ok(NamedSessionClearOutcome {
        removed_primary,
        removed_backup: removed_backup || removed_backup_marker,
        removed_recovery: removed_recovery || removed_recovery_marker,
        removed_clear_marker,
    })
}

#[allow(dead_code)]
pub fn duplicate_named_session_primary(
    source: &Path,
    target: &Path,
) -> Result<NamedSessionDuplicateOutcome> {
    crate::session::validate_named_session_file_for_info(source)?;
    crate::session::validate_named_session_file_for_foreground(target)?;
    if crate::session::catalog::session_paths_match(source, target) {
        return Err(anyhow!(
            "Duplicate Session target must be different from the source: {}",
            target.display()
        ));
    }

    let mut source_file = open_session_artifact_for_read(source, true)
        .with_context(|| format!("failed to open duplicate source {}", source.display()))?;
    let source_metadata = source_file
        .metadata()
        .with_context(|| format!("failed to inspect duplicate source {}", source.display()))?;

    ensure_duplicate_target_has_no_artifacts(target)?;

    let mut target_file = create_duplicate_target_file(target, &source_metadata)?;
    let copy_result = (|| {
        let bytes_copied = io::copy(&mut source_file, &mut target_file).with_context(|| {
            format!(
                "failed to copy duplicate session primary {} -> {}",
                source.display(),
                target.display()
            )
        })?;
        target_file
            .sync_all()
            .with_context(|| format!("failed to sync duplicate target {}", target.display()))?;
        #[cfg(not(unix))]
        target_file
            .set_permissions(source_metadata.permissions())
            .with_context(|| {
                format!(
                    "failed to preserve duplicate target permissions for {}",
                    target.display()
                )
            })?;
        Ok::<u64, anyhow::Error>(bytes_copied)
    })();
    drop(target_file);

    let bytes_copied = match copy_result {
        Ok(bytes_copied) => bytes_copied,
        Err(err) => {
            let _ = fs::remove_file(target);
            return Err(err);
        }
    };

    if let Err(err) = sync_duplicate_parent(target) {
        log::warn!(
            "Duplicated named session primary to {}, but syncing its parent directory failed: {}",
            target.display(),
            err
        );
    }

    Ok(NamedSessionDuplicateOutcome {
        target: target.to_path_buf(),
        bytes_copied,
    })
}

#[allow(dead_code)]
fn collect_recovery_variants(recovery_path: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    let Some(recovery_name) = recovery_path.file_name().and_then(|name| name.to_str()) else {
        return Ok(());
    };
    let Some(parent) = recovery_path.parent() else {
        return Ok(());
    };
    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(err).with_context(|| {
                format!(
                    "failed to scan named session recovery artifacts under {}",
                    parent.display()
                )
            });
        }
    };

    let recovery_prefix = format!("{recovery_name}.");
    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "failed to inspect named session recovery artifacts under {}",
                parent.display()
            )
        })?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name == recovery_name || name.starts_with(&recovery_prefix) {
            paths.push(path);
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn removable_recovery_artifact_paths(recovery_path: &Path) -> Result<Vec<PathBuf>> {
    let Some(recovery_name) = recovery_path.file_name().and_then(|name| name.to_str()) else {
        return removable_artifact_path(recovery_path).map(|path| path.into_iter().collect());
    };
    let Some(parent) = recovery_path.parent() else {
        return removable_artifact_path(recovery_path).map(|path| path.into_iter().collect());
    };

    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return removable_artifact_path(recovery_path).map(|path| path.into_iter().collect());
        }
        Err(err) => {
            return Err(err).with_context(|| {
                format!(
                    "failed to scan named session recovery artifacts under {}",
                    parent.display()
                )
            });
        }
    };

    let recovery_prefix = format!("{recovery_name}.");
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "failed to inspect named session recovery artifacts under {}",
                parent.display()
            )
        })?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if (name == recovery_name || name.starts_with(&recovery_prefix))
            && let Some(path) = removable_artifact_path(&path)?
        {
            paths.push(path);
        }
    }
    dedupe_paths(&mut paths);
    Ok(paths)
}

#[allow(dead_code)]
fn remove_artifacts(paths: &[PathBuf]) -> Result<bool> {
    let mut removed = false;
    for path in paths {
        removed = remove_artifact_if_exists(path)? || removed;
    }
    Ok(removed)
}

#[allow(dead_code)]
fn removable_artifact_path(path: &Path) -> Result<Option<PathBuf>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(None),
        Ok(_) => Ok(Some(path.to_path_buf())),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err)
            .with_context(|| format!("failed to inspect session artifact {}", path.display())),
    }
}

#[allow(dead_code)]
fn remove_artifact_if_exists(path: &Path) -> Result<bool> {
    match removable_artifact_path(path)? {
        Some(_) => {}
        None => return Ok(false),
    }

    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err)
            .with_context(|| format!("failed to remove session artifact {}", path.display())),
    }
}

fn ensure_duplicate_target_has_no_artifacts(target: &Path) -> Result<()> {
    for path in named_session_non_lock_artifact_paths(target)? {
        if artifact_exists(&path)? {
            return Err(anyhow!(
                "Duplicate Session target already has session artifacts; choose a new path: {}",
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

fn create_duplicate_target_file(target: &Path, source_metadata: &fs::Metadata) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options
        .mode(source_metadata.permissions().mode() & 0o777)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);

    options.open(target).with_context(|| {
        format!(
            "failed to create duplicate session target {}",
            target.display()
        )
    })
}

#[cfg(unix)]
fn sync_duplicate_parent(target: &Path) -> Result<()> {
    let Some(parent) = target.parent() else {
        return Ok(());
    };
    let dir = fs::File::open(parent).with_context(|| {
        format!(
            "failed to open duplicate target directory {}",
            parent.display()
        )
    })?;
    dir.sync_all().with_context(|| {
        format!(
            "failed to sync duplicate target directory {}",
            parent.display()
        )
    })
}

#[cfg(not(unix))]
fn sync_duplicate_parent(_target: &Path) -> Result<()> {
    Ok(())
}

#[allow(dead_code)]
fn dedupe_paths(paths: &mut Vec<PathBuf>) {
    let mut deduped = Vec::with_capacity(paths.len());
    for path in paths.drain(..) {
        if !deduped.contains(&path) {
            deduped.push(path);
        }
    }
    *paths = deduped;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_session_artifact_paths_use_exact_primary_suffixes() {
        let path = Path::new("/tmp/lecture.wayscriber-session");
        let artifacts = named_session_artifact_paths(path);

        assert_eq!(artifacts.primary, path);
        assert_eq!(
            artifacts.backup,
            PathBuf::from("/tmp/lecture.wayscriber-session.bak")
        );
        assert_eq!(
            artifacts.clear_marker,
            PathBuf::from("/tmp/lecture.wayscriber-session.cleared")
        );
        assert_eq!(
            artifacts.lock,
            PathBuf::from("/tmp/lecture.wayscriber-session.lock")
        );
    }

    #[test]
    fn clear_named_session_non_lock_artifacts_preserves_lock_and_sibling() {
        let temp = crate::test_temp::tempdir().unwrap();
        let path = temp.path().join("lecture.wayscriber-session");
        let sibling = temp.path().join("other.wayscriber-session");
        let artifacts = named_session_artifact_paths(&path);
        let sibling_artifacts = named_session_artifact_paths(&sibling);

        for path in [
            &artifacts.primary,
            &artifacts.backup,
            &artifacts.backup_recovery_marker,
            &artifacts.recovery,
            &artifacts.recovery_recoverable_marker,
            &artifacts.clear_marker,
            &artifacts.lock,
            &sibling_artifacts.primary,
            &sibling_artifacts.backup,
            &sibling_artifacts.lock,
        ] {
            std::fs::write(path, b"artifact").unwrap();
        }
        let rotated_recovery = PathBuf::from(format!("{}.old", artifacts.recovery.display()));
        std::fs::write(&rotated_recovery, b"rotated").unwrap();

        let outcome = clear_named_session_non_lock_artifacts(&path).unwrap();

        assert!(outcome.removed_primary);
        assert!(outcome.removed_backup);
        assert!(outcome.removed_recovery);
        assert!(outcome.removed_clear_marker);
        for path in [
            &artifacts.primary,
            &artifacts.backup,
            &artifacts.backup_recovery_marker,
            &artifacts.recovery,
            &artifacts.recovery_recoverable_marker,
            &artifacts.clear_marker,
            &rotated_recovery,
        ] {
            assert!(!path.exists(), "{} should be removed", path.display());
        }
        assert!(artifacts.lock.exists(), "lock artifact must be preserved");
        assert!(sibling_artifacts.primary.exists());
        assert!(sibling_artifacts.backup.exists());
        assert!(sibling_artifacts.lock.exists());
    }

    #[test]
    fn clear_named_session_non_lock_artifacts_skips_directory_sidecars() {
        let temp = crate::test_temp::tempdir().unwrap();
        let path = temp.path().join("lecture.wayscriber-session");
        let artifacts = named_session_artifact_paths(&path);
        let recovery_dir = PathBuf::from(format!("{}.old", artifacts.recovery.display()));

        std::fs::write(&artifacts.primary, b"primary").unwrap();
        std::fs::create_dir(&artifacts.backup).unwrap();
        std::fs::create_dir(&recovery_dir).unwrap();

        let outcome = clear_named_session_non_lock_artifacts(&path).unwrap();

        assert!(outcome.removed_primary);
        assert!(!outcome.removed_backup);
        assert!(!outcome.removed_recovery);
        assert!(!artifacts.primary.exists());
        assert!(
            artifacts.backup.is_dir(),
            "backup directory should not be removed as a file artifact"
        );
        assert!(
            recovery_dir.is_dir(),
            "recovery directory should not be removed as a file artifact"
        );
    }

    #[test]
    fn duplicate_named_session_primary_copies_only_primary() {
        let temp = crate::test_temp::tempdir().unwrap();
        let source = temp.path().join("lecture.wayscriber-session");
        let target = temp.path().join("lecture-copy.wayscriber-session");
        let source_artifacts = named_session_artifact_paths(&source);
        let target_artifacts = named_session_artifact_paths(&target);

        std::fs::write(&source_artifacts.primary, b"primary").unwrap();
        std::fs::write(&source_artifacts.backup, b"backup").unwrap();
        std::fs::write(&source_artifacts.recovery, b"recovery").unwrap();
        std::fs::write(&source_artifacts.clear_marker, b"cleared").unwrap();
        std::fs::write(&source_artifacts.lock, b"lock").unwrap();

        let outcome = duplicate_named_session_primary(&source, &target).unwrap();

        assert_eq!(outcome.target, target);
        assert_eq!(outcome.bytes_copied, "primary".len() as u64);
        assert_eq!(
            std::fs::read(&target_artifacts.primary).unwrap(),
            b"primary"
        );
        for path in [
            &target_artifacts.backup,
            &target_artifacts.recovery,
            &target_artifacts.clear_marker,
            &target_artifacts.lock,
        ] {
            assert!(
                !path.exists(),
                "duplicate should not create {}",
                path.display()
            );
        }
        assert!(source_artifacts.backup.exists());
        assert!(source_artifacts.recovery.exists());
        assert!(source_artifacts.clear_marker.exists());
        assert!(source_artifacts.lock.exists());
    }

    #[test]
    fn duplicate_named_session_primary_rejects_existing_target_sidecar() {
        let temp = crate::test_temp::tempdir().unwrap();
        let source = temp.path().join("lecture.wayscriber-session");
        let target = temp.path().join("lecture-copy.wayscriber-session");
        let target_artifacts = named_session_artifact_paths(&target);
        std::fs::write(&source, b"primary").unwrap();
        std::fs::write(&target_artifacts.backup, b"backup").unwrap();

        let err = duplicate_named_session_primary(&source, &target)
            .expect_err("target sidecar should block duplicate");

        assert!(format!("{err:#}").contains("already has session artifacts"));
        assert!(
            !target.exists(),
            "duplicate should not create target primary"
        );
        assert!(target_artifacts.backup.exists());
    }

    #[cfg(unix)]
    #[test]
    fn duplicate_named_session_primary_rejects_symlink_source() {
        use std::os::unix::fs::symlink;

        let temp = crate::test_temp::tempdir().unwrap();
        let target_source = temp.path().join("real.wayscriber-session");
        let source = temp.path().join("link.wayscriber-session");
        let target = temp.path().join("copy.wayscriber-session");
        std::fs::write(&target_source, b"primary").unwrap();
        symlink(&target_source, &source).unwrap();

        let err = duplicate_named_session_primary(&source, &target)
            .expect_err("symlink source should reject duplicate");

        assert!(format!("{err:#}").contains("symlink"));
        assert!(!target.exists());
    }
}
