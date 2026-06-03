use anyhow::{Context, Result};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::options::append_path_suffix;

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
}
