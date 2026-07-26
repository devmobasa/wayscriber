use std::fs;

use super::*;
use crate::runtime_ui_state::{SourceMutationId, SourceMutationResult};

#[test]
fn independent_writer_roots_mint_distinct_controller_identities() {
    let first = RuntimeUiStateStore::with_writer_namespace(
        "/tmp/wayscriber-controller-root-a.toml",
        RuntimeStateWriterNamespace::test_fixture(1),
    );
    let second = RuntimeUiStateStore::with_writer_namespace(
        "/tmp/wayscriber-controller-root-b.toml",
        RuntimeStateWriterNamespace::test_fixture(2),
    );

    assert_ne!(first.controller_id(), second.controller_id());
    assert_eq!(first.controller_id(), first.clone().controller_id());
}

#[test]
fn independent_writer_roots_do_not_reuse_removed_recovery_artifact_paths()
-> Result<(), &'static str> {
    let temp = crate::test_temp::tempdir().expect("test owns its runtime-state directory");
    let path = temp.path().join("runtime-ui.toml");
    let first_invalid = b"first invalid runtime state\n";
    fs::write(&path, first_invalid).expect("test establishes the first invalid source");
    let first_store = RuntimeUiStateStore::with_writer_namespace(
        &path,
        RuntimeStateWriterNamespace::test_fixture(1),
    );
    let first_revision = first_store
        .inspect()
        .expect("first writer root can inspect its invalid source")
        .observation
        .revision;
    let first = first_store.execute_preserve_invalid(SourceMutationId(1), first_revision);
    let SourceMutationResult::Applied {
        recovery_artifacts: first_artifacts,
        ..
    } = first
    else {
        return Err("first writer root must retain its invalid source");
    };
    let [first_artifact] = first_artifacts.as_slice() else {
        return Err("first writer root must report exactly one recovery artifact");
    };
    let first_recovery_path = first_artifact.path.clone();
    fs::remove_file(&first_recovery_path)
        .expect("test removes the first root's retained recovery artifact");

    let second_invalid = b"second unrelated invalid runtime state\n";
    fs::write(&path, second_invalid).expect("test establishes the second invalid source");
    let second_store = RuntimeUiStateStore::with_writer_namespace(
        &path,
        RuntimeStateWriterNamespace::test_fixture(2),
    );
    let second_revision = second_store
        .inspect()
        .expect("second writer root can inspect its invalid source")
        .observation
        .revision;
    let second = second_store.execute_preserve_invalid(SourceMutationId(1), second_revision);
    let SourceMutationResult::Applied {
        recovery_artifacts: second_artifacts,
        ..
    } = second
    else {
        return Err("second writer root must retain its invalid source");
    };
    let [second_artifact] = second_artifacts.as_slice() else {
        return Err("second writer root must report exactly one recovery artifact");
    };

    assert_ne!(first_recovery_path, second_artifact.path);
    assert_eq!(
        fs::read(&second_artifact.path).expect("second root's recovery artifact remains readable"),
        second_invalid
    );
    Ok(())
}
