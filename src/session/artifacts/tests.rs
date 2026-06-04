use std::path::{Path, PathBuf};

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
