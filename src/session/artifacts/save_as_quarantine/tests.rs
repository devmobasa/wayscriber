use super::*;

fn staged_file(directory: &Path, original_name: &str) -> PathBuf {
    fs::read_dir(directory)
        .expect("read quarantine")
        .map(|entry| entry.expect("quarantine entry").path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(original_name))
        })
        .expect("matching staged sidecar")
}

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
fn quarantine_directory_sync_failure_rolls_back_before_commit() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let session = temp.path().join("target.wayscriber-session");
    let recovery = temp.path().join("target.wayscriber-session.recovery");
    fs::write(&recovery, b"recovery").expect("write recovery");

    let err = quarantine_save_as_sidecars_with(
        std::slice::from_ref(&recovery),
        &session,
        |_| Err(anyhow!("injected quarantine directory sync failure")),
        |_| -> Result<()> { panic!("parent sync must not run after quarantine sync failure") },
    )
    .expect_err("quarantine sync failure must abort before commit");

    assert!(format!("{err:#}").contains("restored sidecars"));
    assert_eq!(fs::read(&recovery).expect("recovery restored"), b"recovery");
    assert!(staging_directories(temp.path()).is_empty());
}

#[test]
fn quarantine_parent_sync_failure_rolls_back_before_commit() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let session = temp.path().join("target.wayscriber-session");
    let backup = temp.path().join("target.wayscriber-session.bak");
    let recovery = temp.path().join("target.wayscriber-session.recovery");
    fs::write(&backup, b"backup").expect("write backup");
    fs::write(&recovery, b"recovery").expect("write recovery");

    let err = quarantine_save_as_sidecars_with(
        &[backup.clone(), recovery.clone()],
        &session,
        |_| Ok(()),
        |_| Err(anyhow!("injected quarantine parent sync failure")),
    )
    .expect_err("parent sync failure must abort before commit");

    assert!(format!("{err:#}").contains("restored sidecars"));
    assert_eq!(fs::read(&backup).expect("backup restored"), b"backup");
    assert_eq!(fs::read(&recovery).expect("recovery restored"), b"recovery");
    assert!(staging_directories(temp.path()).is_empty());
}

#[test]
fn cleanup_failure_after_recovery_deletion_restores_every_sidecar() {
    let temp = crate::test_temp::tempdir().expect("tempdir");
    let session = temp.path().join("target.wayscriber-session");
    let recovery = temp.path().join("target.wayscriber-session.recovery");
    let backup = temp.path().join("target.wayscriber-session.bak");
    fs::write(&recovery, b"recovery").expect("write recovery");
    fs::write(&backup, b"backup").expect("write backup");

    let quarantine = quarantine_save_as_sidecars(&[recovery.clone(), backup.clone()], &session)
        .expect("quarantine sidecars");
    let directory = quarantine
        .directory()
        .expect("quarantine directory")
        .to_path_buf();
    let mut removals = 0;

    let err = quarantine
        .remove_after_commit_with(
            |path| {
                removals += 1;
                if removals == 1 {
                    fs::remove_file(path)
                } else {
                    Err(std::io::Error::other("injected second deletion failure"))
                }
            },
            |path| fs::remove_dir(path),
            |path| crate::durable_io::sync_parent_dir(path).map_err(Into::into),
        )
        .expect_err("partial cleanup must report failure");

    assert!(format!("{err:#}").contains("recovery bytes retained"));
    assert_eq!(
        fs::read(staged_file(
            &directory,
            "target.wayscriber-session.recovery"
        ))
        .expect("recovery retained"),
        b"recovery"
    );
    assert_eq!(
        fs::read(staged_file(&directory, "target.wayscriber-session.bak"))
            .expect("backup retained"),
        b"backup"
    );
}

#[test]
fn cleanup_parent_sync_failure_recreates_private_quarantine_with_bytes() {
    use std::os::unix::fs::PermissionsExt;

    let temp = crate::test_temp::tempdir().expect("tempdir");
    let session = temp.path().join("target.wayscriber-session");
    let recovery = temp.path().join("target.wayscriber-session.recovery");
    fs::write(&recovery, b"recovery").expect("write recovery");

    let quarantine = quarantine_save_as_sidecars(std::slice::from_ref(&recovery), &session)
        .expect("quarantine sidecar");
    let directory = quarantine
        .directory()
        .expect("quarantine directory")
        .to_path_buf();

    let err = quarantine
        .remove_after_commit_with(
            |path| fs::remove_file(path),
            |path| fs::remove_dir(path),
            |_| Err(anyhow!("injected parent sync failure")),
        )
        .expect_err("sync failure must retain recovery data");

    assert!(format!("{err:#}").contains("recovery bytes retained"));
    assert_eq!(
        fs::metadata(&directory)
            .expect("recreated quarantine metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::read(staged_file(
            &directory,
            "target.wayscriber-session.recovery"
        ))
        .expect("recovery retained"),
        b"recovery"
    );
}

#[cfg(unix)]
#[test]
fn quarantine_rejects_symlink_sidecar_without_following_it() {
    use std::os::unix::fs::symlink;

    let temp = crate::test_temp::tempdir().expect("tempdir");
    let session = temp.path().join("target.wayscriber-session");
    let sidecar = temp.path().join("target.wayscriber-session.recovery");
    let target = temp.path().join("outside");
    fs::write(&target, b"outside").expect("write target");
    symlink(&target, &sidecar).expect("create sidecar symlink");

    let err = quarantine_save_as_sidecars(std::slice::from_ref(&sidecar), &session)
        .expect_err("symlink sidecar must be rejected");

    assert!(format!("{err:#}").contains("sidecar symlink"));
    assert_eq!(fs::read(&target).expect("target preserved"), b"outside");
    assert!(sidecar.is_symlink());
}
