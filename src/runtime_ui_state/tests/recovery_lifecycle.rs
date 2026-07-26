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
fn matching_recovery_write_completion_is_integrated_before_state_failure_shutdown()
-> Result<(), &'static str> {
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
    let Some(write) = controller.take_recovery_io_command() else {
        return Err("fixture must dispatch a canonical recovery write");
    };
    let RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } = &write.operation else {
        return Err("fixture recovery command must be a canonical write");
    };
    let request = request.clone();
    let Some(incident_state) = controller.incident.as_mut() else {
        return Err("fixture must retain the persistence incident");
    };
    incident_state.handle.availability = RecoveryHandleAvailability::Available;

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
                new_source: present_revision("applied-before-state-failure"),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Shutdown {
            reason: RecoveryShutdownReason::StateFailure(RecoveryStateFailure::HandleStateMismatch),
            ..
        })
    ));
    assert!(!controller.pipeline().has_source_mutation_in_flight());
    assert_eq!(
        controller.pipeline().stable_source(),
        &present_revision("applied-before-state-failure")
    );
    assert!(matches!(
        controller.receipt(through),
        Some(DurabilityOutcome::Failed(error)) if error.message() == "temporary"
    ));
    assert!(controller.pipeline().shutdown_complete());
    Ok(())
}

#[test]
fn malformed_recovery_write_completion_terminally_settles_state_failure_shutdown()
-> Result<(), &'static str> {
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
    let Some(write) = controller.take_recovery_io_command() else {
        return Err("fixture must dispatch a canonical recovery write");
    };
    let RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } = &write.operation else {
        return Err("fixture recovery command must be a canonical write");
    };
    let Some(incident_state) = controller.incident.as_mut() else {
        return Err("fixture must retain the persistence incident");
    };
    incident_state.handle.availability = RecoveryHandleAvailability::Available;

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
                new_source: present_revision("malformed-before-state-failure"),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Shutdown {
            reason: RecoveryShutdownReason::StateFailure(RecoveryStateFailure::HandleStateMismatch),
            ..
        })
    ));
    assert!(!controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.shutdown_complete());
    Ok(())
}

#[test]
fn unrelated_completion_terminally_settles_active_write_on_state_failure()
-> Result<(), &'static str> {
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
    let Some(write) = controller.take_recovery_io_command() else {
        return Err("fixture must dispatch a canonical recovery write");
    };
    let RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } = &write.operation else {
        return Err("fixture recovery command must be a canonical write");
    };
    let Some(incident_state) = controller.incident.as_mut() else {
        return Err("fixture must retain the persistence incident");
    };
    incident_state.handle.availability = RecoveryHandleAvailability::Available;

    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: write.barrier,
            attempt: write.attempt,
            command_id: RecoveryCommandId(write.command_id.get() + 1),
            result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
                id: request.id,
                applied_through: request.accepted_through,
                new_source: present_revision("unrelated-before-state-failure"),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Shutdown {
            reason: RecoveryShutdownReason::StateFailure(RecoveryStateFailure::HandleStateMismatch),
            ..
        })
    ));
    assert!(!controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.shutdown_complete());
    Ok(())
}

#[test]
fn matching_canonical_post_claim_completion_retains_evidence_before_state_failure()
-> Result<(), &'static str> {
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
    let Some(write) = controller.take_recovery_io_command() else {
        return Err("fixture must dispatch a canonical recovery write");
    };
    let RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } = &write.operation else {
        return Err("fixture recovery command must be a canonical write");
    };
    let artifact = RuntimeStateRecoveryArtifact {
        path: "/tmp/state-failure-post-claim-artifact".into(),
        observation: observation(present_revision("quarantined-source")),
    };
    let effect = RuntimeStatePostClaimPathEffect::QuarantinedAndRetained {
        recovery_path: artifact.path.clone(),
    };
    let Some(incident_state) = controller.incident.as_mut() else {
        return Err("fixture must retain the persistence incident");
    };
    incident_state.handle.availability = RecoveryHandleAvailability::Available;

    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: write.barrier,
            attempt: write.attempt,
            command_id: write.command_id,
            result: RecoveryIoResult::SourceMutation(
                SourceMutationResult::ObservationChangedAfterClaim {
                    id: request.id,
                    active: observation(present_revision("post-claim-active")),
                    recovery_artifacts: vec![artifact.clone()],
                    path_effect: effect.clone(),
                }
            ),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Shutdown { evidence, .. })
            if evidence.recovery_artifacts == vec![artifact]
                && evidence.path_effect_history.last()
                    == Some(&RuntimeStateFailurePathEffect::Known(
                        RuntimeStateObservedPathEffect::PostClaim(effect)
                    ))
    ));
    assert!(!controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.shutdown_complete());
    Ok(())
}

#[test]
fn missing_incident_shutdown_retains_the_sole_source_completion_evidence()
-> Result<(), &'static str> {
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
    let Some(write) = controller.take_recovery_io_command() else {
        return Err("fixture must dispatch a canonical recovery write");
    };
    let RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } = &write.operation else {
        return Err("fixture recovery command must be a canonical write");
    };
    let artifact = RuntimeStateRecoveryArtifact {
        path: "/tmp/missing-incident-recovery-artifact".into(),
        observation: observation(present_revision("missing-incident-artifact")),
    };
    let effect = RuntimeStatePostClaimPathEffect::QuarantinedAndRetained {
        recovery_path: artifact.path.clone(),
    };
    controller.incident = None;

    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: write.barrier,
            attempt: write.attempt,
            command_id: write.command_id,
            result: RecoveryIoResult::SourceMutation(
                SourceMutationResult::ObservationChangedAfterClaim {
                    id: request.id,
                    active: observation(present_revision("missing-incident-active")),
                    recovery_artifacts: vec![artifact.clone()],
                    path_effect: effect.clone(),
                }
            ),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Shutdown {
            reason: RecoveryShutdownReason::StateFailure(RecoveryStateFailure::MissingIncident),
            evidence,
            ..
        }) if evidence.recovery_artifacts == vec![artifact]
            && evidence.path_effect_history == vec![RuntimeStateFailurePathEffect::Known(
                RuntimeStateObservedPathEffect::PostClaim(effect)
            )]
    ));
    assert!(!controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.shutdown_complete());
    Ok(())
}

#[test]
fn missing_incident_malformed_completion_retains_conservative_evidence() -> Result<(), &'static str>
{
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
    let Some(write) = controller.take_recovery_io_command() else {
        return Err("fixture must dispatch a canonical recovery write");
    };
    let RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } = &write.operation else {
        return Err("fixture recovery command must be a canonical write");
    };
    let artifact = RuntimeStateRecoveryArtifact {
        path: "/tmp/malformed-missing-incident-artifact".into(),
        observation: observation(present_revision("malformed-missing-incident-artifact")),
    };
    let effect = RuntimeStatePostClaimPathEffect::QuarantinedAndRetained {
        recovery_path: artifact.path.clone(),
    };
    controller.incident = None;

    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: write.barrier,
            attempt: write.attempt,
            command_id: write.command_id,
            result: RecoveryIoResult::SourceMutation(
                SourceMutationResult::ObservationChangedAfterClaim {
                    id: SourceMutationId(request.id.get() + 1),
                    active: observation(present_revision("malformed-missing-incident-active")),
                    recovery_artifacts: vec![artifact.clone()],
                    path_effect: effect.clone(),
                }
            ),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Shutdown {
            reason: RecoveryShutdownReason::StateFailure(RecoveryStateFailure::MissingIncident),
            evidence,
            ..
        }) if evidence.recovery_artifacts == vec![artifact]
            && evidence.path_effect_history == vec![RuntimeStateFailurePathEffect::Known(
                RuntimeStateObservedPathEffect::PostClaim(effect)
            )]
    ));
    assert!(!controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.shutdown_complete());
    Ok(())
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
fn malformed_result_kind_settles_pending_cancellation_from_sole_completion()
-> Result<(), &'static str> {
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
    let Some(write) = controller.take_recovery_io_command() else {
        return Err("fixture must dispatch a canonical recovery write");
    };
    let RecoveryIoOperation::PersistCanonicalIfUnchanged { .. } = &write.operation else {
        return Err("fixture recovery command must be a canonical write");
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
            result: RecoveryIoResult::Inspected(Ok(inspected(observation(present_revision(
                "malformed-result-kind"
            ))))),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(controller.take_recovery_io_command().is_none());
    assert!(matches!(
        completion.try_recv(),
        Some(PersistenceRecoveryResult::Cancelled { evidence, .. })
            if matches!(
                evidence.path_effect_history.last(),
                Some(RuntimeStateFailurePathEffect::UnknownAfterMutation)
            )
    ));
    assert!(!controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.active_barrier().is_some());
    Ok(())
}

#[test]
fn malformed_active_write_completion_terminalizes_requested_shutdown() -> Result<(), &'static str> {
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
    let Some(write) = controller.take_recovery_io_command() else {
        return Err("fixture must dispatch a canonical recovery write");
    };
    let RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } = &write.operation else {
        return Err("fixture recovery command must be a canonical write");
    };
    if controller.request_shutdown().is_err() {
        return Err("fixture shutdown request must be accepted");
    }

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
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Shutdown {
            reason: RecoveryShutdownReason::Requested,
            ..
        })
    ));
    assert!(controller.take_recovery_io_command().is_none());
    assert!(!controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.active_barrier().is_none());
    assert!(controller.pipeline().shutdown_complete());
    Ok(())
}

#[test]
fn recovery_shutdown_drive_failure_terminally_settles_the_pipeline() -> Result<(), &'static str> {
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
    let Some(write) = controller.take_recovery_io_command() else {
        return Err("fixture must dispatch a canonical recovery write");
    };
    let RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } = &write.operation else {
        return Err("fixture recovery command must be a canonical write");
    };
    let reset_through = controller
        .pipeline
        .allocate_reset_revision()
        .map_err(|_| "fixture must allocate a pending reset receipt")?;
    controller
        .pipeline
        .stage_supported_reset(reset_through, controller.authority_epoch() + 1)
        .map_err(|_| "fixture reset must wait behind the recovery write")?;
    controller.pipeline.exhaust_mutation_id_for_test();
    controller
        .request_shutdown()
        .map_err(|_| "fixture shutdown request must wait for the recovery write")?;

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
                new_source: present_revision("uncertain-during-failed-shutdown"),
                recovery_artifacts: Vec::new(),
            }),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Shutdown {
            reason: RecoveryShutdownReason::StateFailure(RecoveryStateFailure::Pipeline(
                PipelineProtocolError::MutationIdExhausted,
            )),
            ..
        })
    ));
    assert!(matches!(
        controller.receipt(reset_through),
        Some(DurabilityOutcome::Failed(error))
            if error.message().contains("recovery shutdown failed: MutationIdExhausted")
    ));
    assert!(!controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.take_source_mutation().is_none());
    assert!(controller.active_barrier().is_none());
    assert!(controller.shutdown_complete());
    Ok(())
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
fn malformed_active_write_completion_is_consumed_before_reinspection() -> Result<(), &'static str> {
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
    let Some(write) = controller.take_recovery_io_command() else {
        return Err("fixture must dispatch a canonical recovery write");
    };
    let RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } = &write.operation else {
        return Err("fixture recovery command must be a canonical write");
    };
    let mismatch = controller.submit_persistence_recovery_io(RecoveryIoCompletion {
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
    });
    assert!(matches!(
        mismatch,
        SubmitPersistenceRecoveryResult::BlockedProtocolFailure {
            reason: RecoveryCompletionProtocolError::UnexpectedSourceMutationIdentity,
            reinspection_dispatched: Some(_),
            ..
        }
    ));
    assert!(controller.active_barrier().is_some());
    assert!(!controller.pipeline().has_source_mutation_in_flight());
    let Some(reinspection) = controller.take_recovery_io_command() else {
        return Err("sole malformed completion must dispatch reinspection");
    };
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
                "uncertain"
            ))))),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy { .. })
    ));
    Ok(())
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
