use super::*;

fn backup_names(directory: &Path) -> Vec<String> {
    let mut names = fs::read_dir(directory)
        .expect("backup directory should be listable")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn write_config(path: &Path, contents: &str) {
    fs::write(path, contents).expect("test config should be written");
}

#[test]
fn the_first_runtime_save_snapshots_the_file_as_it_was() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let source = temp.path().join("config.toml");
    let directory = temp.path().join("config-backups");
    write_config(&source, "[ui]\nshow_status_bar = true\n");
    let mut backup = RuntimeConfigBackup::with_directory(&directory);

    backup.ensure_snapshot(&source);
    // The save the snapshot was taken for.
    write_config(&source, "[ui]\nshow_status_bar = false\n");

    let names = backup_names(&directory);
    assert_eq!(names.len(), 1, "expected exactly one snapshot: {names:?}");
    assert_eq!(
        fs::read_to_string(directory.join(&names[0])).expect("snapshot should be readable"),
        "[ui]\nshow_status_bar = true\n"
    );
}

/// The net is a per-session copy of what the user authored, not a per-write
/// journal: the second save must leave the first snapshot as the newest one.
#[test]
fn later_saves_in_the_same_process_do_not_snapshot_again() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let source = temp.path().join("config.toml");
    let directory = temp.path().join("config-backups");
    write_config(&source, "original = true\n");
    let mut backup = RuntimeConfigBackup::with_directory(&directory);

    backup.ensure_snapshot(&source);
    write_config(&source, "original = false\n");
    backup.ensure_snapshot(&source);
    backup.ensure_snapshot(&source);

    let names = backup_names(&directory);
    assert_eq!(names.len(), 1, "expected exactly one snapshot: {names:?}");
    assert_eq!(
        fs::read_to_string(directory.join(&names[0])).expect("snapshot should be readable"),
        "original = true\n"
    );
}

#[test]
fn pruning_keeps_the_newest_snapshots_by_filename_timestamp() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let directory = temp.path().join("config-backups");
    fs::create_dir_all(&directory).expect("backup directory should be creatable");
    for stamp in [
        "20260720-090000",
        "20260721-090000",
        "20260722-090000",
        "20260723-090000",
        "20260724-090000",
        "20260725-090000",
    ] {
        write_config(&directory.join(format!("config-{stamp}.toml")), stamp);
    }
    // Neither an unrelated file nor a directory counts against the retention.
    write_config(&directory.join("notes.txt"), "not a snapshot");
    fs::create_dir_all(directory.join("config-20260101-000000.toml"))
        .expect("decoy directory should be creatable");

    prune(&directory, BACKUP_RETENTION);

    assert_eq!(
        backup_names(&directory),
        vec![
            "config-20260101-000000.toml".to_string(),
            "config-20260721-090000.toml".to_string(),
            "config-20260722-090000.toml".to_string(),
            "config-20260723-090000.toml".to_string(),
            "config-20260724-090000.toml".to_string(),
            "config-20260725-090000.toml".to_string(),
            "notes.txt".to_string(),
        ]
    );
}

/// A second process in the same second must not overwrite the snapshot the
/// first one just took.
#[test]
fn a_name_collision_takes_the_next_name_instead_of_overwriting() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let directory = temp.path().join("config-backups");
    fs::create_dir_all(&directory).expect("backup directory should be creatable");
    let stamp = format_with_template(now_local(), "%Y%m%d-%H%M%S");
    let taken = directory.join(format!("{BACKUP_PREFIX}{stamp}{BACKUP_SUFFIX}"));
    write_config(&taken, "another process got here first\n");

    let path = write_unique_snapshot(&directory, b"ours\n").expect("snapshot should be written");

    assert_ne!(path, taken);
    assert_eq!(
        fs::read_to_string(&taken).expect("the first snapshot should be readable"),
        "another process got here first\n"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("our snapshot should be readable"),
        "ours\n"
    );
}

#[test]
fn a_config_that_does_not_exist_yet_leaves_no_snapshot() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let source = temp.path().join("config.toml");
    let directory = temp.path().join("config-backups");
    let mut backup = RuntimeConfigBackup::with_directory(&directory);

    backup.ensure_snapshot(&source);

    assert!(!directory.exists(), "no source means no backup directory");
}

/// A snapshot that cannot be taken is a logged warning, never a failed save.
#[test]
fn a_failed_snapshot_is_swallowed_and_never_retried() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let source = temp.path().join("config.toml");
    write_config(&source, "kept = true\n");
    // A regular file where the backup directory belongs: `create_dir_all`
    // cannot succeed, the way an unwritable state directory behaves.
    let directory = temp.path().join("config-backups");
    write_config(&directory, "not a directory\n");
    snapshot(&source, &directory, BACKUP_RETENTION)
        .expect_err("the fixture must make the snapshot fail");

    let mut backup = RuntimeConfigBackup::with_directory(&directory);
    backup.ensure_snapshot(&source);
    backup.ensure_snapshot(&source);

    assert_eq!(
        fs::read_to_string(&directory).expect("the blocking file should be untouched"),
        "not a directory\n"
    );
}
