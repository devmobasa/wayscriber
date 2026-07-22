use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{PermissionsExt, symlink};

use super::mutation::MutationPoint;
use super::*;
use crate::runtime_ui_state::*;

fn seeds() -> ValidatedInteractionSeeds {
    let mut seeds = ValidatedInteractionSeeds::new();
    seeds
        .insert(
            InteractionSeedTarget::TopPinned,
            InteractionSeedValue::Bool(false),
        )
        .unwrap();
    seeds
        .insert(
            InteractionSeedTarget::SidePinned,
            InteractionSeedValue::Bool(false),
        )
        .unwrap();
    seeds
}

fn request(
    id: u64,
    expected_source: RuntimeStateSourceRevision,
    kind: SourceMutationKind,
) -> SourceMutationRequest {
    SourceMutationRequest {
        id: SourceMutationId(id),
        accepted_through: AcceptedStateRevision(id),
        expected_source,
        expected_epoch: 1,
        kind,
    }
}

fn commit_top_pinned(controller: &mut RuntimeUiStateController) -> SourceMutationRequest {
    let permit = controller
        .begin_mutation(RuntimeUiMutationScope::one(
            InteractionSeedTarget::TopPinned,
        ))
        .unwrap();
    let values = RuntimeUiMutationValues::one(
        InteractionSeedTarget::TopPinned,
        InteractionSeedValue::Bool(true),
    )
    .unwrap();
    assert!(matches!(
        controller.commit(permit, values),
        CommitResult::Accepted { .. }
    ));
    controller.take_source_mutation().expect("source mutation")
}

#[test]
fn supported_write_connects_controller_to_restart_inspection() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let store = RuntimeUiStateStore::new(&path);
    let mut controller = store
        .inspect()
        .unwrap()
        .into_controller_bootstrap(seeds())
        .controller;

    let write = commit_top_pinned(&mut controller);
    let result = store.execute_source_mutation(write);
    assert!(matches!(result, SourceMutationResult::Applied { .. }));
    assert_eq!(
        controller.submit_source_mutation(result),
        SubmitSourceMutationResult::Integrated
    );

    let inspection = store.inspect().unwrap();
    assert_eq!(inspection.status, RuntimeUiFileStatus::Supported);
    assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0);
    let restarted = inspection.into_controller_bootstrap(seeds()).controller;
    assert_eq!(
        restarted
            .live_state()
            .get(&InteractionSeedTarget::TopPinned),
        Some(&InteractionSeedValue::Bool(true))
    );
}

#[test]
fn external_change_before_missing_write_wins_without_overwrite() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let store = RuntimeUiStateStore::new(&path);
    let mut controller = store
        .inspect()
        .unwrap()
        .into_controller_bootstrap(seeds())
        .controller;
    let write = commit_top_pinned(&mut controller);
    let external = b"version = 27\nfuture = true\n";
    fs::write(&path, external).unwrap();

    assert!(matches!(
        store.execute_source_mutation(write),
        SourceMutationResult::SourceChangedBeforeMutation { .. }
    ));
    assert_eq!(fs::read(path).unwrap(), external);
}

#[test]
fn replacing_same_bytes_with_a_new_inode_is_a_conflict() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let bytes = encode_runtime_ui_file(&RuntimeUiWireState::default()).unwrap();
    fs::write(&path, &bytes).unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let expected = store.inspect().unwrap().observation.revision;
    fs::remove_file(&path).unwrap();
    fs::write(&path, &bytes).unwrap();

    let result = store.execute_source_mutation(request(
        1,
        expected,
        SourceMutationKind::ResetSupported { publish_epoch: 2 },
    ));
    assert!(matches!(
        result,
        SourceMutationResult::SourceChangedBeforeMutation { .. }
    ));
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn active_file_appearing_after_claim_is_not_overwritten() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let original = encode_runtime_ui_file(&RuntimeUiWireState::default()).unwrap();
    fs::write(&path, &original).unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let mut controller = store
        .inspect()
        .unwrap()
        .into_controller_bootstrap(seeds())
        .controller;
    let write = commit_top_pinned(&mut controller);
    let external = b"version = 88\nfuture = 'external'\n";

    let result = store.execute_source_mutation_with_hook(write, &mut |point| {
        if point == MutationPoint::AfterClaim {
            fs::write(&path, external).unwrap();
        }
    });
    let SourceMutationResult::ObservationChangedAfterClaim {
        recovery_artifacts,
        path_effect,
        ..
    } = result
    else {
        panic!("expected retained post-claim conflict");
    };
    assert_eq!(fs::read(&path).unwrap(), external);
    assert_eq!(recovery_artifacts.len(), 1);
    assert_eq!(
        recovery_artifacts[0].observation.revision.bytes(),
        Some(original.as_slice())
    );
    assert!(matches!(
        path_effect,
        RuntimeStatePostClaimPathEffect::QuarantinedAndRetained { .. }
    ));
}

#[test]
fn unsupported_reset_preserves_exact_bytes_as_an_artifact() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let unsupported = b"version = 9\nfuture = [1, 2, 3]\n";
    fs::write(&path, unsupported).unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let inspection = store.inspect().unwrap();
    let expected = inspection.observation.revision;
    let confirmed = expected.clone();
    assert!(matches!(
        inspection.status,
        RuntimeUiFileStatus::UnsupportedReadOnly { version: Some(9) }
    ));

    let result = store.execute_source_mutation(request(
        3,
        expected.clone(),
        SourceMutationKind::ResetUnsupportedIfUnchanged {
            publish_epoch: 2,
            confirmation_revision: expected,
        },
    ));
    let SourceMutationResult::Applied {
        new_source,
        recovery_artifacts,
        ..
    } = result
    else {
        panic!("expected applied reset");
    };
    assert!(new_source.bytes().is_none());
    assert!(!path.exists());
    assert_eq!(recovery_artifacts.len(), 1);
    assert_eq!(recovery_artifacts[0].observation.revision, confirmed);
    assert_eq!(fs::read(&recovery_artifacts[0].path).unwrap(), unsupported);
}

#[test]
fn invalid_reset_preserves_exact_bytes_as_an_artifact() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let invalid = b"this is not valid TOML\n";
    fs::write(&path, invalid).unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let inspection = store.inspect().unwrap();
    assert_eq!(inspection.status, RuntimeUiFileStatus::Invalid);
    let confirmed = inspection.observation.revision.clone();

    let result =
        store.execute_preserve_invalid(SourceMutationId(5), inspection.observation.revision);
    let SourceMutationResult::Applied {
        recovery_artifacts, ..
    } = result
    else {
        panic!("expected applied preserve-invalid reset");
    };
    assert!(!path.exists());
    assert_eq!(recovery_artifacts.len(), 1);
    assert_eq!(recovery_artifacts[0].observation.revision, confirmed);
    assert_eq!(fs::read(&recovery_artifacts[0].path).unwrap(), invalid);
}

#[test]
fn target_symlink_is_rejected_and_neither_path_is_modified() {
    let temp = crate::test_temp::tempdir().unwrap();
    let target = temp.path().join("external.toml");
    let path = temp.path().join("runtime-ui.toml");
    let external = b"version = 17\n";
    fs::write(&target, external).unwrap();
    symlink(&target, &path).unwrap();
    let store = RuntimeUiStateStore::new(&path);

    assert!(store.inspect().is_err());
    assert_eq!(fs::read_link(&path).unwrap(), target);
    assert_eq!(fs::read(&target).unwrap(), external);
}

#[test]
fn fifo_is_rejected_without_blocking_for_a_writer() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let c_path = CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: the CString is NUL-terminated and remains live for the call.
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
    let started = std::time::Instant::now();
    assert!(RuntimeUiStateStore::new(path).inspect().is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn uncertain_post_install_result_blocks_the_controller() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let store = RuntimeUiStateStore::new(&path);
    let mut controller = store
        .inspect()
        .unwrap()
        .into_controller_bootstrap(seeds())
        .controller;
    let write = commit_top_pinned(&mut controller);

    let result = store.execute_source_mutation_with_hook(write, &mut |point| {
        if point == MutationPoint::AfterInstall {
            fs::write(&path, b"version = 71\nexternal = true\n").unwrap();
        }
    });
    assert!(matches!(
        &result,
        SourceMutationResult::Failed {
            path_effect: RuntimeStateFailurePathEffect::UnknownAfterMutation,
            ..
        }
    ));
    assert!(matches!(
        controller.submit_source_mutation(result),
        SubmitSourceMutationResult::PersistenceUnhealthy { .. }
    ));
    assert!(controller.active_barrier().is_some());
}

#[test]
fn oversized_file_is_rejected_before_parsing() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    fs::write(&path, vec![b'x'; (MAX_RUNTIME_UI_FILE_BYTES + 1) as usize]).unwrap();
    assert!(RuntimeUiStateStore::new(path).inspect().is_err());
}

#[test]
fn preparation_failure_reports_an_untouched_failure() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("missing-parent/runtime-ui.toml");
    let store = RuntimeUiStateStore::new(&path);
    let expected = store.inspect().unwrap().observation.revision;
    let result = store.execute_source_mutation(request(
        9,
        expected,
        SourceMutationKind::Replace(RuntimeUiWireState::default()),
    ));
    assert!(matches!(
        result,
        SourceMutationResult::Failed {
            path_effect: RuntimeStateFailurePathEffect::Known(
                RuntimeStateObservedPathEffect::Untouched
            ),
            ..
        }
    ));
    assert!(!path.exists());
}

#[test]
fn malformed_startup_enters_the_recovery_barrier() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    fs::write(&path, b"not valid TOML").unwrap();
    let mut bootstrap = RuntimeUiStateStore::new(path)
        .inspect()
        .unwrap()
        .into_controller_bootstrap(seeds());
    let incident = bootstrap.startup_incident.expect("startup incident");
    assert!(matches!(
        bootstrap
            .controller
            .active_barrier()
            .map(|barrier| &barrier.operation),
        Some(ControllerBarrierOperation::StartupPersistenceRecovery)
    ));
    assert!(matches!(
        bootstrap
            .controller
            .checkout_persistence_recovery_handle(incident),
        CheckoutPersistenceRecoveryHandleResult::CheckedOut(_)
    ));
}

#[test]
fn unrelated_controller_write_preserves_supported_unknown_fields() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    fs::write(
        &path,
        br#"version = 1
future_root = { answer = 42 }

[toolbar]
future_toolbar = "kept"

[toolbar.top_pinned]
seed = false
value = true
future_entry = [1, 2]

[boards]
future_boards = true
"#,
    )
    .unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let mut controller = store
        .inspect()
        .unwrap()
        .into_controller_bootstrap(seeds())
        .controller;
    let permit = controller
        .begin_mutation(RuntimeUiMutationScope::one(
            InteractionSeedTarget::SidePinned,
        ))
        .unwrap();
    let values = RuntimeUiMutationValues::one(
        InteractionSeedTarget::SidePinned,
        InteractionSeedValue::Bool(true),
    )
    .unwrap();
    assert!(matches!(
        controller.commit(permit, values),
        CommitResult::Accepted { .. }
    ));
    let result = store.execute_source_mutation(controller.take_source_mutation().unwrap());
    assert!(matches!(result, SourceMutationResult::Applied { .. }));

    let value: toml::Value = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(value["future_root"]["answer"].as_integer(), Some(42));
    assert_eq!(value["toolbar"]["future_toolbar"].as_str(), Some("kept"));
    assert_eq!(
        value["toolbar"]["top_pinned"]["future_entry"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(value["boards"]["future_boards"].as_bool(), Some(true));
}

#[test]
fn ordinary_commands_cannot_overwrite_unsupported_or_invalid_files() {
    let temp = crate::test_temp::tempdir().unwrap();
    for (name, bytes) in [
        (
            "unsupported.toml",
            b"version = 12\nfuture = true\n".as_slice(),
        ),
        ("invalid.toml", b"not valid TOML\n".as_slice()),
    ] {
        let path = temp.path().join(name);
        fs::write(&path, bytes).unwrap();
        let store = RuntimeUiStateStore::new(&path);
        let expected = store.inspect().unwrap().observation.revision;
        for kind in [
            SourceMutationKind::Replace(RuntimeUiWireState::default()),
            SourceMutationKind::ResetSupported { publish_epoch: 2 },
        ] {
            let result = store.execute_source_mutation(request(20, expected.clone(), kind));
            assert!(matches!(result, SourceMutationResult::Failed { .. }));
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
    }
}

#[test]
fn symlink_retarget_after_inspection_is_rejected_without_touching_target() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let target = temp.path().join("external.toml");
    let supported = encode_runtime_ui_file(&RuntimeUiWireState::default()).unwrap();
    let external = b"version = 33\n";
    fs::write(&path, &supported).unwrap();
    fs::write(&target, external).unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let expected = store.inspect().unwrap().observation.revision;
    fs::remove_file(&path).unwrap();
    symlink(&target, &path).unwrap();

    let result = store.execute_source_mutation(request(
        30,
        expected,
        SourceMutationKind::ResetSupported { publish_epoch: 2 },
    ));
    assert!(matches!(result, SourceMutationResult::Failed { .. }));
    assert_eq!(fs::read_link(&path).unwrap(), target);
    assert_eq!(fs::read(&target).unwrap(), external);
}

#[test]
fn supported_reset_removes_only_the_exact_confirmed_source() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    fs::write(
        &path,
        encode_runtime_ui_file(&RuntimeUiWireState::default()).unwrap(),
    )
    .unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let expected = store.inspect().unwrap().observation.revision;
    let result = store.execute_source_mutation(request(
        40,
        expected,
        SourceMutationKind::ResetSupported { publish_epoch: 2 },
    ));
    let SourceMutationResult::Applied {
        new_source,
        recovery_artifacts,
        ..
    } = result
    else {
        panic!("expected supported reset");
    };
    assert!(new_source.bytes().is_none());
    assert!(recovery_artifacts.is_empty());
    assert!(!path.exists());
}

#[test]
fn reset_conflict_after_claim_preserves_both_files() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let original = encode_runtime_ui_file(&RuntimeUiWireState::default()).unwrap();
    let external = b"version = 44\nexternal = true\n";
    fs::write(&path, &original).unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let expected = store.inspect().unwrap().observation.revision;
    let request = request(
        50,
        expected,
        SourceMutationKind::ResetSupported { publish_epoch: 2 },
    );
    let result = store.execute_source_mutation_with_hook(request, &mut |point| {
        if point == MutationPoint::AfterClaim {
            fs::write(&path, external).unwrap();
        }
    });
    let SourceMutationResult::ObservationChangedAfterClaim {
        recovery_artifacts, ..
    } = result
    else {
        panic!("expected post-claim conflict");
    };
    assert_eq!(fs::read(&path).unwrap(), external);
    assert_eq!(recovery_artifacts.len(), 1);
    assert_eq!(fs::read(&recovery_artifacts[0].path).unwrap(), original);
}

#[test]
fn stale_board_ids_are_pruned_by_startup_cleanup() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    fs::write(
        &path,
        br#"version = 1

[boards.pinned.stale-board]
seed = false
value = true
"#,
    )
    .unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let mut controller = store
        .inspect()
        .unwrap()
        .into_controller_bootstrap(seeds())
        .controller;
    let cleanup = controller
        .take_source_mutation()
        .expect("startup cleanup mutation");
    let result = store.execute_source_mutation(cleanup);
    assert!(matches!(result, SourceMutationResult::Applied { .. }));
    assert!(!fs::read_to_string(path).unwrap().contains("stale-board"));
}

#[test]
fn changed_claim_is_restored_without_overwriting_another_path() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let original = encode_runtime_ui_file(&RuntimeUiWireState::default()).unwrap();
    let changed = b"version = 61\nchanged_after_claim = true\n";
    fs::write(&path, &original).unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let expected = store.inspect().unwrap().observation.revision;
    let request = request(
        60,
        expected,
        SourceMutationKind::ResetSupported { publish_epoch: 2 },
    );
    let result = store.execute_source_mutation_with_hook(request, &mut |point| {
        if point == MutationPoint::AfterClaim {
            let quarantine = fs::read_dir(temp.path())
                .unwrap()
                .map(Result::unwrap)
                .map(|entry| entry.path())
                .find(|candidate| {
                    candidate
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().contains("wayscriber-recovery"))
                })
                .expect("quarantine path");
            fs::write(quarantine, changed).unwrap();
        }
    });
    let SourceMutationResult::ObservationChangedAfterClaim {
        active,
        recovery_artifacts,
        path_effect,
        ..
    } = result
    else {
        panic!("expected restored post-claim observation");
    };
    assert_eq!(active.revision.bytes(), Some(changed.as_slice()));
    assert!(recovery_artifacts.is_empty());
    assert!(matches!(
        path_effect,
        RuntimeStatePostClaimPathEffect::QuarantinedThenRestored { .. }
    ));
    assert_eq!(fs::read(path).unwrap(), changed);
}

#[test]
fn disappeared_claim_is_not_reported_as_a_recovery_artifact() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let original = encode_runtime_ui_file(&RuntimeUiWireState::default()).unwrap();
    fs::write(&path, original).unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let expected = store.inspect().unwrap().observation.revision;
    let request = request(
        61,
        expected,
        SourceMutationKind::ResetSupported { publish_epoch: 2 },
    );

    let result = store.execute_source_mutation_with_hook(request, &mut |point| {
        if point == MutationPoint::AfterClaim {
            let quarantine = fs::read_dir(temp.path())
                .unwrap()
                .map(Result::unwrap)
                .map(|entry| entry.path())
                .find(|candidate| {
                    candidate
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().contains("wayscriber-recovery"))
                })
                .expect("quarantine path");
            fs::remove_file(quarantine).unwrap();
        }
    });

    assert!(matches!(
        result,
        SourceMutationResult::Failed {
            recovery_artifacts,
            path_effect: RuntimeStateFailurePathEffect::UnknownAfterMutation,
            ..
        } if recovery_artifacts.is_empty()
    ));
    assert!(!path.exists());
}

#[test]
fn confirmed_unsupported_reset_runs_end_to_end_through_the_controller() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let unsupported = b"version = 73\nfuture = 'preserve me'\n";
    fs::write(&path, unsupported).unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let mut bootstrap = store.inspect().unwrap().into_controller_bootstrap(seeds());
    assert!(bootstrap.startup_incident.is_none());
    let confirmation = match bootstrap.controller.request_runtime_ui_reset() {
        RequestResetResult::RequiresUnsupportedConfirmation {
            observed_version: Some(73),
            confirmation,
        } => confirmation,
        result => panic!("unexpected reset request: {result:?}"),
    };
    assert!(matches!(
        bootstrap.controller.confirm_unsupported_reset(confirmation),
        ConfirmUnsupportedResetResult::Started { .. }
    ));
    let command = bootstrap.controller.take_source_mutation().unwrap();
    let result = store.execute_source_mutation(command);
    let outcome = bootstrap.controller.submit_source_mutation(result);
    let SubmitSourceMutationResult::ResetCompleted {
        recovery_artifacts, ..
    } = outcome
    else {
        panic!("unsupported reset did not complete: {outcome:?}");
    };
    assert_eq!(recovery_artifacts.len(), 1);
    assert_eq!(fs::read(&recovery_artifacts[0].path).unwrap(), unsupported);
    assert_eq!(
        store.inspect().unwrap().status,
        RuntimeUiFileStatus::Missing
    );

    let write = commit_top_pinned(&mut bootstrap.controller);
    let result = store.execute_source_mutation(write);
    assert!(matches!(
        bootstrap.controller.submit_source_mutation(result),
        SubmitSourceMutationResult::Integrated
    ));
    let restarted = store
        .inspect()
        .unwrap()
        .into_controller_bootstrap(seeds())
        .controller;
    assert_eq!(
        restarted
            .live_state()
            .get(&InteractionSeedTarget::TopPinned),
        Some(&InteractionSeedValue::Bool(true))
    );
}

#[test]
fn missing_source_rejects_parent_symlink_retarget_before_write() {
    let temp = crate::test_temp::tempdir().unwrap();
    let first_parent = temp.path().join("first");
    let second_parent = temp.path().join("second");
    let selected_parent = temp.path().join("selected");
    fs::create_dir(&first_parent).unwrap();
    fs::create_dir(&second_parent).unwrap();
    symlink(&first_parent, &selected_parent).unwrap();
    let path = selected_parent.join("runtime-ui.toml");
    let store = RuntimeUiStateStore::new(&path);
    let mut controller = store
        .inspect()
        .unwrap()
        .into_controller_bootstrap(seeds())
        .controller;
    let write = commit_top_pinned(&mut controller);

    fs::remove_file(&selected_parent).unwrap();
    symlink(&second_parent, &selected_parent).unwrap();

    assert!(matches!(
        store.execute_source_mutation(write),
        SourceMutationResult::SourceChangedBeforeMutation { .. }
    ));
    assert!(!first_parent.join("runtime-ui.toml").exists());
    assert!(!second_parent.join("runtime-ui.toml").exists());
}

#[test]
fn missing_source_rejects_replacement_of_the_resolved_parent_directory() {
    let temp = crate::test_temp::tempdir().unwrap();
    let parent = temp.path().join("state");
    let displaced_parent = temp.path().join("displaced-state");
    fs::create_dir(&parent).unwrap();
    let path = parent.join("runtime-ui.toml");
    let store = RuntimeUiStateStore::new(&path);
    let mut controller = store
        .inspect()
        .unwrap()
        .into_controller_bootstrap(seeds())
        .controller;
    let write = commit_top_pinned(&mut controller);

    fs::rename(&parent, &displaced_parent).unwrap();
    fs::create_dir(&parent).unwrap();

    assert!(matches!(
        store.execute_source_mutation(write),
        SourceMutationResult::SourceChangedBeforeMutation { .. }
    ));
    assert!(!parent.join("runtime-ui.toml").exists());
    assert!(!displaced_parent.join("runtime-ui.toml").exists());
}

#[test]
fn parent_symlink_retarget_after_claim_retains_the_original_artifact() {
    let temp = crate::test_temp::tempdir().unwrap();
    let first_parent = temp.path().join("first");
    let second_parent = temp.path().join("second");
    let selected_parent = temp.path().join("selected");
    fs::create_dir(&first_parent).unwrap();
    fs::create_dir(&second_parent).unwrap();
    symlink(&first_parent, &selected_parent).unwrap();
    let path = selected_parent.join("runtime-ui.toml");
    let original = encode_runtime_ui_file(&RuntimeUiWireState::default()).unwrap();
    fs::write(first_parent.join("runtime-ui.toml"), &original).unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let mut controller = store
        .inspect()
        .unwrap()
        .into_controller_bootstrap(seeds())
        .controller;
    let write = commit_top_pinned(&mut controller);

    let result = store.execute_source_mutation_with_hook(write, &mut |point| {
        if point == MutationPoint::AfterClaim {
            fs::remove_file(&selected_parent).unwrap();
            symlink(&second_parent, &selected_parent).unwrap();
        }
    });

    let SourceMutationResult::ObservationChangedAfterClaim {
        recovery_artifacts, ..
    } = result
    else {
        panic!("expected a retained post-claim conflict");
    };
    assert_eq!(recovery_artifacts.len(), 1);
    assert_eq!(fs::read(&recovery_artifacts[0].path).unwrap(), original);
    assert!(recovery_artifacts[0].path.starts_with(&first_parent));
    assert!(!second_parent.join("runtime-ui.toml").exists());
}

#[test]
fn resolved_parent_replacement_after_claim_retains_the_original_artifact() {
    let temp = crate::test_temp::tempdir().unwrap();
    let parent = temp.path().join("state");
    let displaced_parent = temp.path().join("displaced-state");
    fs::create_dir(&parent).unwrap();
    let path = parent.join("runtime-ui.toml");
    let original = encode_runtime_ui_file(&RuntimeUiWireState::default()).unwrap();
    fs::write(&path, &original).unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let mut controller = store
        .inspect()
        .unwrap()
        .into_controller_bootstrap(seeds())
        .controller;
    let write = commit_top_pinned(&mut controller);

    let result = store.execute_source_mutation_with_hook(write, &mut |point| {
        if point == MutationPoint::AfterClaim {
            fs::rename(&parent, &displaced_parent).unwrap();
            fs::create_dir(&parent).unwrap();
        }
    });

    let SourceMutationResult::ObservationChangedAfterClaim {
        recovery_artifacts, ..
    } = result
    else {
        panic!("expected a retained post-claim conflict");
    };
    assert_eq!(recovery_artifacts.len(), 1);
    assert_eq!(fs::read(&recovery_artifacts[0].path).unwrap(), original);
    assert!(recovery_artifacts[0].path.starts_with(&displaced_parent));
    assert!(!parent.join("runtime-ui.toml").exists());
    assert!(
        fs::read_dir(&displaced_parent)
            .unwrap()
            .map(Result::unwrap)
            .all(|entry| !entry
                .file_name()
                .to_string_lossy()
                .contains("wayscriber-tmp"))
    );
}

#[test]
fn removed_override_does_not_resurrect_its_entry_passthrough() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    fs::write(
        &path,
        br#"version = 1

[toolbar.top_pinned]
seed = false
value = true
future_entry = "must not return"
"#,
    )
    .unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let mut controller = store
        .inspect()
        .unwrap()
        .into_controller_bootstrap(seeds())
        .controller;

    let permit = controller
        .begin_mutation(RuntimeUiMutationScope::one(
            InteractionSeedTarget::TopPinned,
        ))
        .unwrap();
    let configured = RuntimeUiMutationValues::one(
        InteractionSeedTarget::TopPinned,
        InteractionSeedValue::Bool(false),
    )
    .unwrap();
    assert!(matches!(
        controller.commit(permit, configured),
        CommitResult::Accepted { .. }
    ));
    let removed = store.execute_source_mutation(controller.take_source_mutation().unwrap());
    assert_eq!(
        controller.submit_source_mutation(removed),
        SubmitSourceMutationResult::Integrated
    );

    let recreated = commit_top_pinned(&mut controller);
    let recreated = store.execute_source_mutation(recreated);
    assert_eq!(
        controller.submit_source_mutation(recreated),
        SubmitSourceMutationResult::Integrated
    );
    let encoded = fs::read_to_string(path).unwrap();
    assert!(encoded.contains("[toolbar.top_pinned]"));
    assert!(!encoded.contains("future_entry"));
}

#[test]
fn seed_reconciliation_does_not_resurrect_pruned_entry_passthrough() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    fs::write(
        &path,
        br#"version = 1

[toolbar.top_pinned]
seed = false
value = true
future_entry = "must not return"
"#,
    )
    .unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let mut controller = store
        .inspect()
        .unwrap()
        .into_controller_bootstrap(seeds())
        .controller;
    let mut changed_seeds = ValidatedInteractionSeeds::new();
    changed_seeds
        .insert(
            InteractionSeedTarget::TopPinned,
            InteractionSeedValue::Bool(true),
        )
        .unwrap();
    changed_seeds
        .insert(
            InteractionSeedTarget::SidePinned,
            InteractionSeedValue::Bool(false),
        )
        .unwrap();

    assert!(matches!(
        controller.update_seeds(changed_seeds),
        UpdateSeedsResult::Applied {
            cleanup_through: Some(_),
            ..
        }
    ));
    let cleanup = store.execute_source_mutation(controller.take_source_mutation().unwrap());
    assert_eq!(
        controller.submit_source_mutation(cleanup),
        SubmitSourceMutationResult::Integrated
    );
    assert!(matches!(
        controller.update_seeds(seeds()),
        UpdateSeedsResult::Applied { .. }
    ));

    let recreated = commit_top_pinned(&mut controller);
    let recreated = store.execute_source_mutation(recreated);
    assert_eq!(
        controller.submit_source_mutation(recreated),
        SubmitSourceMutationResult::Integrated
    );
    let encoded = fs::read_to_string(path).unwrap();
    assert!(encoded.contains("[toolbar.top_pinned]"));
    assert!(!encoded.contains("future_entry"));
}
