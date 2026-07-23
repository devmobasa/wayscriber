use super::*;

#[test]
fn recovery_prunes_override_when_board_id_is_removed_and_readded_with_same_seed() {
    let mut controller = controller();
    let target = InteractionSeedTarget::BoardPin("board-6".to_string());
    commit_bool(&mut controller, target.clone(), true);
    let (_, incident) = fail_current_replace(&mut controller, "board pin write failed");

    let mut removed = test_seeds(false, false);
    removed.remove(&target);
    assert!(matches!(
        controller.update_seeds(removed),
        UpdateSeedsResult::StagedBehindBarrier { .. }
    ));
    assert!(matches!(
        controller.update_seeds(test_seeds(false, false)),
        UpdateSeedsResult::StagedBehindBarrier { .. }
    ));

    let (client, inspection) = begin_recovery(
        &mut controller,
        incident,
        PersistenceRecoveryAction::RetryPending,
    );
    let result = controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: inspection.barrier,
        attempt: inspection.attempt,
        command_id: inspection.command_id,
        result: RecoveryIoResult::Inspected(Ok(inspected(observation(missing_revision())))),
    });
    if matches!(result, SubmitPersistenceRecoveryResult::Continue { .. }) {
        let write = controller
            .take_recovery_io_command()
            .expect("canonical recovery write");
        let request = match &write.operation {
            RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
            operation => panic!("unexpected recovery operation: {operation:?}"),
        };
        assert!(matches!(
            controller.submit_persistence_recovery_io(RecoveryIoCompletion {
                controller_id: controller.id(),
                incident,
                barrier: write.barrier,
                attempt: write.attempt,
                command_id: write.command_id,
                result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
                    id: request.id,
                    applied_through: request.accepted_through,
                    new_source: present_revision("recovered-after-board-reuse"),
                    recovery_artifacts: Vec::new(),
                }),
            }),
            SubmitPersistenceRecoveryResult::Terminal { .. }
        ));
    } else {
        assert!(matches!(
            result,
            SubmitPersistenceRecoveryResult::Terminal { .. }
        ));
    }
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Recovered { .. })
    ));
    assert!(controller.model().get(&target).is_none());
    assert_eq!(
        controller.live_state().get(&target),
        Some(&InteractionSeedValue::Bool(false))
    );
}

#[test]
fn recovery_captures_replacements_fenced_by_a_flush_before_closing_barrier() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let second = commit_bool(&mut controller, InteractionSeedTarget::SidePinned, true);
    let flush = controller.request_flush(second).unwrap();
    let (_, incident) = fail_current_replace(&mut controller, "first write failed");
    assert!(matches!(
        controller.update_seeds(test_seeds(true, false)),
        UpdateSeedsResult::StagedBehindBarrier { .. }
    ));

    let (client, inspection) = begin_recovery(
        &mut controller,
        incident,
        PersistenceRecoveryAction::RetryPending,
    );
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: inspection.barrier,
            attempt: inspection.attempt,
            command_id: inspection.command_id,
            result: RecoveryIoResult::Inspected(Ok(inspected(observation(missing_revision())))),
        }),
        SubmitPersistenceRecoveryResult::Continue { .. }
    ));
    let write = controller.take_recovery_io_command().unwrap();
    let request = match &write.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected operation: {operation:?}"),
    };
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: write.barrier,
            attempt: write.attempt,
            command_id: write.command_id,
            result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
                id: request.id,
                applied_through: request.accepted_through,
                new_source: present_revision("recovered-with-flush"),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Recovered { .. })
    ));
    assert!(controller.active_barrier().is_none());
    assert!(controller.take_source_mutation().is_none());
    assert!(matches!(
        controller.flush_outcome(flush),
        Some(FlushOutcome::Failed)
    ));
}

#[test]
fn failed_reset_prerequisite_captures_replacements_fenced_by_a_flush() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let second = commit_bool(&mut controller, InteractionSeedTarget::SidePinned, true);
    let flush = controller.request_flush(second).unwrap();
    let reset_through = match controller.request_supported_reset() {
        RequestResetResult::Started { through, .. } => through,
        result => panic!("reset did not start: {result:?}"),
    };
    let (_, incident) = fail_current_replace(&mut controller, "reset prerequisite failed");

    let (client, inspection) = begin_recovery(
        &mut controller,
        incident,
        PersistenceRecoveryAction::RetryPending,
    );
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: inspection.barrier,
            attempt: inspection.attempt,
            command_id: inspection.command_id,
            result: RecoveryIoResult::Inspected(Ok(inspected(observation(missing_revision())))),
        }),
        SubmitPersistenceRecoveryResult::Continue { .. }
    ));
    let write = controller.take_recovery_io_command().unwrap();
    let request = match &write.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected operation: {operation:?}"),
    };
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: write.barrier,
            attempt: write.attempt,
            command_id: write.command_id,
            result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
                id: request.id,
                applied_through: request.accepted_through,
                new_source: present_revision("recovered-reset-prerequisite"),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Recovered { .. })
    ));
    assert!(controller.active_barrier().is_none());
    assert!(controller.take_source_mutation().is_none());
    assert!(matches!(
        controller.flush_outcome(flush),
        Some(FlushOutcome::Failed)
    ));
    assert!(matches!(
        controller.receipt(reset_through),
        Some(DurabilityOutcome::Failed(error))
            if error.message() == "reset prerequisite failed"
    ));
}

#[test]
fn reload_cleanup_is_written_even_when_failed_reset_has_no_retry_snapshot() {
    let mut original = controller();
    commit_bool(&mut original, InteractionSeedTarget::TopPinned, true);
    let persisted = original.take_source_mutation().unwrap();
    let persisted_wire = match &persisted.kind {
        SourceMutationKind::Replace(wire) => wire.clone(),
        _ => unreachable!(),
    };
    apply_request(&mut original, &persisted, present_revision("r1"));

    let mut controller = RuntimeUiStateController::new_with_authority(
        test_seeds(false, false),
        present_revision("r1"),
        RuntimeUiFileStatus::Supported,
        persisted_wire,
    );
    let reset_through = match controller.request_supported_reset() {
        RequestResetResult::Started { through, .. } => through,
        result => panic!("reset failed to start: {result:?}"),
    };
    let reset = controller.take_source_mutation().expect("reset command");
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: reset.id,
        error: RuntimeStateIoError::new("reset failed"),
        active: Some(observation(present_revision("r1"))),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected reset failure: {result:?}"),
    };
    assert!(matches!(
        controller.receipt(reset_through),
        Some(DurabilityOutcome::Failed(_))
    ));
    controller.update_seeds(test_seeds(true, false));

    let (_client, inspection) = begin_recovery(
        &mut controller,
        incident,
        PersistenceRecoveryAction::RetryPending,
    );
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: inspection.barrier,
            attempt: inspection.attempt,
            command_id: inspection.command_id,
            result: RecoveryIoResult::Inspected(Ok(inspected(observation(present_revision("r1"))))),
        }),
        SubmitPersistenceRecoveryResult::Continue { .. }
    ));
    let cleanup = controller
        .take_recovery_io_command()
        .expect("cleanup write");
    let (request, purpose) = match cleanup.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, purpose } => (request, purpose),
        operation => panic!("unexpected cleanup operation: {operation:?}"),
    };
    assert_eq!(purpose.retry_desired_through, None);
    assert_eq!(purpose.cleanup_through, Some(request.accepted_through));
}

#[test]
fn reload_during_cleanup_acknowledgement_recomputes_and_writes_again() {
    let mut original = controller();
    commit_bool(&mut original, InteractionSeedTarget::TopPinned, true);
    commit_bool(&mut original, InteractionSeedTarget::SidePinned, true);
    let first_persisted = original.take_source_mutation().unwrap();
    apply_request(&mut original, &first_persisted, present_revision("r0.5"));
    let persisted = original.take_source_mutation().unwrap();
    let persisted_wire = match &persisted.kind {
        SourceMutationKind::Replace(wire) => wire.clone(),
        _ => unreachable!(),
    };
    apply_request(&mut original, &persisted, present_revision("r1"));

    let mut controller = RuntimeUiStateController::new_with_authority(
        test_seeds(false, false),
        present_revision("r1"),
        RuntimeUiFileStatus::Supported,
        persisted_wire,
    );
    controller.request_supported_reset();
    let reset = controller.take_source_mutation().unwrap();
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: reset.id,
        error: RuntimeStateIoError::new("reset failed"),
        active: Some(observation(present_revision("r1"))),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected reset failure: {result:?}"),
    };
    controller.update_seeds(test_seeds(true, false));
    let (client, inspection) = begin_recovery(
        &mut controller,
        incident,
        PersistenceRecoveryAction::RetryPending,
    );
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: inspection.barrier,
        attempt: inspection.attempt,
        command_id: inspection.command_id,
        result: RecoveryIoResult::Inspected(Ok(inspected(observation(present_revision("r1"))))),
    });
    let first = controller
        .take_recovery_io_command()
        .expect("first cleanup");
    let first_request = match &first.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected operation: {operation:?}"),
    };
    assert!(matches!(
        controller.update_seeds(test_seeds(true, true)),
        UpdateSeedsResult::StagedBehindBarrier { .. }
    ));
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: first.barrier,
            attempt: first.attempt,
            command_id: first.command_id,
            result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
                id: first_request.id,
                applied_through: first_request.accepted_through,
                new_source: present_revision("r2"),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Continue { .. }
    ));
    let second = controller
        .take_recovery_io_command()
        .expect("second cleanup");
    let second_request = match &second.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, purpose } => {
            assert_eq!(purpose.retry_desired_through, None);
            assert!(purpose.cleanup_through.is_some());
            request.clone()
        }
        operation => panic!("unexpected operation: {operation:?}"),
    };
    assert_eq!(second_request.expected_source, present_revision("r2"));
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: second.barrier,
            attempt: second.attempt,
            command_id: second.command_id,
            result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
                id: second_request.id,
                applied_through: second_request.accepted_through,
                new_source: present_revision("r3"),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Recovered { .. })
    ));
}

#[test]
fn failed_recovery_cleanup_receipt_stays_failed_and_retry_allocates_a_new_one() {
    let mut original = controller();
    commit_bool(&mut original, InteractionSeedTarget::TopPinned, true);
    let persisted = original.take_source_mutation().unwrap();
    let persisted_wire = match &persisted.kind {
        SourceMutationKind::Replace(wire) => wire.clone(),
        kind => panic!("unexpected source kind: {kind:?}"),
    };
    apply_request(&mut original, &persisted, present_revision("r1"));

    let mut controller = RuntimeUiStateController::new_with_authority(
        test_seeds(false, false),
        present_revision("r1"),
        RuntimeUiFileStatus::Supported,
        persisted_wire,
    );
    controller.request_supported_reset();
    let reset = controller.take_source_mutation().unwrap();
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: reset.id,
        error: RuntimeStateIoError::new("reset failed"),
        active: Some(observation(present_revision("r1"))),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected reset failure: {result:?}"),
    };
    controller.update_seeds(test_seeds(true, false));
    let (client, inspection) = begin_recovery(
        &mut controller,
        incident,
        PersistenceRecoveryAction::RetryPending,
    );
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: inspection.barrier,
        attempt: inspection.attempt,
        command_id: inspection.command_id,
        result: RecoveryIoResult::Inspected(Ok(inspected(observation(present_revision("r1"))))),
    });
    let cleanup = controller.take_recovery_io_command().unwrap();
    let first_request = match &cleanup.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, purpose } => {
            assert_eq!(purpose.retry_desired_through, None);
            assert_eq!(purpose.cleanup_through, Some(request.accepted_through));
            request.clone()
        }
        operation => panic!("unexpected operation: {operation:?}"),
    };
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: cleanup.barrier,
        attempt: cleanup.attempt,
        command_id: cleanup.command_id,
        result: RecoveryIoResult::SourceMutation(SourceMutationResult::Failed {
            id: first_request.id,
            error: RuntimeStateIoError::new("cleanup failed"),
            active: Some(observation(present_revision("r1"))),
            recovery_artifacts: Vec::new(),
            path_effect: RuntimeStateFailurePathEffect::Known(
                RuntimeStateObservedPathEffect::Untouched,
            ),
        }),
    });
    assert!(matches!(
        controller.receipt(first_request.accepted_through),
        Some(DurabilityOutcome::Failed(error)) if error.message() == "cleanup failed"
    ));
    let recovery = match client.completion.try_recv() {
        Some(PersistenceRecoveryResult::StillUnhealthy { recovery, .. }) => recovery,
        result => panic!("unexpected recovery result: {result:?}"),
    };
    let retry = match controller.begin_persistence_recovery(PersistenceRecoveryRequest {
        recovery,
        action: PersistenceRecoveryAction::RetryPending,
    }) {
        BeginPersistenceRecoveryResult::Started { client, .. } => client,
        result => panic!("retry did not begin: {result:?}"),
    };
    let reinspection = controller.take_recovery_io_command().unwrap();
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: reinspection.barrier,
        attempt: reinspection.attempt,
        command_id: reinspection.command_id,
        result: RecoveryIoResult::Inspected(Ok(inspected(observation(present_revision("r1"))))),
    });
    let second = controller.take_recovery_io_command().unwrap();
    let second_request = match second.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, purpose } => {
            assert_eq!(purpose.cleanup_through, Some(request.accepted_through));
            request
        }
        operation => panic!("unexpected operation: {operation:?}"),
    };
    assert!(second_request.accepted_through > first_request.accepted_through);
    assert!(retry.completion.try_recv().is_none());
}

#[test]
fn external_conflict_pruning_keeps_recovery_barrier_until_cleanup_ack() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let (_, incident) = fail_current_replace(&mut controller, "temporary");
    let (client, inspection) = begin_recovery(
        &mut controller,
        incident,
        PersistenceRecoveryAction::RetryPending,
    );
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: inspection.barrier,
        attempt: inspection.attempt,
        command_id: inspection.command_id,
        result: RecoveryIoResult::Inspected(Ok(inspected(observation(missing_revision())))),
    });
    let write = controller.take_recovery_io_command().expect("retry write");
    let request = match &write.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected operation: {operation:?}"),
    };
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: write.barrier,
        attempt: write.attempt,
        command_id: write.command_id,
        result: RecoveryIoResult::SourceMutation(
            SourceMutationResult::SourceChangedBeforeMutation {
                id: request.id,
                active: observation(present_revision("external")),
            },
        ),
    });
    let reinspection = controller
        .take_recovery_io_command()
        .expect("external authority reinspection");
    assert!(matches!(
        controller.update_seeds(test_seeds(true, false)),
        UpdateSeedsResult::StagedBehindBarrier { .. }
    ));
    let authority = observation(present_revision("fresh-external"));
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: reinspection.barrier,
            attempt: reinspection.attempt,
            command_id: reinspection.command_id,
            result: RecoveryIoResult::Inspected(Ok(RecoveryInspection::new(
                authority.clone(),
                Some(wire_with_top_pinned(true)),
            ))),
        }),
        SubmitPersistenceRecoveryResult::Continue { .. }
    ));
    assert!(controller.active_barrier().is_some());
    assert!(client.completion.try_recv().is_none());
    let cleanup = controller
        .take_recovery_io_command()
        .expect("canonical cleanup");
    let cleanup_request = match &cleanup.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected operation: {operation:?}"),
    };
    let cleanup_source = present_revision("external-cleanup");
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: cleanup.barrier,
            attempt: cleanup.attempt,
            command_id: cleanup.command_id,
            result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
                id: cleanup_request.id,
                applied_through: cleanup_request.accepted_through,
                new_source: cleanup_source.clone(),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::ExternalAuthorityInstalled {
            authority: installed,
            ..
        }) if installed == authority
    ));
    assert_eq!(controller.pipeline().stable_source(), &cleanup_source);
    assert!(controller.model().is_empty());
    assert!(controller.active_barrier().is_none());
}

#[test]
fn external_reinspection_settles_an_abandoned_cleanup_receipt_after_reload() {
    let persisted_wire = wire_with_top_pinned(true);
    let mut controller = RuntimeUiStateController::new_with_authority(
        test_seeds(false, false),
        present_revision("r1"),
        RuntimeUiFileStatus::Supported,
        persisted_wire,
    );
    controller.request_supported_reset();
    let prerequisite = controller.take_source_mutation().unwrap();
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: prerequisite.id,
        error: RuntimeStateIoError::new("prerequisite failed"),
        active: Some(observation(present_revision("r1"))),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected prerequisite failure: {result:?}"),
    };
    controller.update_seeds(test_seeds(true, false));
    let (client, inspection) = begin_recovery(
        &mut controller,
        incident,
        PersistenceRecoveryAction::RetryPending,
    );
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: inspection.barrier,
        attempt: inspection.attempt,
        command_id: inspection.command_id,
        result: RecoveryIoResult::Inspected(Ok(inspected(observation(present_revision("r1"))))),
    });
    let cleanup = controller.take_recovery_io_command().unwrap();
    let request = match &cleanup.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected cleanup operation: {operation:?}"),
    };
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: cleanup.barrier,
        attempt: cleanup.attempt,
        command_id: cleanup.command_id,
        result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
            id: request.id,
            applied_through: AcceptedStateRevision(request.accepted_through.get() + 1),
            new_source: present_revision("invalid-ack"),
            recovery_artifacts: Vec::new(),
        }),
    });
    let recovery = match client.completion.try_recv() {
        Some(PersistenceRecoveryResult::StillUnhealthy { recovery, .. }) => recovery,
        result => panic!("unexpected invalid acknowledgement result: {result:?}"),
    };
    assert!(matches!(
        controller.update_seeds(test_seeds(true, true)),
        UpdateSeedsResult::StagedBehindBarrier { .. }
    ));
    let retry = match controller.begin_persistence_recovery(PersistenceRecoveryRequest {
        recovery,
        action: PersistenceRecoveryAction::RetryPending,
    }) {
        BeginPersistenceRecoveryResult::Started { client, .. } => client,
        result => panic!("retry did not begin: {result:?}"),
    };
    let reinspection = controller.take_recovery_io_command().unwrap();
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: reinspection.barrier,
        attempt: reinspection.attempt,
        command_id: reinspection.command_id,
        result: RecoveryIoResult::Inspected(Ok(RecoveryInspection::new(
            observation(present_revision("external")),
            Some(RuntimeUiWireState::default()),
        ))),
    });

    assert!(matches!(
        retry.completion.try_recv(),
        Some(PersistenceRecoveryResult::ExternalAuthorityInstalled { .. })
    ));
    assert_eq!(
        controller.receipt(request.accepted_through),
        Some(&DurabilityOutcome::ExternalSourceWon)
    );
    assert!(controller.pipeline().settled_through() >= request.accepted_through);
    assert!(controller.active_barrier().is_none());
}

#[test]
fn preserve_invalid_settles_an_abandoned_cleanup_receipt() {
    let persisted_wire = wire_with_top_pinned(true);
    let mut controller = RuntimeUiStateController::new_with_authority(
        test_seeds(false, false),
        present_revision("r1"),
        RuntimeUiFileStatus::Supported,
        persisted_wire,
    );
    controller.request_supported_reset();
    let prerequisite = controller.take_source_mutation().unwrap();
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: prerequisite.id,
        error: RuntimeStateIoError::new("prerequisite failed"),
        active: Some(observation(present_revision("r1"))),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected prerequisite failure: {result:?}"),
    };
    controller.update_seeds(test_seeds(true, false));
    let (client, inspection) = begin_recovery(
        &mut controller,
        incident,
        PersistenceRecoveryAction::RetryPending,
    );
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: inspection.barrier,
        attempt: inspection.attempt,
        command_id: inspection.command_id,
        result: RecoveryIoResult::Inspected(Ok(inspected(observation(present_revision("r1"))))),
    });
    let cleanup = controller.take_recovery_io_command().unwrap();
    let request = match &cleanup.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected cleanup operation: {operation:?}"),
    };
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: cleanup.barrier,
        attempt: cleanup.attempt,
        command_id: cleanup.command_id,
        result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
            id: request.id,
            applied_through: AcceptedStateRevision(request.accepted_through.get() + 1),
            new_source: present_revision("invalid-ack"),
            recovery_artifacts: Vec::new(),
        }),
    });
    let recovery = match client.completion.try_recv() {
        Some(PersistenceRecoveryResult::StillUnhealthy { recovery, .. }) => recovery,
        result => panic!("unexpected invalid acknowledgement result: {result:?}"),
    };

    let request_client = match controller.begin_persistence_recovery(PersistenceRecoveryRequest {
        recovery,
        action: PersistenceRecoveryAction::RequestPreserveInvalidReset,
    }) {
        BeginPersistenceRecoveryResult::Started { client, .. } => client,
        result => panic!("preserve request did not begin: {result:?}"),
    };
    let request_inspection = controller.take_recovery_io_command().unwrap();
    let invalid = invalid_observation("confirmed-invalid");
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: request_inspection.barrier,
        attempt: request_inspection.attempt,
        command_id: request_inspection.command_id,
        result: RecoveryIoResult::Inspected(Ok(inspected(invalid.clone()))),
    });
    let (recovery, confirmation) = match request_client.completion.try_recv() {
        Some(PersistenceRecoveryResult::RequiresInvalidResetConfirmation {
            recovery,
            confirmation,
            ..
        }) => (recovery, confirmation),
        result => panic!("confirmation was not returned: {result:?}"),
    };
    let confirm_client = match controller.begin_persistence_recovery(PersistenceRecoveryRequest {
        recovery,
        action: PersistenceRecoveryAction::ConfirmPreserveInvalidReset(confirmation),
    }) {
        BeginPersistenceRecoveryResult::Started { client, .. } => client,
        result => panic!("confirmation did not begin: {result:?}"),
    };
    let confirm_inspection = controller.take_recovery_io_command().unwrap();
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: confirm_inspection.barrier,
        attempt: confirm_inspection.attempt,
        command_id: confirm_inspection.command_id,
        result: RecoveryIoResult::Inspected(Ok(inspected(invalid.clone()))),
    });
    let preserve = controller.take_recovery_io_command().unwrap();
    let mutation_id = match preserve.operation {
        RecoveryIoOperation::PreserveInvalidIfUnchanged { mutation_id, .. } => mutation_id,
        operation => panic!("unexpected preserve operation: {operation:?}"),
    };
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: preserve.barrier,
        attempt: preserve.attempt,
        command_id: preserve.command_id,
        result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
            id: mutation_id,
            applied_through: AcceptedStateRevision(0),
            new_source: missing_revision(),
            recovery_artifacts: vec![RuntimeStateRecoveryArtifact {
                path: "/tmp/preserved-invalid".into(),
                observation: invalid,
            }],
        }),
    });

    assert!(matches!(
        confirm_client.completion.try_recv(),
        Some(PersistenceRecoveryResult::InvalidSourcePreservedAndReset { .. })
    ));
    assert_eq!(
        controller.receipt(request.accepted_through),
        Some(&DurabilityOutcome::ExternalSourceWon)
    );
}
