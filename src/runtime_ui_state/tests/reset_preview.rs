use super::*;

#[test]
fn supported_reset_rejects_a_nonmissing_acknowledgement_without_partial_publish() {
    let mut controller = controller();
    let original_epoch = controller.authority_epoch();
    let reset_through = match controller.request_supported_reset() {
        RequestResetResult::Started { through, .. } => through,
        result => panic!("reset did not start: {result:?}"),
    };
    let reset = controller.take_source_mutation().expect("reset request");
    assert!(matches!(
        controller.submit_source_mutation(SourceMutationResult::Applied {
            id: reset.id,
            applied_through: reset.accepted_through,
            new_source: present_revision("not-missing"),
            recovery_artifacts: Vec::new(),
        }),
        SubmitSourceMutationResult::Rejected(
            PipelineProtocolError::ResetDidNotProduceMissingSource
        )
    ));
    assert_eq!(controller.authority_epoch(), original_epoch);
    assert_eq!(controller.pipeline().stable_source(), &missing_revision());
    assert_eq!(controller.receipt(reset_through), None);
    assert!(controller.active_barrier().is_some());

    assert!(matches!(
        controller.submit_source_mutation(SourceMutationResult::Applied {
            id: reset.id,
            applied_through: reset.accepted_through,
            new_source: missing_revision(),
            recovery_artifacts: Vec::new(),
        }),
        SubmitSourceMutationResult::ResetCompleted { .. }
    ));
    assert_eq!(controller.authority_epoch(), original_epoch + 1);
}

#[test]
fn supported_reset_waits_for_in_flight_write_and_publishes_epoch_on_ack() {
    let mut controller = controller();
    let old_permit = controller
        .begin_mutation(RuntimeUiMutationScope::one(
            InteractionSeedTarget::TopPinned,
        ))
        .unwrap();
    let first = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request_a = controller.take_source_mutation().unwrap();
    let held = commit_bool(&mut controller, InteractionSeedTarget::SidePinned, true);
    let old_epoch = controller.authority_epoch();
    let (barrier, reset_through, publish_epoch) = match controller.request_supported_reset() {
        RequestResetResult::Started {
            barrier,
            through,
            publish_epoch,
        } => (barrier, through, publish_epoch),
        result => panic!("reset did not start: {result:?}"),
    };
    assert_eq!(controller.authority_epoch(), old_epoch);
    assert!(matches!(
        controller.begin_mutation(RuntimeUiMutationScope::one(
            InteractionSeedTarget::TopPinned
        )),
        Err(BeginMutationError::ControllerBusy(id)) if id == barrier
    ));

    let revision_a = present_revision("before-reset");
    apply_request(&mut controller, &request_a, revision_a.clone());
    let reset = controller.take_source_mutation().expect("reset request");
    assert_eq!(reset.expected_source, revision_a);
    assert!(matches!(
        reset.kind,
        SourceMutationKind::ResetSupported { publish_epoch: epoch } if epoch == publish_epoch
    ));
    let missing_after_reset = RuntimeStateSourceRevision::missing(path());
    assert_eq!(
        apply_request(&mut controller, &reset, missing_after_reset.clone()),
        SubmitSourceMutationResult::ResetCompleted {
            barrier,
            published_epoch: publish_epoch,
            recovery_artifacts: Vec::new(),
        }
    );

    assert_eq!(controller.authority_epoch(), publish_epoch);
    assert!(controller.model().is_empty());
    assert!(controller.active_barrier().is_none());
    assert!(matches!(
        controller.receipt(first),
        Some(DurabilityOutcome::Persisted { .. })
    ));
    assert_eq!(
        controller.receipt(held),
        Some(&DurabilityOutcome::SupersededByReset { reset_through })
    );
    assert_eq!(
        controller.receipt(reset_through),
        Some(&DurabilityOutcome::Persisted {
            source: missing_after_reset
        })
    );
    assert!(matches!(
        controller.commit(
            old_permit,
            RuntimeUiMutationValues::one(
                InteractionSeedTarget::TopPinned,
                InteractionSeedValue::Bool(true)
            )
            .unwrap()
        ),
        CommitResult::RejectedStaleAuthorityEpoch
    ));
}

#[test]
fn unsupported_mode_keeps_runtime_preview_live_only_but_position_config_persistent() {
    let mut controller = RuntimeUiStateController::new(
        test_seeds(false, false),
        present_revision("version-2"),
        RuntimeUiFileStatus::UnsupportedReadOnly { version: Some(2) },
    );
    let target = InteractionSeedTarget::TopPinned;
    let session = controller
        .begin_runtime_preview(
            RuntimeUiMutationScope::one(target.clone()),
            PreviewRollbackSnapshot::default(),
        )
        .unwrap();
    assert!(matches!(session, RuntimeUiPreviewSession::LiveOnly(_)));
    let result = controller.finish_preview(
        PreviewFinishRequest::RuntimeUi {
            session,
            intent: RuntimePreviewFinishIntent::Commit(
                RuntimeUiMutationValues::one(target.clone(), InteractionSeedValue::Bool(true))
                    .unwrap(),
            ),
        },
        |_, _| unreachable!("runtime preview must not invoke config writer"),
    );
    assert_eq!(result, PreviewFinishResult::AppliedLiveOnly);
    assert_eq!(
        controller.live_state().get(&target),
        Some(&InteractionSeedValue::Bool(true))
    );
    assert!(controller.model().is_empty());
    assert_eq!(
        controller.pipeline().latest_accepted(),
        AcceptedStateRevision(0)
    );
    assert!(controller.take_source_mutation().is_none());

    let position = controller
        .begin_config_position_preview(
            ConfigPositionTarget::Top,
            PreviewRollbackSnapshot::default(),
        )
        .unwrap();
    let mut applied = None;
    let result = controller.finish_preview(
        PreviewFinishRequest::ConfigPosition {
            session: position,
            intent: ConfigPositionFinishIntent::Commit(
                ToolbarPositionSeed::new(50.0, 60.0).unwrap(),
            ),
        },
        |target, value| {
            applied = Some((target, value));
            Ok(())
        },
    );
    assert_eq!(
        result,
        PreviewFinishResult::AppliedConfig {
            target: ConfigPositionTarget::Top
        }
    );
    assert_eq!(
        applied,
        Some((
            ConfigPositionTarget::Top,
            ToolbarPositionSeed::new(50.0, 60.0).unwrap()
        ))
    );
    assert!(controller.take_source_mutation().is_none());
}

#[test]
fn unsupported_reset_request_returns_a_revision_bound_confirmation_without_side_effects() {
    let source = present_revision("version-2");
    let mut controller = RuntimeUiStateController::new(
        test_seeds(false, false),
        source.clone(),
        RuntimeUiFileStatus::UnsupportedReadOnly { version: Some(2) },
    );
    let epoch = controller.authority_epoch();

    let confirmation = match controller.request_runtime_ui_reset() {
        RequestResetResult::RequiresUnsupportedConfirmation {
            observed_version,
            confirmation,
        } => {
            assert_eq!(observed_version, Some(2));
            confirmation
        }
        result => panic!("unsupported reset did not request confirmation: {result:?}"),
    };

    assert_eq!(controller.authority_epoch(), epoch);
    assert_eq!(controller.pipeline().stable_source(), &source);
    assert!(controller.active_barrier().is_none());
    assert!(controller.take_source_mutation().is_none());
    drop(confirmation);

    assert!(matches!(
        controller.request_runtime_ui_reset(),
        RequestResetResult::RequiresUnsupportedConfirmation {
            observed_version: Some(2),
            ..
        }
    ));
}

#[test]
fn cancelled_unsupported_reset_confirmation_is_single_use_and_has_no_side_effects() {
    let source = present_revision("version-2");
    let mut controller = RuntimeUiStateController::new(
        test_seeds(false, false),
        source.clone(),
        RuntimeUiFileStatus::UnsupportedReadOnly { version: Some(2) },
    );
    let epoch = controller.authority_epoch();
    let confirmation = match controller.request_runtime_ui_reset() {
        RequestResetResult::RequiresUnsupportedConfirmation { confirmation, .. } => confirmation,
        result => panic!("unsupported reset did not request confirmation: {result:?}"),
    };
    let replay = confirmation.clone();

    assert_eq!(
        controller.cancel_unsupported_reset_confirmation(confirmation),
        CancelUnsupportedResetConfirmationResult::Cancelled
    );
    assert_eq!(
        controller.confirm_unsupported_reset(replay),
        ConfirmUnsupportedResetResult::RejectedToken
    );
    assert_eq!(controller.authority_epoch(), epoch);
    assert_eq!(controller.pipeline().stable_source(), &source);
    assert!(controller.active_barrier().is_none());
    assert!(controller.take_source_mutation().is_none());
}

#[test]
fn confirmed_unsupported_reset_uses_the_exact_source_and_publishes_only_after_ack() {
    let source = present_revision("version-2");
    let mut controller = RuntimeUiStateController::new(
        test_seeds(false, false),
        source.clone(),
        RuntimeUiFileStatus::UnsupportedReadOnly { version: Some(2) },
    );
    let original_epoch = controller.authority_epoch();
    let confirmation = match controller.request_runtime_ui_reset() {
        RequestResetResult::RequiresUnsupportedConfirmation { confirmation, .. } => confirmation,
        result => panic!("unsupported reset did not request confirmation: {result:?}"),
    };
    let (barrier, through, publish_epoch) = match controller.confirm_unsupported_reset(confirmation)
    {
        ConfirmUnsupportedResetResult::Started {
            barrier,
            through,
            publish_epoch,
        } => (barrier, through, publish_epoch),
        result => panic!("unsupported reset confirmation was rejected: {result:?}"),
    };

    assert_eq!(publish_epoch, original_epoch + 1);
    assert_eq!(controller.authority_epoch(), original_epoch);
    assert!(matches!(
        controller.active_barrier(),
        Some(ActiveControllerBarrier {
            id,
            operation: ControllerBarrierOperation::ConfirmUnsupportedReset,
            ..
        }) if *id == barrier
    ));
    let reset = controller
        .take_source_mutation()
        .expect("unsupported reset");
    assert_eq!(reset.expected_source, source.clone());
    assert!(matches!(
        reset.kind,
        SourceMutationKind::ResetUnsupportedIfUnchanged {
            publish_epoch: epoch,
            confirmation_revision,
        } if epoch == publish_epoch && confirmation_revision == source
    ));

    let artifact = RuntimeStateRecoveryArtifact {
        path: "/tmp/wayscriber-unsupported-preserved".into(),
        observation: RuntimeStateSourceObservation {
            revision: source,
            envelope: RuntimeStateObservedEnvelope::Version(2),
        },
    };
    let missing = missing_revision();
    assert_eq!(
        controller.submit_source_mutation(SourceMutationResult::Applied {
            id: reset.id,
            applied_through: reset.accepted_through,
            new_source: missing.clone(),
            recovery_artifacts: vec![artifact.clone()],
        }),
        SubmitSourceMutationResult::ResetCompleted {
            barrier,
            published_epoch: publish_epoch,
            recovery_artifacts: vec![artifact],
        }
    );
    assert_eq!(
        controller.receipt(through),
        Some(&DurabilityOutcome::Persisted { source: missing })
    );
    assert_eq!(controller.authority_epoch(), publish_epoch);
    assert!(controller.active_barrier().is_none());
}

#[test]
fn unsupported_reset_conflict_retains_live_only_authority_for_unsupported_source() {
    let mut controller = RuntimeUiStateController::new(
        test_seeds(false, false),
        present_revision("version-2"),
        RuntimeUiFileStatus::UnsupportedReadOnly { version: Some(2) },
    );
    let top = InteractionSeedTarget::TopPinned;
    let live_only = controller
        .begin_runtime_preview(
            RuntimeUiMutationScope::one(top.clone()),
            PreviewRollbackSnapshot::default(),
        )
        .unwrap();
    assert_eq!(
        controller.finish_preview(
            PreviewFinishRequest::RuntimeUi {
                session: live_only,
                intent: RuntimePreviewFinishIntent::Commit(
                    RuntimeUiMutationValues::one(top.clone(), InteractionSeedValue::Bool(true),)
                        .unwrap(),
                ),
            },
            |_, _| unreachable!(),
        ),
        PreviewFinishResult::AppliedLiveOnly
    );
    let untouched = controller
        .begin_runtime_preview(
            RuntimeUiMutationScope::one(InteractionSeedTarget::SidePinned),
            PreviewRollbackSnapshot::default(),
        )
        .unwrap();
    let original_epoch = controller.authority_epoch();
    let confirmation = match controller.request_runtime_ui_reset() {
        RequestResetResult::RequiresUnsupportedConfirmation { confirmation, .. } => confirmation,
        result => panic!("unsupported reset did not request confirmation: {result:?}"),
    };
    let through = match controller.confirm_unsupported_reset(confirmation) {
        ConfirmUnsupportedResetResult::Started { through, .. } => through,
        result => panic!("unsupported reset confirmation was rejected: {result:?}"),
    };
    let reset = controller.take_source_mutation().unwrap();
    let changed_revision = present_revision("version-3");
    let changed = RuntimeStateSourceObservation {
        revision: changed_revision.clone(),
        envelope: RuntimeStateObservedEnvelope::Version(3),
    };
    let barrier =
        match controller.submit_source_mutation(SourceMutationResult::SourceChangedBeforeMutation {
            id: reset.id,
            active: changed.clone(),
        }) {
            SubmitSourceMutationResult::ExternalReconciliationRequired { barrier, .. } => barrier,
            result => panic!("unexpected reset conflict result: {result:?}"),
        };

    controller
        .install_external_authority(
            barrier,
            changed,
            RuntimeUiFileStatus::UnsupportedReadOnly { version: Some(3) },
            RuntimeUiModel::default(),
            WirePassthrough::default(),
        )
        .unwrap();

    assert_eq!(controller.authority_epoch(), original_epoch);
    assert_eq!(controller.pipeline().stable_source(), &changed_revision);
    assert_eq!(
        controller.live_state().get(&top),
        Some(&InteractionSeedValue::Bool(true))
    );
    assert_eq!(
        controller.receipt(through),
        Some(&DurabilityOutcome::ExternalSourceWon)
    );
    assert_eq!(
        controller.finish_preview(
            PreviewFinishRequest::RuntimeUi {
                session: untouched,
                intent: RuntimePreviewFinishIntent::Commit(
                    RuntimeUiMutationValues::one(
                        InteractionSeedTarget::SidePinned,
                        InteractionSeedValue::Bool(true),
                    )
                    .unwrap(),
                ),
            },
            |_, _| unreachable!(),
        ),
        PreviewFinishResult::AppliedLiveOnly
    );
    assert!(controller.take_preview_resolutions().is_empty());
}

#[test]
fn shutdown_rejects_seed_updates_and_preexisting_preview_commits() {
    let mut controller = RuntimeUiStateController::new(
        test_seeds(false, false),
        present_revision("version-2"),
        RuntimeUiFileStatus::UnsupportedReadOnly { version: Some(2) },
    );
    let runtime = controller
        .begin_runtime_preview(
            RuntimeUiMutationScope::one(InteractionSeedTarget::TopPinned),
            PreviewRollbackSnapshot::default(),
        )
        .unwrap();
    let config = controller
        .begin_config_position_preview(
            ConfigPositionTarget::Top,
            PreviewRollbackSnapshot::default(),
        )
        .unwrap();
    let direct_config = controller
        .begin_config_interaction(ConfigPositionTarget::Side)
        .unwrap();

    controller.request_shutdown().unwrap();

    assert_eq!(
        controller.validate_config_interaction(direct_config),
        ValidateConfigInteractionResult::RejectedShuttingDown
    );
    assert!(matches!(
        controller.finish_preview(
            PreviewFinishRequest::RuntimeUi {
                session: runtime,
                intent: RuntimePreviewFinishIntent::Commit(
                    RuntimeUiMutationValues::one(
                        InteractionSeedTarget::TopPinned,
                        InteractionSeedValue::Bool(true),
                    )
                    .unwrap(),
                ),
            },
            |_, _| unreachable!("runtime preview must not invoke config writer"),
        ),
        PreviewFinishResult::RejectedStaleAuthority { .. }
    ));
    assert!(matches!(
        controller.finish_preview(
            PreviewFinishRequest::ConfigPosition {
                session: config,
                intent: ConfigPositionFinishIntent::Commit(
                    ToolbarPositionSeed::new(50.0, 60.0).unwrap(),
                ),
            },
            |_, _| unreachable!("shutdown must reject config mutation"),
        ),
        PreviewFinishResult::RejectedStaleAuthority { .. }
    ));
    assert!(matches!(
        controller.update_seeds(test_seeds(true, true)),
        UpdateSeedsResult::RejectedShuttingDown
    ));
    assert_eq!(
        controller
            .live_state()
            .get(&InteractionSeedTarget::TopPinned),
        Some(&InteractionSeedValue::Bool(false))
    );
}

#[test]
fn stale_runtime_and_config_preview_cancellations_do_not_restore_rollbacks() {
    let mut controller = controller();
    let runtime = controller
        .begin_runtime_preview(
            RuntimeUiMutationScope::one(InteractionSeedTarget::TopPinned),
            PreviewRollbackSnapshot::default(),
        )
        .unwrap();
    let config = controller
        .begin_config_position_preview(
            ConfigPositionTarget::Top,
            PreviewRollbackSnapshot::default(),
        )
        .unwrap();
    let mut changed = test_seeds(true, false);
    changed
        .insert(
            InteractionSeedTarget::TopPosition,
            InteractionSeedValue::Position(ToolbarPositionSeed::new(50.0, 60.0).unwrap()),
        )
        .unwrap();
    assert!(matches!(
        controller.update_seeds(changed),
        UpdateSeedsResult::Applied { .. }
    ));

    assert!(matches!(
        controller.finish_preview(
            PreviewFinishRequest::RuntimeUi {
                session: runtime,
                intent: RuntimePreviewFinishIntent::Cancel,
            },
            |_, _| unreachable!(),
        ),
        PreviewFinishResult::RejectedStaleAuthority { .. }
    ));
    assert!(matches!(
        controller.finish_preview(
            PreviewFinishRequest::ConfigPosition {
                session: config,
                intent: ConfigPositionFinishIntent::Cancel,
            },
            |_, _| unreachable!(),
        ),
        PreviewFinishResult::RejectedStaleAuthority { .. }
    ));
}

#[test]
fn preview_release_during_failed_reset_is_resolved_once_without_replay() {
    let mut controller = controller();
    let preview = controller
        .begin_runtime_preview(
            RuntimeUiMutationScope::one(InteractionSeedTarget::TopPinned),
            PreviewRollbackSnapshot::default(),
        )
        .unwrap();
    let barrier = match controller.request_supported_reset() {
        RequestResetResult::Started { barrier, .. } => barrier,
        result => panic!("reset failed to start: {result:?}"),
    };
    assert!(matches!(
        controller.finish_preview(
            PreviewFinishRequest::RuntimeUi {
                session: preview,
                intent: RuntimePreviewFinishIntent::Commit(
                    RuntimeUiMutationValues::one(
                        InteractionSeedTarget::TopPinned,
                        InteractionSeedValue::Bool(true),
                    )
                    .unwrap(),
                ),
            },
            |_, _| unreachable!(),
        ),
        PreviewFinishResult::AbandonedDuringBarrier { barrier: active } if active == barrier
    ));
    let reset = controller.take_source_mutation().unwrap();
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: reset.id,
        error: RuntimeStateIoError::new("reset failed"),
        active: Some(observation(missing_revision())),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy {
            barrier: same,
            incident,
            ..
        } if same == barrier => incident,
        result => panic!("unexpected failure: {result:?}"),
    };
    let resolutions = controller.take_preview_resolutions();
    assert_eq!(resolutions.len(), 1);
    assert_eq!(
        resolutions[0].reason,
        AbandonedPreviewResolutionReason::CancelledUnderRetainedAuthority
    );
    assert!(controller.active_barrier().is_some());
    let (client, inspection) = begin_recovery(
        &mut controller,
        incident,
        PersistenceRecoveryAction::RetryPending,
    );
    assert!(matches!(
        controller.submit_persistence_recovery_io(RecoveryIoCompletion {
            controller_id: controller.id(),
            incident,
            barrier,
            attempt: inspection.attempt,
            command_id: inspection.command_id,
            result: RecoveryIoResult::Inspected(Ok(inspected(observation(missing_revision())))),
        }),
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Recovered { .. })
    ));
    assert!(controller.take_preview_resolutions().is_empty());
    assert!(controller.model().is_empty());
}

#[test]
fn retained_authority_failure_resolves_previews_before_a_later_staged_reload() {
    let mut controller = controller();
    let mut top_rollback = PreviewRollbackSnapshot::default();
    top_rollback.values.insert(
        InteractionSeedTarget::TopPinned,
        InteractionSeedValue::Bool(false),
    );
    let mut side_rollback = PreviewRollbackSnapshot::default();
    side_rollback.values.insert(
        InteractionSeedTarget::SidePinned,
        InteractionSeedValue::Bool(false),
    );
    let top = controller
        .begin_runtime_preview(
            RuntimeUiMutationScope::one(InteractionSeedTarget::TopPinned),
            top_rollback,
        )
        .unwrap();
    let side = controller
        .begin_runtime_preview(
            RuntimeUiMutationScope::one(InteractionSeedTarget::SidePinned),
            side_rollback,
        )
        .unwrap();
    let barrier = match controller.request_supported_reset() {
        RequestResetResult::Started { barrier, .. } => barrier,
        result => panic!("reset failed to start: {result:?}"),
    };
    for session in [top, side] {
        assert_eq!(
            controller.finish_preview(
                PreviewFinishRequest::RuntimeUi {
                    session,
                    intent: RuntimePreviewFinishIntent::Cancel,
                },
                |_, _| unreachable!(),
            ),
            PreviewFinishResult::AbandonedDuringBarrier { barrier }
        );
    }
    let reset = controller.take_source_mutation().unwrap();
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: reset.id,
        error: RuntimeStateIoError::new("reset failed"),
        active: Some(observation(missing_revision())),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected failure: {result:?}"),
    };
    let resolutions = controller.take_preview_resolutions();
    assert_eq!(resolutions.len(), 2);
    assert!(resolutions.iter().all(|resolution| {
        resolution.reason == AbandonedPreviewResolutionReason::CancelledUnderRetainedAuthority
    }));
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
        SubmitPersistenceRecoveryResult::Terminal { .. }
    ));
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Recovered { .. })
    ));
    assert!(controller.take_preview_resolutions().is_empty());
}
