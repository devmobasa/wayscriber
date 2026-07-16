use super::*;
use std::fs;
use std::io::Write;

#[cfg(unix)]
use std::os::unix::fs::{PermissionsExt, symlink};

fn replace_reject_options() -> AtomicWriteOptions {
    AtomicWriteOptions {
        overwrite: OverwriteMode::Replace,
        permissions: PermissionPolicy::PreserveExistingOrMode(0o644),
        symlink: SymlinkPolicy::Reject,
        sync_file: true,
        sync_parent: true,
    }
}

fn create_new_reject_options() -> AtomicWriteOptions {
    AtomicWriteOptions {
        overwrite: OverwriteMode::CreateNew,
        permissions: PermissionPolicy::FixedMode(0o640),
        symlink: SymlinkPolicy::Reject,
        sync_file: true,
        sync_parent: true,
    }
}

#[cfg(unix)]
#[test]
fn replacing_existing_file_preserves_mode() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("state.toml");
    fs::write(&path, "old").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

    write_text_atomic(&path, "new", replace_reject_options()).unwrap();

    assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[cfg(unix)]
#[test]
fn creating_new_file_uses_fixed_mode() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("created.toml");

    write_text_atomic(&path, "created", create_new_reject_options()).unwrap();

    assert_eq!(fs::read_to_string(&path).unwrap(), "created");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
}

#[test]
fn create_new_reports_existing_destination() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("existing.txt");
    fs::write(&path, "old").unwrap();

    let err = write_text_atomic(&path, "new", create_new_reject_options()).unwrap_err();

    assert!(matches!(err, DurableIoError::AlreadyExists { path: actual } if actual == path));
    assert_eq!(fs::read_to_string(&path).unwrap(), "old");
}

#[test]
fn temporary_file_creation_skips_colliding_paths() {
    let temp = crate::test_temp::tempdir().unwrap();
    let first = temp.path().join(".first.tmp");
    let second = temp.path().join(".second.tmp");
    fs::write(&first, "existing").unwrap();

    let (path, mut file) =
        create_temp_file_from_candidates([first.clone(), second.clone()]).unwrap();
    file.write_all(b"new").unwrap();
    drop(file);

    assert_eq!(path, second);
    assert_eq!(fs::read_to_string(first).unwrap(), "existing");
    assert_eq!(fs::read_to_string(second).unwrap(), "new");
}

#[cfg(unix)]
#[test]
fn temporary_file_creation_skips_symlink_collisions() {
    let temp = crate::test_temp::tempdir().unwrap();
    let target = temp.path().join("target");
    let link = temp.path().join(".link.tmp");
    let second = temp.path().join(".second.tmp");
    fs::write(&target, "target").unwrap();
    symlink(&target, &link).unwrap();

    let (path, _file) = create_temp_file_from_candidates([link, second.clone()]).unwrap();

    assert_eq!(path, second);
    assert_eq!(fs::read_to_string(target).unwrap(), "target");
}

#[cfg(unix)]
#[test]
fn follows_existing_symlink_and_preserves_link() {
    let temp = crate::test_temp::tempdir().unwrap();
    let target = temp.path().join("target.toml");
    let link = temp.path().join("config.toml");
    fs::write(&target, "old").unwrap();
    symlink("target.toml", &link).unwrap();

    write_text_atomic(&link, "new", AtomicWriteOptions::user_config_file()).unwrap();

    assert_eq!(fs::read_to_string(&target).unwrap(), "new");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read_link(&link).unwrap(),
        std::path::PathBuf::from("target.toml")
    );
}

#[cfg(unix)]
#[test]
fn followed_symlink_change_is_rejected_before_finalization() {
    let temp = crate::test_temp::tempdir().unwrap();
    let first = temp.path().join("first.toml");
    let second = temp.path().join("second.toml");
    let link = temp.path().join("config.toml");
    fs::write(&first, "first").unwrap();
    fs::write(&second, "second").unwrap();
    symlink(&first, &link).unwrap();
    let options = AtomicWriteOptions::user_config_file();
    let destination = inspect_destination(&link, options).unwrap();
    fs::remove_file(&link).unwrap();
    symlink(&second, &link).unwrap();

    let err = revalidate_destination(&destination, options).unwrap_err();

    assert!(matches!(err, DurableIoError::DestinationChanged { .. }));
    assert_eq!(fs::read_to_string(first).unwrap(), "first");
    assert_eq!(fs::read_to_string(second).unwrap(), "second");
}

#[cfg(unix)]
#[test]
fn followed_symlink_target_replacement_is_rejected_before_finalization() {
    let temp = crate::test_temp::tempdir().unwrap();
    let target = temp.path().join("target.toml");
    let link = temp.path().join("config.toml");
    fs::write(&target, "old").unwrap();
    symlink(&target, &link).unwrap();
    let options = AtomicWriteOptions::user_config_file();
    let destination = inspect_destination(&link, options).unwrap();
    let _old_file = fs::File::open(&target).unwrap();
    fs::remove_file(&target).unwrap();
    fs::write(&target, "replacement").unwrap();

    let err = revalidate_destination(&destination, options).unwrap_err();

    assert!(matches!(err, DurableIoError::DestinationChanged { .. }));
}

#[cfg(unix)]
#[test]
fn follow_policy_detects_replaced_regular_file() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("config.toml");
    fs::write(&path, "old").unwrap();
    let options = AtomicWriteOptions::user_config_file();
    let destination = inspect_destination(&path, options).unwrap();
    let _old_file = fs::File::open(&path).unwrap();
    fs::remove_file(&path).unwrap();
    fs::write(&path, "replacement").unwrap();

    let err = revalidate_destination(&destination, options).unwrap_err();

    assert!(matches!(err, DurableIoError::DestinationChanged { .. }));
}

#[cfg(unix)]
#[test]
fn reject_policy_rejects_destination_symlink() {
    let temp = crate::test_temp::tempdir().unwrap();
    let target = temp.path().join("target");
    let link = temp.path().join("state");
    fs::write(&target, "target").unwrap();
    symlink(&target, &link).unwrap();

    let err = write_text_atomic(&link, "new", replace_reject_options()).unwrap_err();

    assert!(matches!(err, DurableIoError::SymlinkRejected { path } if path == link));
    assert_eq!(fs::read_to_string(target).unwrap(), "target");
}

#[cfg(unix)]
#[test]
fn reject_policy_rejects_final_path_changed_to_symlink() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("state");
    let target = temp.path().join("target");
    fs::write(&path, "old").unwrap();
    fs::write(&target, "target").unwrap();
    let options = replace_reject_options();
    let destination = inspect_destination(&path, options).unwrap();
    fs::remove_file(&path).unwrap();
    symlink(&target, &path).unwrap();

    let err = revalidate_destination(&destination, options).unwrap_err();

    assert!(matches!(err, DurableIoError::SymlinkRejected { path: actual } if actual == path));
}

#[cfg(unix)]
#[test]
fn reject_policy_detects_replaced_regular_file() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("state");
    fs::write(&path, "old").unwrap();
    let options = replace_reject_options();
    let destination = inspect_destination(&path, options).unwrap();
    let _old_file = fs::File::open(&path).unwrap();
    fs::remove_file(&path).unwrap();
    fs::write(&path, "replacement").unwrap();

    let err = revalidate_destination(&destination, options).unwrap_err();

    assert!(matches!(err, DurableIoError::DestinationChanged { .. }));
}

#[test]
fn reject_policy_detects_created_regular_file_after_missing_inspect() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("state");
    let options = replace_reject_options();
    let destination = inspect_destination(&path, options).unwrap();
    fs::write(&path, "created-by-other-writer").unwrap();

    let err = revalidate_destination(&destination, options).unwrap_err();

    assert!(matches!(err, DurableIoError::DestinationChanged {
            operation: DurableIoOperation::InspectDestination,
            path: actual,
        } if actual == path));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "created-by-other-writer"
    );
}

#[cfg(unix)]
#[test]
fn dangling_followed_symlink_creates_target_and_preserves_link() {
    let temp = crate::test_temp::tempdir().unwrap();
    let link = temp.path().join("config.toml");
    let target = temp.path().join("missing-target.toml");
    symlink("missing-target.toml", &link).unwrap();

    write_text_atomic(&link, "new", AtomicWriteOptions::user_config_file()).unwrap();

    assert_eq!(fs::read_to_string(target).unwrap(), "new");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[cfg(unix)]
#[test]
fn follows_multi_level_symlink_chain() {
    let temp = crate::test_temp::tempdir().unwrap();
    let target = temp.path().join("target.toml");
    let middle = temp.path().join("middle.toml");
    let link = temp.path().join("config.toml");
    fs::write(&target, "old").unwrap();
    symlink("target.toml", &middle).unwrap();
    symlink("middle.toml", &link).unwrap();

    write_text_atomic(&link, "new", AtomicWriteOptions::user_config_file()).unwrap();

    assert_eq!(fs::read_to_string(target).unwrap(), "new");
    assert!(
        fs::symlink_metadata(&middle)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[cfg(unix)]
#[test]
fn changed_intermediate_symlink_is_rejected_before_finalization() {
    let temp = crate::test_temp::tempdir().unwrap();
    let first = temp.path().join("first.toml");
    let second = temp.path().join("second.toml");
    let middle = temp.path().join("middle.toml");
    let link = temp.path().join("config.toml");
    fs::write(&first, "first").unwrap();
    fs::write(&second, "second").unwrap();
    symlink("first.toml", &middle).unwrap();
    symlink("middle.toml", &link).unwrap();
    let options = AtomicWriteOptions::user_config_file();
    let destination = inspect_destination(&link, options).unwrap();
    fs::remove_file(&middle).unwrap();
    symlink("second.toml", &middle).unwrap();

    let err = revalidate_destination(&destination, options).unwrap_err();

    assert!(matches!(err, DurableIoError::DestinationChanged { .. }));
    assert_eq!(fs::read_to_string(first).unwrap(), "first");
    assert_eq!(fs::read_to_string(second).unwrap(), "second");
}

#[cfg(unix)]
#[test]
fn parent_directory_sync_succeeds() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("state");
    fs::write(&path, "state").unwrap();

    sync_parent_dir(&path).unwrap();
}
