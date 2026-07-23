use super::*;

#[test]
fn discard_pending_adopts_observed_authority_and_settles_receipt() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let (_, incident) = fail_current_replace(&mut controller, "temporary");
    let (client, inspection) = begin_recovery(
        &mut controller,
        incident,
        PersistenceRecoveryAction::DiscardPendingAndAdoptObserved,
    );
    let external = present_revision("external-authority");
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: inspection.barrier,
            attempt: inspection.attempt,
            command_id: inspection.command_id,
            result: RecoveryIoResult::Inspected(Ok(inspected(observation(external.clone())))),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::ExternalAuthorityInstalled { authority, .. })
            if authority.revision == external
    ));
    assert!(matches!(
        controller.receipt(through),
        Some(DurabilityOutcome::Failed(error)) if error.message() == "temporary"
    ));
    assert_eq!(controller.pipeline().stable_source(), &external);
    assert!(controller.model().is_empty());
    assert!(controller.active_barrier().is_none());
}

#[test]
fn confirmed_invalid_reset_is_observation_bound_and_retains_artifact() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let failed = controller.take_source_mutation().unwrap();
    let invalid = invalid_observation("invalid-source");
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: failed.id,
        error: RuntimeStateIoError::new("malformed"),
        active: Some(invalid.clone()),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::UnknownAfterMutation,
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected result: {result:?}"),
    };
    let (request_client, inspection) = begin_recovery(
        &mut controller,
        incident,
        PersistenceRecoveryAction::RequestPreserveInvalidReset,
    );
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: inspection.barrier,
        attempt: inspection.attempt,
        command_id: inspection.command_id,
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
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: confirm_inspection.barrier,
            attempt: confirm_inspection.attempt,
            command_id: confirm_inspection.command_id,
            result: RecoveryIoResult::Inspected(Ok(inspected(invalid))),
        }),
        SubmitPersistenceRecoveryResult::Continue { .. }
    ));
    let preserve = controller
        .take_recovery_io_command()
        .expect("preserve command");
    let mutation_id = match preserve.operation {
        RecoveryIoOperation::PreserveInvalidIfUnchanged { mutation_id, .. } => mutation_id,
        operation => panic!("unexpected operation: {operation:?}"),
    };
    let artifact_path = std::path::PathBuf::from("/tmp/wayscriber-invalid-preserved");
    let missing = missing_revision();
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: preserve.barrier,
            attempt: preserve.attempt,
            command_id: preserve.command_id,
            result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
                id: mutation_id,
                applied_through: AcceptedStateRevision(0),
                new_source: missing.clone(),
                recovery_artifacts: vec![RuntimeStateRecoveryArtifact {
                    path: artifact_path.clone(),
                    observation: invalid_observation("invalid-source"),
                }],
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        confirm_client.completion.try_recv(),
        Some(PersistenceRecoveryResult::InvalidSourcePreservedAndReset {
            path_effect: RuntimeStatePostClaimPathEffect::QuarantinedAndRetained {
                recovery_path,
            },
            ..
        }) if recovery_path == artifact_path
    ));
    assert!(matches!(
        controller.receipt(through),
        Some(DurabilityOutcome::Failed(error)) if error.message() == "malformed"
    ));
    assert_eq!(controller.pipeline().stable_source(), &missing);
    assert_eq!(controller.authority_epoch(), 2);
    assert!(controller.active_barrier().is_none());
}

#[test]
fn confirmed_invalid_reset_rejects_artifact_from_another_path_identity() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let failed = controller.take_source_mutation().unwrap();
    let invalid = invalid_observation("same-invalid-bytes");
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: failed.id,
        error: RuntimeStateIoError::new("malformed"),
        active: Some(invalid.clone()),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::UnknownAfterMutation,
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected result: {result:?}"),
    };
    let (request_client, inspection) = begin_recovery(
        &mut controller,
        incident,
        PersistenceRecoveryAction::RequestPreserveInvalidReset,
    );
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: inspection.barrier,
        attempt: inspection.attempt,
        command_id: inspection.command_id,
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
        result: RecoveryIoResult::Inspected(Ok(inspected(invalid))),
    });
    let preserve = controller
        .take_recovery_io_command()
        .expect("preserve command");
    let mutation_id = match preserve.operation {
        RecoveryIoOperation::PreserveInvalidIfUnchanged { mutation_id, .. } => mutation_id,
        operation => panic!("unexpected operation: {operation:?}"),
    };
    let wrong_path_observation = RuntimeStateSourceObservation {
        revision: present_revision_at(
            RuntimeStatePathIdentity::direct("/tmp/different-runtime-ui-state.toml"),
            "same-invalid-bytes",
        ),
        envelope: RuntimeStateObservedEnvelope::PresentWithoutReadableVersion,
    };
    assert!(matches!(
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
                    observation: wrong_path_observation,
                }],
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        confirm_client.completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy { error, .. })
            if error.message().contains("did not report an artifact matching")
    ));
    assert!(controller.active_barrier().is_some());
}

#[test]
fn changed_invalid_source_during_preserve_returns_observation_changed() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let failed = controller.take_source_mutation().unwrap();
    let confirmed_source = invalid_observation("invalid-confirmed");
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: failed.id,
        error: RuntimeStateIoError::new("malformed"),
        active: Some(confirmed_source.clone()),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::UnknownAfterMutation,
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected result: {result:?}"),
    };
    let (request_client, inspection) = begin_recovery(
        &mut controller,
        incident,
        PersistenceRecoveryAction::RequestPreserveInvalidReset,
    );
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: inspection.barrier,
        attempt: inspection.attempt,
        command_id: inspection.command_id,
        result: RecoveryIoResult::Inspected(Ok(inspected(confirmed_source.clone()))),
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
        result: RecoveryIoResult::Inspected(Ok(inspected(confirmed_source.clone()))),
    });
    let preserve = controller
        .take_recovery_io_command()
        .expect("preserve command");
    let mutation_id = match preserve.operation {
        RecoveryIoOperation::PreserveInvalidIfUnchanged { mutation_id, .. } => mutation_id,
        operation => panic!("unexpected operation: {operation:?}"),
    };
    let writer_observation = invalid_observation("invalid-after-claim");
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: preserve.barrier,
            attempt: preserve.attempt,
            command_id: preserve.command_id,
            result: RecoveryIoResult::SourceMutation(
                SourceMutationResult::SourceChangedBeforeMutation {
                    id: mutation_id,
                    active: writer_observation,
                },
            ),
        }),
        SubmitPersistenceRecoveryResult::Continue { .. }
    ));
    let reinspection = controller
        .take_recovery_io_command()
        .expect("invalid source reinspection");
    let active = invalid_observation("invalid-fresh");
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: reinspection.barrier,
            attempt: reinspection.attempt,
            command_id: reinspection.command_id,
            result: RecoveryIoResult::Inspected(Ok(inspected(active.clone()))),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        confirm_client.completion.try_recv(),
        Some(PersistenceRecoveryResult::ObservationChanged {
            confirmed,
            active: returned,
            path_effect: RuntimeStateObservedPathEffect::Untouched,
            ..
        }) if confirmed == confirmed_source && returned == active
    ));
    assert_eq!(controller.authority_epoch(), 1);
    assert!(controller.active_barrier().is_some());
}

#[test]
fn preserve_invalid_without_reported_artifact_remains_unhealthy() {
    let (mut controller, through, incident, client, preserve, mutation_id) =
        begin_confirmed_invalid_preserve();
    let missing = missing_revision();
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: preserve.barrier,
            attempt: preserve.attempt,
            command_id: preserve.command_id,
            result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
                id: mutation_id,
                applied_through: AcceptedStateRevision(0),
                new_source: missing.clone(),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy {
            error,
            active: Some(active),
            evidence,
            ..
        }) if error.message().contains("did not report")
            && active == observation(missing.clone())
            && evidence.recovery_artifacts.is_empty()
    ));
    assert_eq!(controller.pipeline().stable_source(), &missing);
    assert_eq!(controller.authority_epoch(), 2);
    assert!(matches!(
        controller.receipt(through),
        Some(DurabilityOutcome::Failed(error)) if error.message() == "malformed"
    ));
    assert!(controller.active_barrier().is_some());
}

#[test]
fn cancellation_does_not_accept_a_preserve_without_a_retained_artifact() {
    let (mut controller, _, incident, client, preserve, mutation_id) =
        begin_confirmed_invalid_preserve();
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
            barrier: preserve.barrier,
            attempt: preserve.attempt,
            command_id: preserve.command_id,
            result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
                id: mutation_id,
                applied_through: AcceptedStateRevision(0),
                new_source: missing_revision(),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy { error, .. })
            if error.message().contains("did not report")
    ));
    assert!(controller.active_barrier().is_some());
}

#[test]
fn preserve_invalid_reports_a_source_mutation_in_flight() {
    let (controller, _, incident, _client, preserve, _) = begin_confirmed_invalid_preserve();
    assert!(matches!(
        controller.active_barrier(),
        Some(ActiveControllerBarrier {
            phase: ControllerBarrierPhase::Recovering {
                incident: active_incident,
                attempt,
                step: RecoveryAttemptStep::SourceMutationInFlight(command),
            },
            ..
        }) if *active_incident == incident
            && *attempt == preserve.attempt
            && *command == preserve.command_id
    ));
}

#[test]
fn preserve_invalid_that_reports_a_present_source_remains_unhealthy() {
    let (mut controller, through, incident, client, preserve, mutation_id) =
        begin_confirmed_invalid_preserve();
    let unexpected = present_revision("unexpected-preserve-output");
    let artifact = RuntimeStateRecoveryArtifact {
        path: "/tmp/preserved-invalid".into(),
        observation: invalid_observation("preserved"),
    };
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: preserve.barrier,
            attempt: preserve.attempt,
            command_id: preserve.command_id,
            result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
                id: mutation_id,
                applied_through: AcceptedStateRevision(0),
                new_source: unexpected.clone(),
                recovery_artifacts: vec![artifact.clone()],
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy {
            error,
            active: Some(active),
            evidence,
            ..
        }) if error.message().contains("did not leave")
            && active.revision == unexpected
            && evidence.recovery_artifacts == vec![artifact]
    ));
    assert_eq!(controller.pipeline().stable_source(), &missing_revision());
    assert_eq!(controller.authority_epoch(), 1);
    assert!(matches!(
        controller.receipt(through),
        Some(DurabilityOutcome::Failed(error)) if error.message() == "malformed"
    ));
    assert!(controller.active_barrier().is_some());
}

#[test]
fn preserve_invalid_rejects_duplicate_recovery_artifact_paths() {
    let (mut controller, _, incident, client, preserve, mutation_id) =
        begin_confirmed_invalid_preserve();
    let duplicate_path = std::path::PathBuf::from("/tmp/duplicate-preserved-invalid");
    assert!(matches!(
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
                recovery_artifacts: vec![
                    RuntimeStateRecoveryArtifact {
                        path: duplicate_path.clone(),
                        observation: invalid_observation("first-preserved-copy"),
                    },
                    RuntimeStateRecoveryArtifact {
                        path: duplicate_path,
                        observation: invalid_observation("second-preserved-copy"),
                    },
                ],
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy { error, .. })
            if error.message().contains("DuplicateRecoveryArtifactPath")
    ));
    assert_eq!(controller.authority_epoch(), 1);
    assert!(controller.active_barrier().is_some());
}

#[test]
fn retry_does_not_overwrite_an_exact_invalid_source() {
    let invalid = invalid_observation("startup-invalid");
    let (mut controller, incident) = RuntimeUiStateController::new_startup_unhealthy(
        test_seeds(false, false),
        invalid.clone(),
        RuntimeStateIoError::new("malformed startup state"),
        Vec::new(),
        RuntimeStateFailurePathEffect::Known(RuntimeStateObservedPathEffect::Untouched),
    );
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
            result: RecoveryIoResult::Inspected(Ok(inspected(invalid.clone()))),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy {
            error,
            active: Some(active),
            ..
        }) if error.message().contains("not safely writable") && active == invalid
    ));
    assert!(controller.take_recovery_io_command().is_none());
    assert_eq!(controller.authority_epoch(), 1);
    assert!(controller.active_barrier().is_some());
}

#[test]
fn retry_does_not_replay_pending_state_after_unknown_mutation_effects() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let failed = controller.take_source_mutation().unwrap();
    let exact = observation(failed.expected_source.clone());
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: failed.id,
        error: RuntimeStateIoError::new("uncertain failure"),
        active: Some(exact.clone()),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::UnknownAfterMutation,
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected result: {result:?}"),
    };
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
            result: RecoveryIoResult::Inspected(Ok(inspected(exact.clone()))),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy {
            error,
            active: Some(active),
            ..
        }) if error.message().contains("effects are unknown") && active == exact
    ));
    assert!(controller.take_recovery_io_command().is_none());
    assert!(matches!(
        controller.receipt(through),
        Some(DurabilityOutcome::Failed(error)) if error.message() == "uncertain failure"
    ));
    assert!(controller.active_barrier().is_some());
}

#[test]
fn preserve_invalid_requires_an_artifact_matching_the_confirmed_source() {
    let (mut controller, _, incident, client, preserve, mutation_id) =
        begin_confirmed_invalid_preserve();

    assert!(matches!(
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
                    path: "/tmp/unrelated-preserved-artifact".into(),
                    observation: invalid_observation("different-bytes"),
                }],
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy { error, evidence, .. })
            if error.message().contains("confirmed invalid source")
                && evidence.recovery_artifacts.len() == 1
    ));
    assert!(controller.active_barrier().is_some());
}

#[test]
fn preserve_invalid_rejects_a_missing_source_at_another_path_identity() {
    let (mut controller, _, incident, client, preserve, mutation_id) =
        begin_confirmed_invalid_preserve();
    let original_source = controller.pipeline().stable_source().clone();
    let wrong_missing = RuntimeStateSourceRevision::missing(RuntimeStatePathIdentity::direct(
        "/tmp/different-runtime-ui-state.toml",
    ));
    let artifact = RuntimeStateRecoveryArtifact {
        path: "/tmp/preserved-invalid".into(),
        observation: invalid_observation("invalid-source"),
    };

    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: preserve.barrier,
            attempt: preserve.attempt,
            command_id: preserve.command_id,
            result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
                id: mutation_id,
                applied_through: AcceptedStateRevision(0),
                new_source: wrong_missing.clone(),
                recovery_artifacts: vec![artifact.clone()],
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy {
            error,
            active: Some(active),
            evidence,
            ..
        }) if error.message().contains("path identity")
            && active.revision == wrong_missing
            && evidence.recovery_artifacts == vec![artifact]
    ));
    assert_eq!(controller.pipeline().stable_source(), &original_source);
    assert_eq!(controller.authority_epoch(), 1);
    assert!(controller.active_barrier().is_some());
}
