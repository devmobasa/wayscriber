use std::fs;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::*;
use crate::runtime_ui_state::*;

fn startup_seeds() -> ValidatedInteractionSeeds {
    let mut seeds = ValidatedInteractionSeeds::new();
    seeds
        .insert(
            InteractionSeedTarget::TopPinned,
            InteractionSeedValue::Bool(false),
        )
        .unwrap();
    seeds
}

#[test]
fn every_accepted_command_produces_one_typed_completion() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let store = RuntimeUiStateStore::new(&path);
    let expected = store.inspect().unwrap().observation.revision;
    let writer = RuntimeUiStateWriter::spawn(store).unwrap();
    let request = SourceMutationRequest {
        id: SourceMutationId(1),
        accepted_through: AcceptedStateRevision(1),
        expected_source: expected,
        expected_epoch: 1,
        kind: SourceMutationKind::Replace(RuntimeUiWireState::default()),
    };

    writer
        .submit(RuntimeStateWriterCommand::SourceMutation(request))
        .expect("accepted");
    assert!(matches!(
        writer.recv().unwrap(),
        RuntimeStateWriterCompletion::SourceMutation(SourceMutationResult::Applied {
            id: SourceMutationId(1),
            ..
        })
    ));
    assert!(matches!(writer.try_recv(), Err(TryRecvError::Empty)));
    writer.shutdown();
    assert!(path.exists());
}

#[test]
fn a_panicking_completion_notifier_does_not_stop_the_writer() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let store = RuntimeUiStateStore::new(&path);
    let expected = store.inspect().unwrap().observation.revision;
    let calls = Arc::new(AtomicUsize::new(0));
    let notify_calls = Arc::clone(&calls);
    let writer = RuntimeUiStateWriter::spawn_with_completion_notifier(store, move || {
        if notify_calls.fetch_add(1, Ordering::SeqCst) == 0 {
            panic!("test notifier panic");
        }
    })
    .unwrap();

    writer
        .submit(RuntimeStateWriterCommand::SourceMutation(
            SourceMutationRequest {
                id: SourceMutationId(1),
                accepted_through: AcceptedStateRevision(1),
                expected_source: expected,
                expected_epoch: 1,
                kind: SourceMutationKind::Replace(RuntimeUiWireState::default()),
            },
        ))
        .unwrap();
    let RuntimeStateWriterCompletion::SourceMutation(SourceMutationResult::Applied {
        new_source,
        ..
    }) = writer.recv().unwrap()
    else {
        panic!("first command did not complete");
    };
    writer
        .submit(RuntimeStateWriterCommand::SourceMutation(
            SourceMutationRequest {
                id: SourceMutationId(2),
                accepted_through: AcceptedStateRevision(2),
                expected_source: new_source,
                expected_epoch: 1,
                kind: SourceMutationKind::Replace(RuntimeUiWireState::default()),
            },
        ))
        .unwrap();
    assert!(matches!(
        writer.recv().unwrap(),
        RuntimeStateWriterCompletion::SourceMutation(SourceMutationResult::Applied {
            id: SourceMutationId(2),
            ..
        })
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    writer.shutdown();
}

#[test]
fn source_commands_are_serialized_against_the_previous_completion_source() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let store = RuntimeUiStateStore::new(&path);
    let missing = store.inspect().unwrap().observation.revision;
    let writer = RuntimeUiStateWriter::spawn(store.clone()).unwrap();
    let first = SourceMutationRequest {
        id: SourceMutationId(1),
        accepted_through: AcceptedStateRevision(1),
        expected_source: missing,
        expected_epoch: 1,
        kind: SourceMutationKind::Replace(RuntimeUiWireState::default()),
    };
    writer
        .submit(RuntimeStateWriterCommand::SourceMutation(first))
        .unwrap();
    let RuntimeStateWriterCompletion::SourceMutation(SourceMutationResult::Applied {
        new_source,
        ..
    }) = writer.recv().unwrap()
    else {
        panic!("first write did not apply");
    };
    let second = SourceMutationRequest {
        id: SourceMutationId(2),
        accepted_through: AcceptedStateRevision(2),
        expected_source: new_source,
        expected_epoch: 1,
        kind: SourceMutationKind::ResetSupported { publish_epoch: 2 },
    };
    writer
        .submit(RuntimeStateWriterCommand::SourceMutation(second))
        .unwrap();
    assert!(matches!(
        writer.recv().unwrap(),
        RuntimeStateWriterCompletion::SourceMutation(SourceMutationResult::Applied {
            id: SourceMutationId(2),
            ..
        })
    ));
    writer.shutdown();
    assert!(!path.exists());
}

#[test]
fn writer_reports_filesystem_failures_without_dropping_completion() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("gone/runtime-ui.toml");
    let store = RuntimeUiStateStore::new(&path);
    let expected = store.inspect().unwrap().observation.revision;
    let writer = RuntimeUiStateWriter::spawn(store).unwrap();
    writer
        .submit(RuntimeStateWriterCommand::SourceMutation(
            SourceMutationRequest {
                id: SourceMutationId(8),
                accepted_through: AcceptedStateRevision(8),
                expected_source: expected,
                expected_epoch: 1,
                kind: SourceMutationKind::Replace(RuntimeUiWireState::default()),
            },
        ))
        .unwrap();
    assert!(matches!(
        writer.recv().unwrap(),
        RuntimeStateWriterCompletion::SourceMutation(SourceMutationResult::Failed {
            id: SourceMutationId(8),
            ..
        })
    ));
    writer.shutdown();
    assert!(!path.exists());
}

#[test]
fn inspection_command_returns_exact_unsupported_bytes() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let bytes = b"version = 22\nfuture = true\n";
    fs::write(&path, bytes).unwrap();
    let writer = RuntimeUiStateWriter::spawn(RuntimeUiStateStore::new(path)).unwrap();
    let command = RecoveryIoCommand {
        controller_id: ControllerId(1),
        incident: PersistenceIncidentId(2),
        barrier: ControllerBarrierId(3),
        attempt: RecoveryAttemptId(4),
        command_id: RecoveryCommandId(5),
        operation: RecoveryIoOperation::Inspect,
    };
    writer
        .submit(RuntimeStateWriterCommand::Recovery(command))
        .unwrap();
    let RuntimeStateWriterCompletion::Recovery(RecoveryIoCompletion {
        result: RecoveryIoResult::Inspected(Ok(inspection)),
        ..
    }) = writer.recv().unwrap()
    else {
        panic!("expected inspection completion");
    };
    assert_eq!(
        inspection.observation.revision.bytes(),
        Some(bytes.as_slice())
    );
    assert_eq!(
        inspection.observation.envelope,
        RuntimeStateObservedEnvelope::Version(22)
    );
    assert!(inspection.supported_wire.is_none());
    writer.shutdown();
}

#[test]
fn a_batch_of_accepted_inspections_has_no_missing_or_duplicate_completion() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let writer = RuntimeUiStateWriter::spawn(RuntimeUiStateStore::new(path)).unwrap();
    for id in 1..=12 {
        writer
            .submit(RuntimeStateWriterCommand::Recovery(RecoveryIoCommand {
                controller_id: ControllerId(1),
                incident: PersistenceIncidentId(2),
                barrier: ControllerBarrierId(3),
                attempt: RecoveryAttemptId(4),
                command_id: RecoveryCommandId(id),
                operation: RecoveryIoOperation::Inspect,
            }))
            .unwrap();
    }
    let mut completed = std::collections::BTreeSet::new();
    for _ in 0..12 {
        let RuntimeStateWriterCompletion::Recovery(completion) = writer.recv().unwrap() else {
            panic!("expected recovery completion");
        };
        assert!(completed.insert(completion.command_id));
    }
    assert_eq!(completed.len(), 12);
    assert!(matches!(writer.try_recv(), Err(TryRecvError::Empty)));
    writer.shutdown();
}

#[test]
fn malformed_startup_is_preserved_and_reset_through_real_recovery_io() {
    let temp = crate::test_temp::tempdir().unwrap();
    let path = temp.path().join("runtime-ui.toml");
    let invalid = b"not valid TOML at all\n";
    fs::write(&path, invalid).unwrap();
    let store = RuntimeUiStateStore::new(&path);
    let bootstrap = store
        .inspect()
        .unwrap()
        .into_controller_bootstrap(startup_seeds());
    let incident = bootstrap.startup_incident.unwrap();
    let mut controller = bootstrap.controller;
    let writer = RuntimeUiStateWriter::spawn(store.clone()).unwrap();

    let recovery = match controller.checkout_persistence_recovery_handle(incident) {
        CheckoutPersistenceRecoveryHandleResult::CheckedOut(handle) => handle,
        result => panic!("could not check out recovery: {result:?}"),
    };
    let request_client = match controller.begin_persistence_recovery(PersistenceRecoveryRequest {
        recovery,
        action: PersistenceRecoveryAction::RequestPreserveInvalidReset,
    }) {
        BeginPersistenceRecoveryResult::Started { client, .. } => client,
        result => panic!("could not request invalid reset: {result:?}"),
    };
    let inspect = controller.take_recovery_io_command().unwrap();
    writer
        .submit(RuntimeStateWriterCommand::Recovery(inspect))
        .unwrap();
    let RuntimeStateWriterCompletion::Recovery(completion) = writer.recv().unwrap() else {
        panic!("expected recovery inspection");
    };
    assert!(matches!(
        controller.submit_persistence_recovery_io(completion),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    let (recovery, confirmation) = match request_client.completion.try_recv() {
        Some(PersistenceRecoveryResult::RequiresInvalidResetConfirmation {
            recovery,
            confirmation,
            ..
        }) => (recovery, confirmation),
        result => panic!("invalid reset confirmation missing: {result:?}"),
    };

    let confirm_client = match controller.begin_persistence_recovery(PersistenceRecoveryRequest {
        recovery,
        action: PersistenceRecoveryAction::ConfirmPreserveInvalidReset(confirmation),
    }) {
        BeginPersistenceRecoveryResult::Started { client, .. } => client,
        result => panic!("could not confirm invalid reset: {result:?}"),
    };
    let confirm_inspection = controller.take_recovery_io_command().unwrap();
    writer
        .submit(RuntimeStateWriterCommand::Recovery(confirm_inspection))
        .unwrap();
    let RuntimeStateWriterCompletion::Recovery(completion) = writer.recv().unwrap() else {
        panic!("expected confirmation inspection");
    };
    assert!(matches!(
        controller.submit_persistence_recovery_io(completion),
        SubmitPersistenceRecoveryResult::Continue { .. }
    ));
    let preserve = controller.take_recovery_io_command().unwrap();
    writer
        .submit(RuntimeStateWriterCommand::Recovery(preserve))
        .unwrap();
    let RuntimeStateWriterCompletion::Recovery(completion) = writer.recv().unwrap() else {
        panic!("expected preserve-invalid completion");
    };
    assert!(matches!(
        controller.submit_persistence_recovery_io(completion),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    let recovery_path = match confirm_client.completion.try_recv() {
        Some(PersistenceRecoveryResult::InvalidSourcePreservedAndReset {
            path_effect: RuntimeStatePostClaimPathEffect::QuarantinedAndRetained { recovery_path },
            ..
        }) => recovery_path,
        result => panic!("preserve-invalid reset did not complete: {result:?}"),
    };
    assert_eq!(fs::read(recovery_path).unwrap(), invalid);
    assert_eq!(
        store.inspect().unwrap().status,
        RuntimeUiFileStatus::Missing
    );
    assert!(controller.active_barrier().is_none());
    writer.shutdown();
}
