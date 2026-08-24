use super::*;
use std::cell::RefCell;
use std::os::unix::fs::PermissionsExt;

fn staging_directories(parent: &Path) -> Vec<PathBuf> {
    fs::read_dir(parent)
        .expect("read test directory")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(SAVE_AS_STAGING_PREFIX))
        })
        .collect()
}

#[test]
fn quarantine_validation_failure_preserves_every_sidecar() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let session = temp.path().join("target.wayscriber-session");
    let backup = temp.path().join("target.wayscriber-session.bak");
    let recovery = temp.path().join("target.wayscriber-session.recovery");
    fs::write(&backup, b"backup").expect("write backup");
    fs::create_dir(&recovery).expect("create invalid recovery directory");

    let err = quarantine_save_as_sidecars(&[backup.clone(), recovery.clone()], &session)
        .expect_err("sidecar directory must fail validation");

    assert!(format!("{err:#}").contains("refusing to replace"));
    assert_eq!(fs::read(&backup).expect("backup preserved"), b"backup");
    assert!(recovery.is_dir());
    assert!(staging_directories(temp.path()).is_empty());
}

#[test]
fn primary_rename_failure_restores_quarantined_sidecars() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let session = temp.path().join("target.wayscriber-session");
    let tmp = temp.path().join("target.tmp");
    let backup = temp.path().join("target.wayscriber-session.bak");
    let recovery = temp.path().join("target.wayscriber-session.recovery");
    fs::write(&session, b"old primary").expect("write primary");
    fs::write(&tmp, b"new primary").expect("write temporary primary");
    fs::write(&backup, b"backup").expect("write backup");
    fs::write(&recovery, b"recovery").expect("write recovery");

    let err = commit_save_as_target_with(
        &tmp,
        &session,
        &[backup.clone(), recovery.clone()],
        |_, _| Err(std::io::Error::other("injected primary rename failure")),
        || -> Result<()> { panic!("sync must not run before primary replacement") },
        |_| -> Result<()> { panic!("cleanup must not run before primary replacement") },
    )
    .expect_err("injected rename failure");

    assert!(format!("{err:#}").contains("restored stale sidecars"));
    assert_eq!(
        fs::read(&session).expect("old primary preserved"),
        b"old primary"
    );
    assert_eq!(
        fs::read(&tmp).expect("temporary primary preserved"),
        b"new primary"
    );
    assert_eq!(fs::read(&backup).expect("backup restored"), b"backup");
    assert_eq!(fs::read(&recovery).expect("recovery restored"), b"recovery");
    assert!(staging_directories(temp.path()).is_empty());
}

#[test]
fn successful_commit_removes_quarantined_sidecars() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let session = temp.path().join("target.wayscriber-session");
    let tmp = temp.path().join("target.tmp");
    let backup = temp.path().join("target.wayscriber-session.bak");
    let recovery = temp.path().join("target.wayscriber-session.recovery");
    fs::write(&session, b"old primary").expect("write primary");
    fs::write(&tmp, b"new primary").expect("write temporary primary");
    fs::write(&backup, b"backup").expect("write backup");
    fs::write(&recovery, b"recovery").expect("write recovery");

    commit_save_as_target(&tmp, &session, &[backup.clone(), recovery.clone()])
        .expect("commit Save As target");

    assert_eq!(
        fs::read(&session).expect("new primary committed"),
        b"new primary"
    );
    assert!(!tmp.exists());
    assert!(!backup.exists());
    assert!(!recovery.exists());
    assert!(staging_directories(temp.path()).is_empty());
}

#[test]
fn postcommit_cleanup_failure_keeps_recovery_bytes_quarantined() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let session = temp.path().join("target.wayscriber-session");
    let tmp = temp.path().join("target.tmp");
    let recovery = temp.path().join("target.wayscriber-session.recovery");
    fs::write(&session, b"old primary").expect("write primary");
    fs::write(&tmp, b"new primary").expect("write temporary primary");
    fs::write(&recovery, b"recovery").expect("write recovery");
    let staged_directory = RefCell::new(None);

    commit_save_as_target_with(
        &tmp,
        &session,
        std::slice::from_ref(&recovery),
        |source, target| fs::rename(source, target),
        || Ok(()),
        |quarantine| {
            *staged_directory.borrow_mut() = quarantine.directory().map(Path::to_path_buf);
            Err(anyhow!("injected postcommit cleanup failure"))
        },
    )
    .expect("cleanup failure must not roll back a committed primary");

    assert_eq!(
        fs::read(&session).expect("new primary committed"),
        b"new primary"
    );
    assert!(!recovery.exists());
    let staged_directory = staged_directory
        .into_inner()
        .expect("quarantine directory recorded");
    assert_eq!(
        fs::metadata(&staged_directory)
            .expect("quarantine metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let staged_files = fs::read_dir(&staged_directory)
        .expect("quarantine retained")
        .map(|entry| entry.expect("staged entry").path())
        .collect::<Vec<_>>();
    assert_eq!(staged_files.len(), 1);
    assert!(
        staged_files[0]
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains("target.wayscriber-session.recovery")),
        "quarantined files should retain their original name for manual recovery"
    );
    assert_eq!(
        fs::read(&staged_files[0]).expect("recovery retained"),
        b"recovery"
    );
}
