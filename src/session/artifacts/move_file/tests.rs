use crate::test_temp;

use super::*;
use crate::session::{SessionArtifactPaths, named_session_artifact_paths};

fn assert_moved_target_artifacts(
    artifacts: &SessionArtifactPaths,
    rotated_recovery: &Path,
    preserved: &Path,
    preserved_moved: &Path,
) {
    assert_eq!(fs::read(&artifacts.primary).unwrap(), b"primary");
    assert_eq!(fs::read(&artifacts.backup).unwrap(), b"backup");
    assert_eq!(fs::read(&artifacts.recovery).unwrap(), b"recovery");
    assert_eq!(fs::read(&artifacts.clear_marker).unwrap(), b"cleared");
    assert_eq!(fs::read(rotated_recovery).unwrap(), b"rotated");
    assert_eq!(fs::read(preserved).unwrap(), b"newer");
    assert_eq!(fs::read(preserved_moved).unwrap(), b"newer moved");
}

fn assert_moved_sources_removed(
    artifacts: &SessionArtifactPaths,
    rotated_recovery: &Path,
    preserved: &Path,
    preserved_moved: &Path,
) {
    assert!(!artifacts.primary.exists());
    assert!(!artifacts.backup.exists());
    assert!(!artifacts.recovery.exists());
    assert!(!artifacts.clear_marker.exists());
    assert!(!rotated_recovery.exists());
    assert!(!preserved.exists());
    assert!(!preserved_moved.exists());
}

fn assert_source_lock_was_not_moved(
    source_artifacts: &SessionArtifactPaths,
    target_artifacts: &SessionArtifactPaths,
) {
    assert_eq!(
        fs::read(&source_artifacts.lock).unwrap(),
        b"source lock",
        "source lock must not be moved as session data"
    );
    if target_artifacts.lock.exists() {
        assert_ne!(fs::read(&target_artifacts.lock).unwrap(), b"source lock");
    }
}

#[test]
fn move_named_session_non_lock_artifacts_moves_primary_and_sidecars_without_lock() {
    let temp = test_temp::tempdir().unwrap();
    let source = temp.path().join("lecture.wayscriber-session");
    let target = temp.path().join("archive.wayscriber-session");
    let source_artifacts = named_session_artifact_paths(&source);
    let target_artifacts = named_session_artifact_paths(&target);
    let rotated_recovery = PathBuf::from(format!("{}.old", source_artifacts.recovery.display()));
    let target_rotated_recovery =
        PathBuf::from(format!("{}.old", target_artifacts.recovery.display()));
    let preserved_suffix = ".v2-preserved-0123456789abcdef";
    let preserved_moved_suffix = ".v3-preserved-fedcba9876543210-moved-1";
    let preserved = session::append_path_suffix(&source, preserved_suffix);
    let preserved_moved = session::append_path_suffix(&source, preserved_moved_suffix);
    let target_preserved = session::append_path_suffix(&target, preserved_suffix);
    let target_preserved_moved = session::append_path_suffix(&target, preserved_moved_suffix);

    fs::write(&source_artifacts.primary, b"primary").unwrap();
    fs::write(&source_artifacts.backup, b"backup").unwrap();
    fs::write(&source_artifacts.recovery, b"recovery").unwrap();
    fs::write(&source_artifacts.clear_marker, b"cleared").unwrap();
    fs::write(&rotated_recovery, b"rotated").unwrap();
    fs::write(&source_artifacts.lock, b"source lock").unwrap();
    fs::write(&preserved, b"newer").unwrap();
    fs::write(&preserved_moved, b"newer moved").unwrap();

    let outcome = move_named_session_non_lock_artifacts(&source, &target).unwrap();

    assert_eq!(outcome.source, source);
    assert_eq!(outcome.target, target);
    assert_eq!(outcome.moved_artifacts, 7);
    assert_eq!(outcome.moved_artifact_paths.len(), 7);
    assert_moved_target_artifacts(
        &target_artifacts,
        &target_rotated_recovery,
        &target_preserved,
        &target_preserved_moved,
    );
    assert_moved_sources_removed(
        &source_artifacts,
        &rotated_recovery,
        &preserved,
        &preserved_moved,
    );
    assert_source_lock_was_not_moved(&source_artifacts, &target_artifacts);
}

#[test]
fn rollback_named_session_move_uses_exact_moved_artifacts_and_ignores_skipped_source_dirs() {
    let temp = test_temp::tempdir().unwrap();
    let source = temp.path().join("lecture.wayscriber-session");
    let target = temp.path().join("archive.wayscriber-session");
    let source_artifacts = named_session_artifact_paths(&source);
    let target_artifacts = named_session_artifact_paths(&target);
    fs::write(&source_artifacts.primary, b"primary").unwrap();
    fs::create_dir(&source_artifacts.backup).unwrap();

    let outcome = move_named_session_non_lock_artifacts(&source, &target).unwrap();

    assert_eq!(outcome.moved_artifacts, 1);
    assert_eq!(outcome.moved_artifact_paths.len(), 1);
    assert!(!source_artifacts.primary.exists());
    assert!(source_artifacts.backup.is_dir());
    assert_eq!(fs::read(&target_artifacts.primary).unwrap(), b"primary");

    rollback_named_session_non_lock_artifacts_move(&outcome).unwrap();

    assert_eq!(fs::read(&source_artifacts.primary).unwrap(), b"primary");
    assert!(source_artifacts.backup.is_dir());
    assert!(!target_artifacts.primary.exists());
}

#[test]
fn move_named_session_non_lock_artifacts_rejects_target_artifact_collision() {
    let temp = test_temp::tempdir().unwrap();
    let source = temp.path().join("lecture.wayscriber-session");
    let target = temp.path().join("archive.wayscriber-session");
    let source_artifacts = named_session_artifact_paths(&source);
    let target_artifacts = named_session_artifact_paths(&target);
    fs::write(&source_artifacts.primary, b"primary").unwrap();
    fs::write(&source_artifacts.backup, b"backup").unwrap();
    fs::write(&target_artifacts.clear_marker, b"cleared").unwrap();

    let err = move_named_session_non_lock_artifacts(&source, &target)
        .expect_err("target sidecar should block move");

    assert!(format!("{err:#}").contains("already has session artifacts"));
    assert_eq!(fs::read(&source_artifacts.primary).unwrap(), b"primary");
    assert_eq!(fs::read(&source_artifacts.backup).unwrap(), b"backup");
    assert!(!target_artifacts.primary.exists());
    assert_eq!(
        fs::read(&target_artifacts.clear_marker).unwrap(),
        b"cleared"
    );
}

#[test]
fn rename_artifact_no_replace_does_not_overwrite_existing_target() {
    let temp = test_temp::tempdir().unwrap();
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    fs::write(&source, b"source").unwrap();
    fs::write(&target, b"target").unwrap();

    let err =
        rename_artifact_no_replace(&source, &target).expect_err("target must not be overwritten");

    assert_eq!(err.kind(), ErrorKind::AlreadyExists);
    assert_eq!(fs::read(&source).unwrap(), b"source");
    assert_eq!(fs::read(&target).unwrap(), b"target");
}

#[cfg(unix)]
#[test]
fn move_named_session_non_lock_artifacts_rejects_symlink_source_sidecar_before_move() {
    use std::os::unix::fs::symlink;

    let temp = test_temp::tempdir().unwrap();
    let source = temp.path().join("lecture.wayscriber-session");
    let target = temp.path().join("archive.wayscriber-session");
    let source_artifacts = named_session_artifact_paths(&source);
    let backup_target = temp.path().join("backup-target");
    fs::write(&source_artifacts.primary, b"primary").unwrap();
    fs::write(&backup_target, b"backup").unwrap();
    symlink(&backup_target, &source_artifacts.backup).unwrap();

    let err = move_named_session_non_lock_artifacts(&source, &target)
        .expect_err("source sidecar symlink should reject move");

    assert!(format!("{err:#}").contains("symlink"));
    assert_eq!(fs::read(&source_artifacts.primary).unwrap(), b"primary");
    assert!(source_artifacts.backup.exists());
    assert!(!target.exists());
}
