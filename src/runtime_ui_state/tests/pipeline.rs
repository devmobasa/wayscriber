use super::*;

#[test]
fn contradictory_post_claim_restoration_is_rejected_without_settling() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();
    let active = observation(present_revision("active-after-claim"));
    let result =
        controller.submit_source_mutation(SourceMutationResult::ObservationChangedAfterClaim {
            id: request.id,
            active,
            recovery_artifacts: Vec::new(),
            path_effect: RuntimeStatePostClaimPathEffect::QuarantinedThenRestored {
                restored_source: present_revision("different-restored-source"),
            },
        });
    assert!(matches!(
        result,
        SubmitSourceMutationResult::Rejected(
            PipelineProtocolError::ContradictoryPostClaimObservation
        )
    ));
    assert!(controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.receipt(through).is_none());
    assert!(controller.active_barrier().is_none());
}

#[test]
fn restored_managed_path_cannot_be_reported_as_a_recovery_artifact() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();
    let active = observation(present_revision("active-after-restore"));
    let managed_path = active.revision.path_identity().source_path().to_path_buf();
    let result =
        controller.submit_source_mutation(SourceMutationResult::ObservationChangedAfterClaim {
            id: request.id,
            active: active.clone(),
            recovery_artifacts: vec![RuntimeStateRecoveryArtifact {
                path: managed_path,
                observation: active.clone(),
            }],
            path_effect: RuntimeStatePostClaimPathEffect::QuarantinedThenRestored {
                restored_source: active.revision,
            },
        });
    assert!(matches!(
        result,
        SubmitSourceMutationResult::Rejected(
            PipelineProtocolError::ContradictoryPostClaimObservation
        )
    ));
    assert!(controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.receipt(through).is_none());
}

#[test]
fn inconsistent_conflict_observation_is_rejected_without_settling() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();
    let inconsistent = RuntimeStateSourceObservation {
        revision: missing_revision(),
        envelope: RuntimeStateObservedEnvelope::Version(1),
    };
    let result =
        controller.submit_source_mutation(SourceMutationResult::SourceChangedBeforeMutation {
            id: request.id,
            active: inconsistent,
        });
    assert!(matches!(
        result,
        SubmitSourceMutationResult::Rejected(PipelineProtocolError::InconsistentSourceObservation)
    ));
    assert!(controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.receipt(through).is_none());
}

#[test]
fn conflict_matching_the_expected_source_is_rejected_without_settling() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();
    let result =
        controller.submit_source_mutation(SourceMutationResult::SourceChangedBeforeMutation {
            id: request.id,
            active: observation(request.expected_source),
        });
    assert!(matches!(
        result,
        SubmitSourceMutationResult::Rejected(PipelineProtocolError::ConflictMatchedExpectedSource)
    ));
    assert!(controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.receipt(through).is_none());
}

#[test]
fn applied_source_from_another_path_is_rejected_without_settling() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();
    let result = controller.submit_source_mutation(SourceMutationResult::Applied {
        id: request.id,
        applied_through: request.accepted_through,
        new_source: present_revision_at(
            RuntimeStatePathIdentity::direct("/tmp/another-runtime-ui-state.toml"),
            "persisted",
        ),
        recovery_artifacts: Vec::new(),
    });

    assert!(matches!(
        result,
        SubmitSourceMutationResult::Rejected(PipelineProtocolError::AppliedSourcePathMismatch)
    ));
    assert!(controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.receipt(through).is_none());
    assert_eq!(controller.pipeline().stable_source(), &missing_revision());
}

#[test]
fn retained_post_claim_path_must_name_a_reported_artifact() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();
    let result =
        controller.submit_source_mutation(SourceMutationResult::ObservationChangedAfterClaim {
            id: request.id,
            active: observation(missing_revision()),
            recovery_artifacts: Vec::new(),
            path_effect: RuntimeStatePostClaimPathEffect::QuarantinedAndRetained {
                recovery_path: "/tmp/unreported-recovery".into(),
            },
        });
    assert!(matches!(
        result,
        SubmitSourceMutationResult::Rejected(
            PipelineProtocolError::RetainedRecoveryArtifactMissing
        )
    ));
    assert!(controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.receipt(through).is_none());
}

#[test]
fn known_untouched_failure_cannot_report_recovery_artifacts() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();
    let result = controller.submit_source_mutation(SourceMutationResult::Failed {
        id: request.id,
        error: RuntimeStateIoError::new("pre-mutation failure"),
        active: Some(observation(missing_revision())),
        recovery_artifacts: vec![RuntimeStateRecoveryArtifact {
            path: "/tmp/unexpected-artifact".into(),
            observation: observation(present_revision("artifact")),
        }],
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    });
    assert!(matches!(
        result,
        SubmitSourceMutationResult::Rejected(
            PipelineProtocolError::UntouchedResultReportedRecoveryArtifacts
        )
    ));
    assert!(controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.receipt(through).is_none());
}

#[test]
fn rapid_non_coalesced_writes_chain_acknowledged_revisions() {
    let mut controller = controller();
    let first = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request_a = controller.take_source_mutation().expect("first request");
    let second = commit_bool(&mut controller, InteractionSeedTarget::SidePinned, true);
    assert!(controller.take_source_mutation().is_none());

    let revision_a = present_revision("r1");
    apply_request(&mut controller, &request_a, revision_a.clone());
    let request_b = controller.take_source_mutation().expect("second request");
    assert_eq!(request_b.expected_source, revision_a);
    assert_eq!(request_b.accepted_through, second);

    let revision_b = present_revision("r2");
    apply_request(&mut controller, &request_b, revision_b.clone());
    assert!(matches!(
        controller.receipt(first),
        Some(DurabilityOutcome::Persisted { .. })
    ));
    assert_eq!(
        controller.receipt(second),
        Some(&DurabilityOutcome::Persisted { source: revision_b })
    );
}

#[test]
fn staged_replacements_coalesce_but_flush_waits_for_the_latest_snapshot() {
    let mut controller = controller();
    let first = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request_a = controller.take_source_mutation().unwrap();
    let second = commit_bool(&mut controller, InteractionSeedTarget::SidePinned, true);
    let third = commit_bool(&mut controller, InteractionSeedTarget::SidePinned, false);
    let flush = controller.request_flush(third).unwrap();
    assert!(controller.flush_outcome(flush).is_none());

    apply_request(&mut controller, &request_a, present_revision("r1"));
    let request_b = controller.take_source_mutation().unwrap();
    assert_eq!(request_b.accepted_through, third);
    assert!(controller.flush_outcome(flush).is_none());
    apply_request(&mut controller, &request_b, present_revision("r2"));

    assert!(matches!(
        controller.receipt(first),
        Some(DurabilityOutcome::Persisted { .. })
    ));
    assert!(matches!(
        controller.receipt(second),
        Some(DurabilityOutcome::Persisted { .. })
    ));
    assert!(matches!(
        controller.receipt(third),
        Some(DurabilityOutcome::Persisted { .. })
    ));
    assert!(matches!(
        controller.flush_outcome(flush),
        Some(FlushOutcome::Settled { .. })
    ));
}

#[test]
fn completed_receipts_and_flushes_can_be_consumed_without_losing_flush_history() {
    let mut controller = controller();
    let first = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let first_request = controller.take_source_mutation().unwrap();
    let first_source = present_revision("first-persisted");
    apply_request(&mut controller, &first_request, first_source.clone());

    assert_eq!(
        controller.take_receipt(first),
        Some(DurabilityOutcome::Persisted {
            source: first_source.clone(),
        })
    );
    assert!(controller.receipt(first).is_none());
    let durable_flush = controller.request_flush(first).unwrap();
    assert_eq!(
        controller.take_flush_outcome(durable_flush),
        Some(FlushOutcome::Settled {
            source: first_source,
        })
    );
    assert!(controller.flush_outcome(durable_flush).is_none());

    let second = commit_bool(&mut controller, InteractionSeedTarget::SidePinned, true);
    let second_request = controller.take_source_mutation().unwrap();
    let barrier =
        match controller.submit_source_mutation(SourceMutationResult::SourceChangedBeforeMutation {
            id: second_request.id,
            active: observation(present_revision("external-source")),
        }) {
            SubmitSourceMutationResult::ExternalReconciliationRequired { barrier, .. } => barrier,
            result => panic!("unexpected conflict result: {result:?}"),
        };
    controller
        .install_external_authority(
            barrier,
            observation(present_revision("installed-external-source")),
            RuntimeUiFileStatus::Supported,
            RuntimeUiModel::default(),
            WirePassthrough::default(),
        )
        .unwrap();
    assert_eq!(
        controller.take_receipt(second),
        Some(DurabilityOutcome::ExternalSourceWon)
    );
    assert!(controller.receipt(second).is_none());

    let failed_flush = controller.request_flush(second).unwrap();
    assert_eq!(
        controller.take_flush_outcome(failed_flush),
        Some(FlushOutcome::Failed)
    );
    assert!(controller.flush_outcome(failed_flush).is_none());
}

#[test]
fn flush_is_rejected_while_external_authority_reconciliation_is_active() {
    let mut controller = controller();
    let persisted = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let persisted_request = controller.take_source_mutation().unwrap();
    apply_request(
        &mut controller,
        &persisted_request,
        present_revision("persisted-before-conflict"),
    );
    commit_bool(&mut controller, InteractionSeedTarget::SidePinned, true);
    let conflicting_request = controller.take_source_mutation().unwrap();
    let barrier =
        match controller.submit_source_mutation(SourceMutationResult::SourceChangedBeforeMutation {
            id: conflicting_request.id,
            active: observation(present_revision("external-conflict")),
        }) {
            SubmitSourceMutationResult::ExternalReconciliationRequired { barrier, .. } => barrier,
            result => panic!("unexpected conflict result: {result:?}"),
        };

    assert_eq!(
        controller.request_flush(persisted),
        Err(PipelineProtocolError::ControllerBarrierActive { barrier })
    );

    let installed = present_revision("installed-external");
    controller
        .install_external_authority(
            barrier,
            observation(installed.clone()),
            RuntimeUiFileStatus::Supported,
            RuntimeUiModel::default(),
            WirePassthrough::default(),
        )
        .unwrap();
    let flush = controller.request_flush(persisted).unwrap();
    assert_eq!(flush, FlushRequestId(1));
    assert_eq!(
        controller.flush_outcome(flush),
        Some(&FlushOutcome::Settled { source: installed })
    );
}

#[test]
fn duplicate_or_stale_source_ack_does_not_advance_stable_revision() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();
    let wrong = SourceMutationResult::Applied {
        id: SourceMutationId(request.id.get() + 1),
        applied_through: request.accepted_through,
        new_source: present_revision("wrong"),
        recovery_artifacts: Vec::new(),
    };
    assert!(matches!(
        controller.submit_source_mutation(wrong),
        SubmitSourceMutationResult::Rejected(PipelineProtocolError::WrongMutationId { .. })
    ));
    assert_eq!(controller.pipeline().stable_source(), &missing_revision());
    assert!(matches!(
        apply_request(&mut controller, &request, present_revision("right")),
        SubmitSourceMutationResult::Integrated { .. }
    ));
    assert_eq!(
        controller.pipeline().stable_source(),
        &present_revision("right")
    );
    assert!(matches!(
        apply_request(&mut controller, &request, present_revision("duplicate")),
        SubmitSourceMutationResult::Rejected(PipelineProtocolError::NoMutationInFlight)
    ));
    assert_eq!(
        controller.pipeline().stable_source(),
        &present_revision("right")
    );
}

#[test]
fn source_ack_before_dispatch_and_completion_without_recovery_are_rejected() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller
        .pipeline()
        .source_mutation_in_flight()
        .expect("in-flight request")
        .clone();
    assert!(matches!(
        apply_request(&mut controller, &request, present_revision("too-early")),
        SubmitSourceMutationResult::Rejected(PipelineProtocolError::MutationNotDispatched)
    ));
    assert_eq!(controller.receipt(through), None);
    assert!(controller.take_source_mutation().is_some());

    let completion = RecoveryIoCompletion {
        controller_id: controller.id(),
        incident: PersistenceIncidentId(99),
        barrier: ControllerBarrierId(99),
        attempt: RecoveryAttemptId(99),
        command_id: RecoveryCommandId(99),
        result: RecoveryIoResult::Inspected(Err(RuntimeStateInspectionError::new("late"))),
    };
    assert!(matches!(
        controller.submit_persistence_recovery_io(completion),
        SubmitPersistenceRecoveryResult::RejectedNoActiveRecovery { .. }
    ));
}

#[test]
fn replacement_acknowledgement_requires_a_present_source() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();

    assert!(matches!(
        controller.submit_source_mutation(SourceMutationResult::Applied {
            id: request.id,
            applied_through: request.accepted_through,
            new_source: missing_revision(),
            recovery_artifacts: Vec::new(),
        }),
        SubmitSourceMutationResult::Rejected(
            PipelineProtocolError::ReplaceDidNotProducePresentSource
        )
    ));
    assert!(controller.pipeline().has_source_mutation_in_flight());
    assert!(controller.receipt(through).is_none());
    assert_eq!(controller.pipeline().stable_source(), &missing_revision());
}

#[test]
fn successful_replacement_publishes_supported_file_status() {
    let mut controller = controller();
    commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);
    let request = controller.take_source_mutation().unwrap();

    apply_request(&mut controller, &request, present_revision("r1"));

    assert!(matches!(
        controller.file_status,
        RuntimeUiFileStatus::Supported
    ));
}
