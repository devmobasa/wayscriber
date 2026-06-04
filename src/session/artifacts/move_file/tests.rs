use super::*;
use crate::session::named_session_artifact_paths;

#[test]
fn move_named_session_non_lock_artifacts_moves_primary_and_sidecars_without_lock() {
    let temp = crate::test_temp::tempdir().unwrap();
    let source = temp.path().join("lecture.wayscriber-session");
    let target = temp.path().join("archive.wayscriber-session");
    let source_artifacts = named_session_artifact_paths(&source);
    let target_artifacts = named_session_artifact_paths(&target);
    let rotated_recovery = PathBuf::from(format!("{}.old", source_artifacts.recovery.display()));
    let target_rotated_recovery =
        PathBuf::from(format!("{}.old", target_artifacts.recovery.display()));

    std::fs::write(&source_artifacts.primary, b"primary").unwrap();
    std::fs::write(&source_artifacts.backup, b"backup").unwrap();
    std::fs::write(&source_artifacts.recovery, b"recovery").unwrap();
    std::fs::write(&source_artifacts.clear_marker, b"cleared").unwrap();
    std::fs::write(&rotated_recovery, b"rotated").unwrap();
    std::fs::write(&source_artifacts.lock, b"source lock").unwrap();

    let outcome = move_named_session_non_lock_artifacts(&source, &target).unwrap();

    assert_eq!(outcome.source, source);
    assert_eq!(outcome.target, target);
    assert_eq!(outcome.moved_artifacts, 5);
    assert_eq!(outcome.moved_artifact_paths.len(), 5);
    assert_eq!(
        std::fs::read(&target_artifacts.primary).unwrap(),
        b"primary"
    );
    assert_eq!(std::fs::read(&target_artifacts.backup).unwrap(), b"backup");
    assert_eq!(
        std::fs::read(&target_artifacts.recovery).unwrap(),
        b"recovery"
    );
    assert_eq!(
        std::fs::read(&target_artifacts.clear_marker).unwrap(),
        b"cleared"
    );
    assert_eq!(std::fs::read(&target_rotated_recovery).unwrap(), b"rotated");
    assert!(!source_artifacts.primary.exists());
    assert!(!source_artifacts.backup.exists());
    assert!(!source_artifacts.recovery.exists());
    assert!(!source_artifacts.clear_marker.exists());
    assert!(!rotated_recovery.exists());
    assert_eq!(
        std::fs::read(&source_artifacts.lock).unwrap(),
        b"source lock",
        "source lock must not be moved as session data"
    );
    if target_artifacts.lock.exists() {
        assert_ne!(
            std::fs::read(&target_artifacts.lock).unwrap(),
            b"source lock"
        );
    }
}

#[test]
fn rollback_named_session_move_uses_exact_moved_artifacts_and_ignores_skipped_source_dirs() {
    let temp = crate::test_temp::tempdir().unwrap();
    let source = temp.path().join("lecture.wayscriber-session");
    let target = temp.path().join("archive.wayscriber-session");
    let source_artifacts = named_session_artifact_paths(&source);
    let target_artifacts = named_session_artifact_paths(&target);
    std::fs::write(&source_artifacts.primary, b"primary").unwrap();
    std::fs::create_dir(&source_artifacts.backup).unwrap();

    let outcome = move_named_session_non_lock_artifacts(&source, &target).unwrap();

    assert_eq!(outcome.moved_artifacts, 1);
    assert_eq!(outcome.moved_artifact_paths.len(), 1);
    assert!(!source_artifacts.primary.exists());
    assert!(source_artifacts.backup.is_dir());
    assert_eq!(
        std::fs::read(&target_artifacts.primary).unwrap(),
        b"primary"
    );

    rollback_named_session_non_lock_artifacts_move(&outcome).unwrap();

    assert_eq!(
        std::fs::read(&source_artifacts.primary).unwrap(),
        b"primary"
    );
    assert!(source_artifacts.backup.is_dir());
    assert!(!target_artifacts.primary.exists());
}

#[test]
fn move_named_session_non_lock_artifacts_rejects_target_artifact_collision() {
    let temp = crate::test_temp::tempdir().unwrap();
    let source = temp.path().join("lecture.wayscriber-session");
    let target = temp.path().join("archive.wayscriber-session");
    let source_artifacts = named_session_artifact_paths(&source);
    let target_artifacts = named_session_artifact_paths(&target);
    std::fs::write(&source_artifacts.primary, b"primary").unwrap();
    std::fs::write(&source_artifacts.backup, b"backup").unwrap();
    std::fs::write(&target_artifacts.clear_marker, b"cleared").unwrap();

    let err = move_named_session_non_lock_artifacts(&source, &target)
        .expect_err("target sidecar should block move");

    assert!(format!("{err:#}").contains("already has session artifacts"));
    assert_eq!(
        std::fs::read(&source_artifacts.primary).unwrap(),
        b"primary"
    );
    assert_eq!(std::fs::read(&source_artifacts.backup).unwrap(), b"backup");
    assert!(!target_artifacts.primary.exists());
    assert_eq!(
        std::fs::read(&target_artifacts.clear_marker).unwrap(),
        b"cleared"
    );
}

#[test]
fn rename_artifact_no_replace_does_not_overwrite_existing_target() {
    let temp = crate::test_temp::tempdir().unwrap();
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    std::fs::write(&source, b"source").unwrap();
    std::fs::write(&target, b"target").unwrap();

    let err =
        rename_artifact_no_replace(&source, &target).expect_err("target must not be overwritten");

    assert_eq!(err.kind(), ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(&source).unwrap(), b"source");
    assert_eq!(std::fs::read(&target).unwrap(), b"target");
}

#[cfg(unix)]
#[test]
fn move_named_session_non_lock_artifacts_rejects_symlink_source_sidecar_before_move() {
    use std::os::unix::fs::symlink;

    let temp = crate::test_temp::tempdir().unwrap();
    let source = temp.path().join("lecture.wayscriber-session");
    let target = temp.path().join("archive.wayscriber-session");
    let source_artifacts = named_session_artifact_paths(&source);
    let backup_target = temp.path().join("backup-target");
    std::fs::write(&source_artifacts.primary, b"primary").unwrap();
    std::fs::write(&backup_target, b"backup").unwrap();
    symlink(&backup_target, &source_artifacts.backup).unwrap();

    let err = move_named_session_non_lock_artifacts(&source, &target)
        .expect_err("source sidecar symlink should reject move");

    assert!(format!("{err:#}").contains("symlink"));
    assert_eq!(
        std::fs::read(&source_artifacts.primary).unwrap(),
        b"primary"
    );
    assert!(source_artifacts.backup.exists());
    assert!(!target.exists());
}
