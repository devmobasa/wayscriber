use super::*;

#[test]
fn persistence_failure_allocates_barrier_before_recovery_handle_checkout() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();
    let result = controller.submit_source_mutation(SourceMutationResult::Failed {
        id: request.id,
        error: RuntimeStateIoError::new("disk full"),
        active: Some(observation(missing_revision())),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    });
    let (barrier, incident) = match result {
        SubmitSourceMutationResult::PersistenceUnhealthy {
            barrier, incident, ..
        } => (barrier, incident),
        result => panic!("unexpected failure result: {result:?}"),
    };
    assert!(matches!(
        controller.active_barrier(),
        Some(ActiveControllerBarrier {
            id,
            phase: ControllerBarrierPhase::PersistenceUnhealthy { incident: active },
            ..
        }) if *id == barrier && *active == incident
    ));
    assert!(matches!(
        controller.receipt(through),
        Some(DurabilityOutcome::Failed(error)) if error.message() == "disk full"
    ));
    assert!(matches!(
        controller.checkout_persistence_recovery_handle(incident),
        CheckoutPersistenceRecoveryHandleResult::CheckedOut(_)
    ));
}

#[test]
fn wrong_controller_rejection_returns_the_exact_recovery_request() {
    let mut owner = controller();
    commit_bool(&mut owner, InteractionSeedTarget::TopPinned, true);
    let request = owner.take_source_mutation().unwrap();
    let incident = match owner.submit_source_mutation(SourceMutationResult::Failed {
        id: request.id,
        error: RuntimeStateIoError::new("failed"),
        active: Some(observation(missing_revision())),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected result: {result:?}"),
    };
    let handle = match owner.checkout_persistence_recovery_handle(incident) {
        CheckoutPersistenceRecoveryHandleResult::CheckedOut(handle) => handle,
        result => panic!("handle checkout failed: {result:?}"),
    };
    let handle_id = handle.handle_id();
    let request = PersistenceRecoveryRequest {
        recovery: handle,
        action: PersistenceRecoveryAction::RetryPending,
    };
    let mut wrong = controller();
    let request = match wrong.begin_persistence_recovery(request) {
        BeginPersistenceRecoveryResult::Rejected {
            request,
            reason: RecoveryBeginRejection::WrongController,
        } => request,
        result => panic!("wrong controller did not preserve request: {result:?}"),
    };
    assert_eq!(request.recovery.handle_id(), handle_id);
    assert!(matches!(
        owner.begin_persistence_recovery(request),
        BeginPersistenceRecoveryResult::Started { .. }
    ));
}

#[test]
fn recovery_inspection_is_split_phase_and_reload_is_staged_while_io_is_held() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let failed = controller.take_source_mutation().unwrap();
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: failed.id,
        error: RuntimeStateIoError::new("temporary failure"),
        active: Some(observation(missing_revision())),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected result: {result:?}"),
    };
    let handle = match controller.checkout_persistence_recovery_handle(incident) {
        CheckoutPersistenceRecoveryHandleResult::CheckedOut(handle) => handle,
        result => panic!("handle checkout failed: {result:?}"),
    };
    let (client, dispatched) =
        match controller.begin_persistence_recovery(PersistenceRecoveryRequest {
            recovery: handle,
            action: PersistenceRecoveryAction::RetryPending,
        }) {
            BeginPersistenceRecoveryResult::Started { client, dispatched } => (client, dispatched),
            result => panic!("recovery did not begin: {result:?}"),
        };
    let command = controller.take_recovery_io_command().unwrap();
    assert_eq!(command.command_id, dispatched);
    assert!(matches!(command.operation, RecoveryIoOperation::Inspect));

    assert!(matches!(
        controller.update_seeds(test_seeds(false, true)),
        UpdateSeedsResult::StagedBehindBarrier { .. }
    ));
    let result = controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: command.barrier,
        attempt: command.attempt,
        command_id: command.command_id,
        result: RecoveryIoResult::Inspected(Ok(inspected(observation(missing_revision())))),
    });
    assert!(matches!(
        result,
        SubmitPersistenceRecoveryResult::Continue { .. }
    ));
    assert!(matches!(
        controller
            .take_recovery_io_command()
            .expect("canonical recovery command")
            .operation,
        RecoveryIoOperation::PersistCanonicalIfUnchanged { .. }
    ));
    drop(client);
}

#[test]
fn dropped_recovery_cancellation_restores_unhealthy_and_late_read_is_consumed_once() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let failed = controller.take_source_mutation().unwrap();
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: failed.id,
        error: RuntimeStateIoError::new("failure"),
        active: Some(observation(missing_revision())),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected result: {result:?}"),
    };
    let handle = match controller.checkout_persistence_recovery_handle(incident) {
        CheckoutPersistenceRecoveryHandleResult::CheckedOut(handle) => handle,
        result => panic!("checkout failed: {result:?}"),
    };
    let client = match controller.begin_persistence_recovery(PersistenceRecoveryRequest {
        recovery: handle,
        action: PersistenceRecoveryAction::RetryPending,
    }) {
        BeginPersistenceRecoveryResult::Started { client, .. } => client,
        result => panic!("begin failed: {result:?}"),
    };
    let command = controller.take_recovery_io_command().unwrap();
    let RecoveryAttemptClient {
        cancellation,
        completion,
    } = client;
    drop(cancellation);
    controller.drain_lifecycle_controls();
    assert!(matches!(
        completion.try_recv(),
        Some(PersistenceRecoveryResult::Cancelled { .. })
    ));

    let late = RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: command.barrier,
        attempt: command.attempt,
        command_id: command.command_id,
        result: RecoveryIoResult::Inspected(Ok(inspected(observation(missing_revision())))),
    };
    assert!(matches!(
        controller.submit_persistence_recovery_io(late.clone()),
        SubmitPersistenceRecoveryResult::IgnoredCancelledReadOnly { .. }
    ));
    assert!(matches!(
        controller.submit_persistence_recovery_io(late),
        SubmitPersistenceRecoveryResult::IgnoredDuplicateAlreadyIntegrated { .. }
    ));
}

#[test]
fn shutdown_owns_a_queued_recovery_cancellation() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let (_, incident) = fail_current_replace(&mut controller, "failure");
    let handle = match controller.checkout_persistence_recovery_handle(incident) {
        CheckoutPersistenceRecoveryHandleResult::CheckedOut(handle) => handle,
        result => panic!("checkout failed: {result:?}"),
    };
    let client = match controller.begin_persistence_recovery(PersistenceRecoveryRequest {
        recovery: handle,
        action: PersistenceRecoveryAction::RetryPending,
    }) {
        BeginPersistenceRecoveryResult::Started { client, .. } => client,
        result => panic!("begin failed: {result:?}"),
    };
    let inspection = controller.take_recovery_io_command().unwrap();
    let RecoveryAttemptClient {
        cancellation,
        completion,
    } = client;
    drop(cancellation);

    controller.request_shutdown().unwrap();

    assert!(matches!(
        completion.try_recv(),
        Some(PersistenceRecoveryResult::Shutdown {
            incident: completed_incident,
            ..
        }) if completed_incident == incident
    ));
    assert!(controller.active_barrier().is_none());
    assert!(controller.pipeline().shutdown_complete());
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: inspection.barrier,
            attempt: inspection.attempt,
            command_id: inspection.command_id,
            result: RecoveryIoResult::Inspected(Ok(inspected(observation(missing_revision())))),
        }),
        SubmitPersistenceRecoveryResult::IgnoredCancelledReadOnly { .. }
    ));
}

#[test]
fn invalid_state_reset_request_is_read_only_and_confirmation_is_bound_to_observation() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let failed = controller.take_source_mutation().unwrap();
    let invalid = invalid_observation("invalid-a");
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: failed.id,
        error: RuntimeStateIoError::new("invalid source"),
        active: Some(invalid.clone()),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::UnknownAfterMutation,
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected result: {result:?}"),
    };
    let handle = match controller.checkout_persistence_recovery_handle(incident) {
        CheckoutPersistenceRecoveryHandleResult::CheckedOut(handle) => handle,
        result => panic!("checkout failed: {result:?}"),
    };
    let RecoveryAttemptClient {
        cancellation,
        completion,
    } = match controller.begin_persistence_recovery(PersistenceRecoveryRequest {
        recovery: handle,
        action: PersistenceRecoveryAction::RequestPreserveInvalidReset,
    }) {
        BeginPersistenceRecoveryResult::Started { client, .. } => client,
        result => panic!("begin failed: {result:?}"),
    };
    let command = controller.take_recovery_io_command().unwrap();
    assert!(matches!(command.operation, RecoveryIoOperation::Inspect));
    let terminal = controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: command.barrier,
        attempt: command.attempt,
        command_id: command.command_id,
        result: RecoveryIoResult::Inspected(Ok(inspected(invalid.clone()))),
    });
    assert!(matches!(
        terminal,
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(controller.take_recovery_io_command().is_none());
    assert_eq!(controller.pipeline().stable_source(), &missing_revision());
    assert!(matches!(
        completion.try_recv(),
        Some(PersistenceRecoveryResult::RequiresInvalidResetConfirmation { observed, .. })
            if observed == invalid
    ));
    drop(cancellation);
}

#[test]
fn failed_receipt_stays_failed_while_recovery_reconstructs_persisted_state() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let (_, incident) = fail_current_replace(&mut controller, "temporary");
    assert!(matches!(
        controller.receipt(through),
        Some(DurabilityOutcome::Failed(error)) if error.message() == "temporary"
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
    let write = controller.take_recovery_io_command().expect("retry write");
    let (request, persisted_wire) = match write.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => {
            let SourceMutationKind::Replace(wire) = &request.kind else {
                panic!("expected replacement");
            };
            (request.clone(), wire.clone())
        }
        operation => panic!("unexpected operation: {operation:?}"),
    };
    assert_eq!(request.accepted_through, through);
    let persisted_source = present_revision("recovered");
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
                new_source: persisted_source.clone(),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Recovered { .. })
    ));
    assert!(matches!(
        controller.receipt(through),
        Some(DurabilityOutcome::Failed(error)) if error.message() == "temporary"
    ));
    assert!(controller.active_barrier().is_none());

    let reconstructed = RuntimeUiStateController::new_with_authority(
        test_seeds(false, false),
        persisted_source,
        RuntimeUiFileStatus::Supported,
        persisted_wire,
    );
    assert_eq!(
        reconstructed
            .live_state()
            .get(&InteractionSeedTarget::TopPinned),
        Some(&InteractionSeedValue::Bool(true))
    );
}

#[test]
fn dropped_recovery_handle_returns_its_lease_to_the_owner() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let (_, incident) = fail_current_replace(&mut controller, "temporary");
    let first = match controller.checkout_persistence_recovery_handle(incident) {
        CheckoutPersistenceRecoveryHandleResult::CheckedOut(handle) => handle,
        result => panic!("checkout failed: {result:?}"),
    };
    let handle_id = first.handle_id();
    drop(first);
    controller.drain_lifecycle_controls();
    let second = match controller.checkout_persistence_recovery_handle(incident) {
        CheckoutPersistenceRecoveryHandleResult::CheckedOut(handle) => handle,
        result => panic!("returned lease was not available: {result:?}"),
    };
    assert_eq!(second.handle_id(), handle_id);
}

#[test]
fn shutdown_settles_unhealthy_receipts_and_closes_the_barrier() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    fail_current_replace(&mut controller, "temporary");
    controller.request_shutdown().unwrap();
    assert!(matches!(
        controller.receipt(through),
        Some(DurabilityOutcome::Failed(error))
            if error.message() == "temporary"
    ));
    assert!(controller.active_barrier().is_none());
    assert!(controller.pipeline().shutdown_complete());
}

#[test]
fn startup_invalid_authority_begins_with_a_recoverable_barrier() {
    let invalid = invalid_observation("malformed");
    let (mut controller, incident) = RuntimeUiStateController::new_startup_unhealthy(
        test_seeds(false, false),
        invalid,
        RuntimeStateIoError::new("malformed runtime state"),
        Vec::new(),
        RuntimeStateFailurePathEffect::Known(RuntimeStateObservedPathEffect::Untouched),
    );
    assert!(matches!(
        controller.active_barrier(),
        Some(ActiveControllerBarrier {
            operation: ControllerBarrierOperation::StartupPersistenceRecovery,
            phase: ControllerBarrierPhase::PersistenceUnhealthy { incident: active },
            ..
        }) if *active == incident
    ));
    assert!(matches!(
        controller.begin_runtime_preview(
            RuntimeUiMutationScope::one(InteractionSeedTarget::TopPinned),
            PreviewRollbackSnapshot::default(),
        ),
        Err(BeginPreviewError::ControllerBusy(_))
    ));
    assert!(matches!(
        controller.checkout_persistence_recovery_handle(incident),
        CheckoutPersistenceRecoveryHandleResult::CheckedOut(_)
    ));
}

#[test]
fn failed_recovery_rotates_handle_and_retains_cumulative_evidence() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let (_, incident) = fail_current_replace(&mut controller, "first failure");
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
    let artifact = RuntimeStateRecoveryArtifact {
        path: "/tmp/recovery-artifact".into(),
        observation: observation(present_revision("artifact")),
    };
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: write.barrier,
        attempt: write.attempt,
        command_id: write.command_id,
        result: RecoveryIoResult::SourceMutation(SourceMutationResult::Failed {
            id: request.id,
            error: RuntimeStateIoError::new("second failure"),
            active: Some(observation(missing_revision())),
            recovery_artifacts: vec![artifact.clone()],
            path_effect: RuntimeStateFailurePathEffect::UnknownAfterMutation,
        }),
    });
    let recovery = match client.completion.try_recv() {
        Some(PersistenceRecoveryResult::StillUnhealthy {
            recovery, evidence, ..
        }) => {
            assert_eq!(evidence.recovery_artifacts, vec![artifact]);
            assert_eq!(evidence.path_effect_history.len(), 2);
            recovery
        }
        result => panic!("unexpected recovery result: {result:?}"),
    };
    assert_eq!(recovery.incident(), incident);
    assert!(matches!(
        controller.begin_persistence_recovery(PersistenceRecoveryRequest {
            recovery,
            action: PersistenceRecoveryAction::RetryPending,
        }),
        BeginPersistenceRecoveryResult::Started { .. }
    ));
}

#[test]
fn startup_reconciles_stale_overrides_and_queues_pruning() {
    let target = InteractionSeedTarget::ItemOrder(ToolbarItemOrderGroup::TopTools);
    let stale_value =
        InteractionSeedValue::ItemOrder(vec![item_ids::TOP_TOOL_MARKER, item_ids::TOP_TOOL_PEN]);
    let mut source = controller();
    let permit = source
        .begin_mutation(RuntimeUiMutationScope::one(target.clone()))
        .unwrap();
    assert!(matches!(
        source.commit(
            permit,
            RuntimeUiMutationValues::one(target.clone(), stale_value).unwrap(),
        ),
        CommitResult::Accepted { .. }
    ));
    let stale_wire = match source.take_source_mutation().unwrap().kind {
        SourceMutationKind::Replace(wire) => wire,
        kind => panic!("unexpected source mutation: {kind:?}"),
    };

    let current_value =
        InteractionSeedValue::ItemOrder(vec![item_ids::TOP_TOOL_PEN, item_ids::TOP_TOOL_ERASER]);
    let mut current_seeds = test_seeds(false, false);
    current_seeds
        .insert(target.clone(), current_value.clone())
        .unwrap();
    let stable_source = present_revision("startup-stale-override");
    let mut reconstructed = RuntimeUiStateController::new_with_authority(
        current_seeds,
        stable_source.clone(),
        RuntimeUiFileStatus::Supported,
        stale_wire,
    );

    assert_eq!(
        reconstructed.live_state().get(&target),
        Some(&current_value)
    );
    assert!(reconstructed.model().get(&target).is_none());
    let cleanup = reconstructed
        .take_source_mutation()
        .expect("startup pruning write");
    assert_eq!(cleanup.expected_source, stable_source);
    assert!(matches!(
        cleanup.kind,
        SourceMutationKind::Replace(ref wire) if wire.model.get(&target).is_none()
    ));
}
