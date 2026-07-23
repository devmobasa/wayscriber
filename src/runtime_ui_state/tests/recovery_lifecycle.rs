use super::*;

#[test]
fn invalid_recovery_acknowledgement_does_not_strand_in_flight_mutation() {
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
                applied_through: AcceptedStateRevision(request.accepted_through.get() + 1),
                new_source: present_revision("invalid-ack"),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    let recovery = match client.completion.try_recv() {
        Some(PersistenceRecoveryResult::StillUnhealthy {
            recovery,
            active: Some(active),
            ..
        }) if active.revision == present_revision("invalid-ack")
            && active.envelope == RuntimeStateObservedEnvelope::Version(1) =>
        {
            recovery
        }
        result => panic!("unexpected invalid acknowledgement result: {result:?}"),
    };
    assert!(!controller.pipeline().has_source_mutation_in_flight());

    let retry = match controller.begin_persistence_recovery(PersistenceRecoveryRequest {
        recovery,
        action: PersistenceRecoveryAction::RetryPending,
    }) {
        BeginPersistenceRecoveryResult::Started { client, .. } => client,
        result => panic!("retry failed: {result:?}"),
    };
    let reinspection = controller.take_recovery_io_command().unwrap();
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: reinspection.barrier,
            attempt: reinspection.attempt,
            command_id: reinspection.command_id,
            result: RecoveryIoResult::Inspected(Ok(inspected(observation(missing_revision())))),
        }),
        SubmitPersistenceRecoveryResult::Continue { .. }
    ));
    assert!(matches!(
        controller.take_recovery_io_command().unwrap().operation,
        RecoveryIoOperation::PersistCanonicalIfUnchanged { .. }
    ));
    drop(retry);
}

#[test]
fn cancellation_wins_when_recovery_acknowledgement_is_invalid() {
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
    let write = controller.take_recovery_io_command().unwrap();
    let request = match &write.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected operation: {operation:?}"),
    };
    let RecoveryAttemptClient {
        cancellation,
        completion,
    } = client;
    assert!(matches!(
        controller.cancel_persistence_recovery(cancellation),
        CancelPersistenceRecoveryResult::PendingIrrevocableIo { .. }
    ));
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: write.barrier,
            attempt: write.attempt,
            command_id: write.command_id,
            result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
                id: request.id,
                applied_through: AcceptedStateRevision(request.accepted_through.get() + 1),
                new_source: present_revision("invalid-ack-after-cancel"),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        completion.try_recv(),
        Some(PersistenceRecoveryResult::Cancelled {
            active: Some(active),
            ..
        }) if active.revision == present_revision("invalid-ack-after-cancel")
            && active.envelope == RuntimeStateObservedEnvelope::Version(1)
    ));
    assert!(!controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.active_barrier().is_some());
}

#[test]
fn cancellation_waits_for_the_legitimate_writer_after_protocol_failure() {
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
    let write = controller.take_recovery_io_command().unwrap();
    let request = match &write.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected operation: {operation:?}"),
    };
    let RecoveryAttemptClient {
        cancellation,
        completion,
    } = client;
    assert!(matches!(
        controller.cancel_persistence_recovery(cancellation),
        CancelPersistenceRecoveryResult::PendingIrrevocableIo { .. }
    ));

    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: write.barrier,
            attempt: write.attempt,
            command_id: write.command_id,
            result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
                id: SourceMutationId(request.id.get() + 1),
                applied_through: request.accepted_through,
                new_source: present_revision("uncertain-after-protocol-failure"),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::BlockedProtocolFailure {
            reinspection_dispatched: None,
            ..
        }
    ));
    assert!(controller.take_recovery_io_command().is_none());
    let active = present_revision("actual-write-after-protocol-failure");
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
                new_source: active.clone(),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        completion.try_recv(),
        Some(PersistenceRecoveryResult::Cancelled {
            active: Some(returned),
            ..
        }) if returned == observation(active)
    ));
    assert!(controller.take_recovery_io_command().is_none());
    assert!(controller.active_barrier().is_some());
}

#[test]
fn shutdown_waits_for_the_legitimate_writer_after_protocol_failure() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let (_, incident) = fail_current_replace(&mut controller, "temporary");
    let (_client, inspection) = begin_recovery(
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
    let write = controller.take_recovery_io_command().unwrap();
    let request = match &write.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected operation: {operation:?}"),
    };
    controller.request_shutdown().unwrap();

    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: write.barrier,
            attempt: write.attempt,
            command_id: write.command_id,
            result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
                id: SourceMutationId(request.id.get() + 1),
                applied_through: request.accepted_through,
                new_source: present_revision("uncertain-during-shutdown"),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::BlockedProtocolFailure {
            reinspection_dispatched: None,
            ..
        }
    ));
    assert!(controller.take_recovery_io_command().is_none());
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
                new_source: present_revision("actual-write-during-shutdown"),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(controller.active_barrier().is_none());
    assert!(controller.pipeline().shutdown_complete());
}

#[test]
fn cancel_during_recovery_write_waits_for_evidence_and_can_finish_on_retry() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
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
    let write = controller.take_recovery_io_command().unwrap();
    let request = match &write.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        _ => unreachable!(),
    };
    let RecoveryAttemptClient {
        cancellation,
        completion,
    } = client;
    assert!(matches!(
        controller.cancel_persistence_recovery(cancellation),
        CancelPersistenceRecoveryResult::PendingIrrevocableIo { .. }
    ));
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
                new_source: present_revision("applied-before-cancel"),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    let recovery = match completion.try_recv() {
        Some(PersistenceRecoveryResult::Cancelled {
            recovery,
            active: Some(active),
            ..
        }) => {
            assert_eq!(active.revision, present_revision("applied-before-cancel"));
            recovery
        }
        result => panic!("unexpected cancellation result: {result:?}"),
    };
    assert!(matches!(
        controller.receipt(through),
        Some(DurabilityOutcome::Failed(error)) if error.message() == "temporary"
    ));
    let retry = match controller.begin_persistence_recovery(PersistenceRecoveryRequest {
        recovery,
        action: PersistenceRecoveryAction::RetryPending,
    }) {
        BeginPersistenceRecoveryResult::Started { client, .. } => client,
        result => panic!("retry failed: {result:?}"),
    };
    let inspect = controller.take_recovery_io_command().unwrap();
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: inspect.barrier,
            attempt: inspect.attempt,
            command_id: inspect.command_id,
            result: RecoveryIoResult::Inspected(Ok(inspected(observation(present_revision(
                "applied-before-cancel",
            ))))),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        retry.completion.try_recv(),
        Some(PersistenceRecoveryResult::Recovered { .. })
    ));
}

#[test]
fn cancelled_recovery_applies_reload_and_retains_cleanup_work() {
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
    let write = controller.take_recovery_io_command().unwrap();
    let request = match &write.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected operation: {operation:?}"),
    };
    let RecoveryAttemptClient {
        cancellation,
        completion,
    } = client;
    assert!(matches!(
        controller.cancel_persistence_recovery(cancellation),
        CancelPersistenceRecoveryResult::PendingIrrevocableIo { .. }
    ));
    assert!(matches!(
        controller.update_seeds(test_seeds(true, false)),
        UpdateSeedsResult::StagedBehindBarrier { .. }
    ));
    let applied = present_revision("applied-before-cancel-with-reload");
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
                new_source: applied.clone(),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    let recovery = match completion.try_recv() {
        Some(PersistenceRecoveryResult::Cancelled { recovery, .. }) => recovery,
        result => panic!("unexpected cancellation result: {result:?}"),
    };
    assert!(controller.model().is_empty());
    assert_eq!(
        controller
            .live_state()
            .get(&InteractionSeedTarget::TopPinned),
        Some(&InteractionSeedValue::Bool(true))
    );

    let retry = match controller.begin_persistence_recovery(PersistenceRecoveryRequest {
        recovery,
        action: PersistenceRecoveryAction::RetryPending,
    }) {
        BeginPersistenceRecoveryResult::Started { client, .. } => client,
        result => panic!("retry failed: {result:?}"),
    };
    let inspect = controller.take_recovery_io_command().unwrap();
    let result = controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: inspect.barrier,
        attempt: inspect.attempt,
        command_id: inspect.command_id,
        result: RecoveryIoResult::Inspected(Ok(inspected(observation(applied)))),
    });
    assert!(
        matches!(result, SubmitPersistenceRecoveryResult::Continue { .. }),
        "unexpected retry result: {result:?}"
    );
    assert!(matches!(
        controller.take_recovery_io_command().unwrap().operation,
        RecoveryIoOperation::PersistCanonicalIfUnchanged { .. }
    ));
    drop(retry);
}

#[test]
fn cancel_during_recovery_conflict_rotates_without_dispatching_reinspection() {
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
    let write = controller.take_recovery_io_command().unwrap();
    let request = match &write.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected operation: {operation:?}"),
    };
    let RecoveryAttemptClient {
        cancellation,
        completion,
    } = client;
    assert!(matches!(
        controller.cancel_persistence_recovery(cancellation),
        CancelPersistenceRecoveryResult::PendingIrrevocableIo { .. }
    ));
    let active = observation(present_revision("external"));
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: write.barrier,
            attempt: write.attempt,
            command_id: write.command_id,
            result: RecoveryIoResult::SourceMutation(
                SourceMutationResult::SourceChangedBeforeMutation {
                    id: request.id,
                    active: active.clone(),
                },
            ),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        completion.try_recv(),
        Some(PersistenceRecoveryResult::Cancelled {
            active: Some(returned),
            ..
        }) if returned == active
    ));
    assert!(controller.take_recovery_io_command().is_none());
    assert!(controller.active_barrier().is_some());
}

#[test]
fn malformed_active_write_completion_waits_for_the_legitimate_writer_result() {
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
    let write = controller.take_recovery_io_command().unwrap();
    let request = match &write.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected operation: {operation:?}"),
    };
    let completion = RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: write.barrier,
        attempt: write.attempt,
        command_id: write.command_id,
        result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
            id: SourceMutationId(request.id.get() + 1),
            applied_through: request.accepted_through,
            new_source: present_revision("uncertain"),
            recovery_artifacts: Vec::new(),
        }),
    };
    let mismatch = controller.submit_persistence_recovery_io(completion.clone());
    assert!(matches!(
        mismatch,
        SubmitPersistenceRecoveryResult::BlockedProtocolFailure {
            reason: RecoveryCompletionProtocolError::UnexpectedSourceMutationIdentity,
            reinspection_dispatched: None,
            ..
        }
    ));
    assert!(controller.active_barrier().is_some());
    assert!(controller.take_recovery_io_command().is_none());
    assert!(matches!(
        controller.submit_persistence_recovery_io(completion),
        SubmitPersistenceRecoveryResult::IgnoredDuplicateAlreadyIntegrated { .. }
    ));

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
                new_source: present_revision("actual-write-completed"),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Continue { .. }
    ));
    let reinspection = controller.take_recovery_io_command().unwrap();
    assert!(matches!(
        reinspection.operation,
        RecoveryIoOperation::Inspect
    ));
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: reinspection.barrier,
            attempt: reinspection.attempt,
            command_id: reinspection.command_id,
            result: RecoveryIoResult::Inspected(Ok(inspected(observation(present_revision(
                "actual-write-completed"
            ))))),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy { .. })
    ));
}

#[test]
fn unknown_completion_is_tracked_while_the_active_write_finishes() {
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
    let write = controller.take_recovery_io_command().unwrap();
    let request = match &write.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected operation: {operation:?}"),
    };

    let unknown = RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: write.barrier,
        attempt: write.attempt,
        command_id: RecoveryCommandId(write.command_id.get() + 100),
        result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
            id: request.id,
            applied_through: request.accepted_through,
            new_source: present_revision("unrelated-completion"),
            recovery_artifacts: Vec::new(),
        }),
    };
    let mismatch = controller.submit_persistence_recovery_io(unknown.clone());
    assert!(matches!(
        mismatch,
        SubmitPersistenceRecoveryResult::BlockedProtocolFailure {
            reason: RecoveryCompletionProtocolError::UnknownCommand,
            reinspection_dispatched: None,
            ..
        }
    ));
    assert!(controller.take_recovery_io_command().is_none());
    assert!(matches!(
        controller.submit_persistence_recovery_io(unknown),
        SubmitPersistenceRecoveryResult::IgnoredDuplicateAlreadyIntegrated { .. }
    ));

    let active_completion = controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: write.barrier,
        attempt: write.attempt,
        command_id: write.command_id,
        result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
            id: request.id,
            applied_through: request.accepted_through,
            new_source: present_revision("actual-write-completed"),
            recovery_artifacts: Vec::new(),
        }),
    });
    assert!(matches!(
        active_completion,
        SubmitPersistenceRecoveryResult::Continue { .. }
    ));
    let reinspection = controller.take_recovery_io_command().unwrap();
    assert!(matches!(
        reinspection.operation,
        RecoveryIoOperation::Inspect
    ));
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: reinspection.barrier,
            attempt: reinspection.attempt,
            command_id: reinspection.command_id,
            result: RecoveryIoResult::Inspected(Ok(inspected(observation(present_revision(
                "actual-write-completed"
            ))))),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy { .. })
    ));
}

#[test]
fn failed_reinspection_retains_the_last_safe_writer_observation() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let failed = controller.take_source_mutation().unwrap();
    let writer_observation = observation(present_revision("writer-observation"));
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: failed.id,
        error: RuntimeStateIoError::new("uncertain write"),
        active: Some(writer_observation.clone()),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::UnknownAfterMutation,
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected failure result: {result:?}"),
    };
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
        result: RecoveryIoResult::Inspected(Err(RuntimeStateInspectionError::new(
            "inspection failed",
        ))),
    });

    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy {
            active: Some(active),
            ..
        }) if active == writer_observation
    ));
}

#[test]
fn inconsistent_reinspection_falls_back_to_the_last_safe_writer_observation() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let failed = controller.take_source_mutation().unwrap();
    let writer_observation = observation(present_revision("writer-observation"));
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: failed.id,
        error: RuntimeStateIoError::new("uncertain write"),
        active: Some(writer_observation.clone()),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::UnknownAfterMutation,
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected failure result: {result:?}"),
    };
    let (client, inspection) = begin_recovery(
        &mut controller,
        incident,
        PersistenceRecoveryAction::RetryPending,
    );
    let inconsistent = RuntimeStateSourceObservation {
        revision: missing_revision(),
        envelope: RuntimeStateObservedEnvelope::Version(1),
    };

    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: inspection.barrier,
        attempt: inspection.attempt,
        command_id: inspection.command_id,
        result: RecoveryIoResult::Inspected(Ok(RecoveryInspection::new(inconsistent, None))),
    });

    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy {
            active: Some(active),
            ..
        }) if active == writer_observation
    ));
}

#[test]
fn failed_conflict_reinspection_retains_the_writer_observation() {
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
    let write = controller.take_recovery_io_command().unwrap();
    let request = match &write.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected operation: {operation:?}"),
    };
    let writer_observation = observation(present_revision("conflicting-writer-source"));
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: write.barrier,
            attempt: write.attempt,
            command_id: write.command_id,
            result: RecoveryIoResult::SourceMutation(
                SourceMutationResult::SourceChangedBeforeMutation {
                    id: request.id,
                    active: writer_observation.clone(),
                },
            ),
        }),
        SubmitPersistenceRecoveryResult::Continue { .. }
    ));
    let reinspection = controller.take_recovery_io_command().unwrap();
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: reinspection.barrier,
            attempt: reinspection.attempt,
            command_id: reinspection.command_id,
            result: RecoveryIoResult::Inspected(Err(RuntimeStateInspectionError::new(
                "reinspection failed",
            ))),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy {
            active: Some(active),
            ..
        }) if active == writer_observation
    ));
}

#[test]
fn cancelling_conflict_reinspection_retains_the_writer_observation() {
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
    let write = controller.take_recovery_io_command().unwrap();
    let request = match &write.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected operation: {operation:?}"),
    };
    let writer_observation = observation(present_revision("conflicting-writer-source"));
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: write.barrier,
            attempt: write.attempt,
            command_id: write.command_id,
            result: RecoveryIoResult::SourceMutation(
                SourceMutationResult::SourceChangedBeforeMutation {
                    id: request.id,
                    active: writer_observation.clone(),
                },
            ),
        }),
        SubmitPersistenceRecoveryResult::Continue { .. }
    ));
    let reinspection = controller.take_recovery_io_command().unwrap();
    let RecoveryAttemptClient {
        cancellation,
        completion,
    } = client;

    assert!(matches!(
        controller.cancel_persistence_recovery(cancellation),
        CancelPersistenceRecoveryResult::Cancelled
    ));
    assert!(matches!(
        completion.try_recv(),
        Some(PersistenceRecoveryResult::Cancelled {
            active: Some(active),
            ..
        }) if active == writer_observation
    ));
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: reinspection.barrier,
            attempt: reinspection.attempt,
            command_id: reinspection.command_id,
            result: RecoveryIoResult::Inspected(Err(RuntimeStateInspectionError::new("late"))),
        }),
        SubmitPersistenceRecoveryResult::IgnoredCancelledReadOnly { .. }
    ));
}
