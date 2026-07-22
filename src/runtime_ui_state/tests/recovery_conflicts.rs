use super::*;

#[test]
fn external_conflict_discards_old_snapshots_and_chains_from_installed_authority() {
    let mut controller = controller();
    let first = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();
    let second = commit_bool(&mut controller, InteractionSeedTarget::SidePinned, true);
    let external = present_revision("external");
    let result =
        controller.submit_source_mutation(SourceMutationResult::SourceChangedBeforeMutation {
            id: request.id,
            active: observation(external.clone()),
        });
    let barrier = match result {
        SubmitSourceMutationResult::ExternalReconciliationRequired {
            barrier,
            recovery_artifacts,
            path_effect: RuntimeStateObservedPathEffect::Untouched,
            ..
        } => {
            assert!(recovery_artifacts.is_empty());
            barrier
        }
        result => panic!("unexpected conflict result: {result:?}"),
    };
    assert_eq!(
        controller.receipt(first),
        Some(&DurabilityOutcome::ExternalSourceWon)
    );
    assert_eq!(
        controller.receipt(second),
        Some(&DurabilityOutcome::ExternalSourceWon)
    );
    assert!(controller.take_source_mutation().is_none());

    let installed = controller
        .install_external_authority(
            barrier,
            observation(external.clone()),
            RuntimeUiFileStatus::Supported,
            RuntimeUiModel::default(),
            WirePassthrough::default(),
        )
        .unwrap();
    assert_eq!(installed.cleanup_through, None);
    assert_eq!(
        installed.evidence.path_effect,
        RuntimeStateObservedPathEffect::Untouched
    );
    assert!(controller.active_barrier().is_none());
    assert!(controller.model().is_empty());

    let third = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let next = controller
        .take_source_mutation()
        .expect("post-conflict write");
    assert_eq!(next.accepted_through, third);
    assert_eq!(next.expected_source, external);
    assert_eq!(barrier.get(), 1);
}

#[test]
fn external_authority_install_validates_barrier_and_observed_status() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();
    let authority = observation(present_revision("external"));
    let barrier =
        match controller.submit_source_mutation(SourceMutationResult::SourceChangedBeforeMutation {
            id: request.id,
            active: authority.clone(),
        }) {
            SubmitSourceMutationResult::ExternalReconciliationRequired { barrier, .. } => barrier,
            result => panic!("unexpected conflict result: {result:?}"),
        };

    assert_eq!(
        controller.install_external_authority(
            ControllerBarrierId(barrier.get() + 1),
            authority.clone(),
            RuntimeUiFileStatus::Supported,
            RuntimeUiModel::default(),
            WirePassthrough::default(),
        ),
        Err(ExternalAuthorityInstallError::WrongBarrier)
    );
    assert_eq!(controller.authority_epoch(), 1);
    assert_eq!(controller.pipeline().stable_source(), &missing_revision());
    assert_eq!(
        controller.install_external_authority(
            barrier,
            authority.clone(),
            RuntimeUiFileStatus::Missing,
            RuntimeUiModel::default(),
            WirePassthrough::default(),
        ),
        Err(ExternalAuthorityInstallError::FileStatusMismatch)
    );
    assert_eq!(
        controller.install_external_authority(
            barrier,
            observation(missing_revision()),
            RuntimeUiFileStatus::Missing,
            wire_with_top_pinned(true).model,
            WirePassthrough::default(),
        ),
        Err(ExternalAuthorityInstallError::UnexpectedDecodedAuthority)
    );
    assert!(controller.active_barrier().is_some());

    controller
        .install_external_authority(
            barrier,
            authority.clone(),
            RuntimeUiFileStatus::Supported,
            RuntimeUiModel::default(),
            WirePassthrough::default(),
        )
        .unwrap();
    assert_eq!(controller.pipeline().stable_source(), &authority.revision);
    assert_eq!(controller.authority_epoch(), 2);
}

#[test]
fn source_conflict_after_shutdown_settles_without_stranding_a_barrier() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();
    controller.request_shutdown().unwrap();
    let active = observation(present_revision("external-during-shutdown"));
    assert!(matches!(
        controller.submit_source_mutation(SourceMutationResult::SourceChangedBeforeMutation {
            id: request.id,
            active: active.clone(),
        }),
        SubmitSourceMutationResult::ExternalReconciliationSettledForShutdown {
            active: returned,
            ..
        } if returned == active
    ));
    assert_eq!(
        controller.receipt(through),
        Some(&DurabilityOutcome::ExternalSourceWon)
    );
    assert!(controller.active_barrier().is_none());
    assert!(controller.pipeline().shutdown_complete());
}

#[test]
fn failed_source_mutation_after_shutdown_settles_without_recovery() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();
    controller.request_shutdown().unwrap();

    let _ = controller.submit_source_mutation(SourceMutationResult::Failed {
        id: request.id,
        error: RuntimeStateIoError::new("failed during shutdown"),
        active: Some(observation(request.expected_source)),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    });

    assert!(matches!(
        controller.receipt(through),
        Some(DurabilityOutcome::Failed(error))
            if error.message() == "failed during shutdown"
    ));
    assert!(controller.incident.is_none());
    assert!(controller.active_barrier().is_none());
    assert!(controller.pipeline().shutdown_complete());
}

#[test]
fn malformed_external_authority_enters_recoverable_invalid_state() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();
    let invalid = invalid_observation("malformed-external");
    let barrier =
        match controller.submit_source_mutation(SourceMutationResult::SourceChangedBeforeMutation {
            id: request.id,
            active: invalid.clone(),
        }) {
            SubmitSourceMutationResult::ExternalReconciliationRequired { barrier, .. } => barrier,
            result => panic!("unexpected conflict result: {result:?}"),
        };

    assert!(matches!(
        controller.install_external_authority(
            barrier,
            invalid.clone(),
            RuntimeUiFileStatus::Invalid,
            RuntimeUiModel::default(),
            WirePassthrough::default(),
        ),
        Err(ExternalAuthorityInstallError::InvalidAuthority { .. })
    ));
    let incident = controller
        .incident
        .as_ref()
        .expect("invalid authority must create a recovery incident")
        .id;
    assert_eq!(controller.pipeline().stable_source(), &invalid.revision);
    assert!(matches!(
        controller.active_barrier(),
        Some(ActiveControllerBarrier {
            phase: ControllerBarrierPhase::PersistenceUnhealthy {
                incident: active,
            },
            ..
        }) if *active == incident
    ));
    assert!(matches!(
        controller.checkout_persistence_recovery_handle(incident),
        CheckoutPersistenceRecoveryHandleResult::CheckedOut(_)
    ));
}

#[test]
fn recovery_write_conflict_reinspects_before_installing_decoded_authority() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let (_, incident) = fail_current_replace(&mut controller, "temporary");
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
    let request = match &write.operation {
        RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => request.clone(),
        operation => panic!("unexpected operation: {operation:?}"),
    };
    let writer_observation = observation(present_revision("writer-observation"));
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
                }
            ),
        }),
        SubmitPersistenceRecoveryResult::Continue { .. }
    ));
    assert!(matches!(
        controller.receipt(through),
        Some(DurabilityOutcome::Failed(error)) if error.message() == "temporary"
    ));
    assert_eq!(controller.pipeline().stable_source(), &missing_revision());

    let reinspection = controller
        .take_recovery_io_command()
        .expect("external authority reinspection");
    assert!(matches!(
        reinspection.operation,
        RecoveryIoOperation::Inspect
    ));
    let authority = observation(present_revision("fresh-authority"));
    let authority_wire = wire_with_top_pinned(true);
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: reinspection.barrier,
            attempt: reinspection.attempt,
            command_id: reinspection.command_id,
            result: RecoveryIoResult::Inspected(Ok(RecoveryInspection::new(
                authority.clone(),
                Some(authority_wire),
            ))),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::ExternalAuthorityInstalled {
            writer_observation: Some(writer),
            authority: installed,
            path_effect: RuntimeStateObservedPathEffect::Untouched,
            ..
        }) if writer == writer_observation && installed == authority
    ));
    assert_eq!(controller.pipeline().stable_source(), &authority.revision);
    assert_eq!(
        controller
            .live_state()
            .get(&InteractionSeedTarget::TopPinned),
        Some(&InteractionSeedValue::Bool(true))
    );
    assert!(controller.active_barrier().is_none());
}

#[test]
fn invalid_reinspection_after_recovery_conflict_keeps_authority_blocked() {
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
                active: observation(present_revision("writer-observation")),
            },
        ),
    });
    let reinspection = controller
        .take_recovery_io_command()
        .expect("external authority reinspection");
    let invalid = invalid_observation("malformed-external");
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier: reinspection.barrier,
            attempt: reinspection.attempt,
            command_id: reinspection.command_id,
            result: RecoveryIoResult::Inspected(Ok(inspected(invalid.clone()))),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::StillUnhealthy {
            active: Some(active),
            ..
        }) if active == invalid
    ));
    assert_eq!(controller.pipeline().stable_source(), &missing_revision());
    assert_eq!(controller.authority_epoch(), 1);
    assert!(controller.active_barrier().is_some());
}

#[test]
fn post_claim_conflict_preserves_typed_path_effect_and_artifacts() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();
    let active = observation(present_revision("new-active"));
    let artifact = RuntimeStateRecoveryArtifact {
        path: "/tmp/claimed-copy".into(),
        observation: observation(present_revision("claimed")),
    };
    let restored = active.revision.clone();
    let effect = RuntimeStatePostClaimPathEffect::QuarantinedThenRestored {
        restored_source: restored,
    };
    let barrier = match controller.submit_source_mutation(
        SourceMutationResult::ObservationChangedAfterClaim {
            id: request.id,
            active: active.clone(),
            recovery_artifacts: vec![artifact.clone()],
            path_effect: effect.clone(),
        },
    ) {
        SubmitSourceMutationResult::ExternalReconciliationRequired {
            barrier,
            recovery_artifacts,
            path_effect: RuntimeStateObservedPathEffect::PostClaim(returned),
            ..
        } => {
            assert_eq!(recovery_artifacts, vec![artifact.clone()]);
            assert_eq!(returned, effect);
            barrier
        }
        result => panic!("unexpected result: {result:?}"),
    };
    assert!(matches!(
        controller.receipt(through),
        Some(DurabilityOutcome::ObservationChangedAfterClaim {
            recovery_artifacts,
            ..
        }) if recovery_artifacts == &vec![artifact.clone()]
    ));
    let installed = controller
        .install_external_authority(
            barrier,
            active,
            RuntimeUiFileStatus::Supported,
            RuntimeUiModel::default(),
            WirePassthrough::default(),
        )
        .unwrap();
    assert_eq!(
        installed.evidence.path_effect,
        RuntimeStateObservedPathEffect::PostClaim(effect)
    );
    assert_eq!(installed.evidence.recovery_artifacts, vec![artifact]);
    assert_eq!(barrier.get(), 1);
}

#[test]
fn reload_during_external_cleanup_is_applied_before_reconciliation_closes() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let write = controller.take_source_mutation().unwrap();
    let barrier =
        match controller.submit_source_mutation(SourceMutationResult::SourceChangedBeforeMutation {
            id: write.id,
            active: observation(present_revision("external")),
        }) {
            SubmitSourceMutationResult::ExternalReconciliationRequired { barrier, .. } => barrier,
            result => panic!("unexpected result: {result:?}"),
        };
    assert!(matches!(
        controller.update_seeds(test_seeds(true, false)),
        UpdateSeedsResult::StagedBehindBarrier { .. }
    ));
    let RuntimeUiWireState { model, passthrough } = wire_with_top_pinned(true);
    let installed = controller
        .install_external_authority(
            barrier,
            observation(present_revision("fresh")),
            RuntimeUiFileStatus::Supported,
            model,
            passthrough,
        )
        .unwrap();
    assert!(installed.cleanup_through.is_some());
    let cleanup = controller.take_source_mutation().unwrap();
    assert!(matches!(
        controller.update_seeds(test_seeds(false, false)),
        UpdateSeedsResult::StagedBehindBarrier { .. }
    ));

    apply_request(&mut controller, &cleanup, present_revision("cleaned"));

    assert!(controller.active_barrier().is_none());
    assert_eq!(
        controller
            .seeds()
            .current_value(&InteractionSeedTarget::TopPinned),
        Some(&InteractionSeedValue::Bool(false))
    );
    assert!(controller.staged_reload.is_none());
}

#[test]
fn shutdown_during_external_cleanup_closes_after_a_late_reload() {
    let mut wire_source = controller();
    commit_bool(&mut wire_source, InteractionSeedTarget::TopPinned, true);
    let first = wire_source.take_source_mutation().unwrap();
    apply_request(&mut wire_source, &first, present_revision("wire-1"));
    commit_bool(&mut wire_source, InteractionSeedTarget::SidePinned, true);
    let second = wire_source.take_source_mutation().unwrap();
    let SourceMutationKind::Replace(external_wire) = second.kind else {
        unreachable!();
    };

    let mut controller = controller();
    let preview = controller
        .begin_runtime_preview(
            RuntimeUiMutationScope::one(InteractionSeedTarget::TopPinned),
            PreviewRollbackSnapshot::default(),
        )
        .unwrap();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let write = controller.take_source_mutation().unwrap();
    let barrier =
        match controller.submit_source_mutation(SourceMutationResult::SourceChangedBeforeMutation {
            id: write.id,
            active: observation(present_revision("external")),
        }) {
            SubmitSourceMutationResult::ExternalReconciliationRequired { barrier, .. } => barrier,
            result => panic!("unexpected conflict result: {result:?}"),
        };
    assert_eq!(
        controller.finish_preview(
            PreviewFinishRequest::RuntimeUi {
                session: preview,
                intent: RuntimePreviewFinishIntent::Cancel,
            },
            |_, _| unreachable!(),
        ),
        PreviewFinishResult::AbandonedDuringBarrier { barrier }
    );
    controller.update_seeds(test_seeds(true, false));
    let RuntimeUiWireState { model, passthrough } = external_wire;
    controller
        .install_external_authority(
            barrier,
            observation(present_revision("fresh")),
            RuntimeUiFileStatus::Supported,
            model,
            passthrough,
        )
        .unwrap();
    let cleanup = controller.take_source_mutation().unwrap();
    controller.update_seeds(test_seeds(true, true));
    controller.request_shutdown().unwrap();

    apply_request(&mut controller, &cleanup, present_revision("cleaned"));

    assert!(controller.pipeline().shutdown_complete());
    assert!(controller.active_barrier().is_none());
    assert!(controller.staged_reload.is_none());
    assert_eq!(
        controller.take_preview_resolutions()[0].reason,
        AbandonedPreviewResolutionReason::DiscardedForAuthorityChange
    );
}

#[test]
fn invalid_external_authority_discards_pre_change_preview_rollback() {
    let persisted_wire = wire_with_top_pinned(true);
    let mut controller = RuntimeUiStateController::new_with_authority(
        test_seeds(false, false),
        present_revision("r1"),
        RuntimeUiFileStatus::Supported,
        persisted_wire,
    );
    let mut rollback = PreviewRollbackSnapshot::default();
    rollback.values.insert(
        InteractionSeedTarget::TopPinned,
        InteractionSeedValue::Bool(true),
    );
    let preview = controller
        .begin_runtime_preview(
            RuntimeUiMutationScope::one(InteractionSeedTarget::TopPinned),
            rollback,
        )
        .unwrap();
    commit_bool(&mut controller, InteractionSeedTarget::SidePinned, true);
    let write = controller.take_source_mutation().unwrap();
    let invalid = invalid_observation("malformed-external");
    let barrier =
        match controller.submit_source_mutation(SourceMutationResult::SourceChangedBeforeMutation {
            id: write.id,
            active: invalid.clone(),
        }) {
            SubmitSourceMutationResult::ExternalReconciliationRequired { barrier, .. } => barrier,
            result => panic!("unexpected conflict result: {result:?}"),
        };
    assert_eq!(
        controller.finish_preview(
            PreviewFinishRequest::RuntimeUi {
                session: preview,
                intent: RuntimePreviewFinishIntent::Cancel,
            },
            |_, _| unreachable!(),
        ),
        PreviewFinishResult::AbandonedDuringBarrier { barrier }
    );
    assert!(matches!(
        controller.install_external_authority(
            barrier,
            invalid,
            RuntimeUiFileStatus::Invalid,
            RuntimeUiModel::default(),
            WirePassthrough::default(),
        ),
        Err(ExternalAuthorityInstallError::InvalidAuthority { .. })
    ));

    assert_eq!(
        controller.take_preview_resolutions()[0].reason,
        AbandonedPreviewResolutionReason::DiscardedForAuthorityChange
    );
}
