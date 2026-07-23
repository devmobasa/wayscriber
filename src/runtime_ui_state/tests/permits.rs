use super::*;

#[test]
fn coalesced_a_to_b_to_a_reload_invalidates_pre_barrier_permit() {
    let mut controller = controller();
    let stale = controller
        .begin_mutation(RuntimeUiMutationScope::one(
            InteractionSeedTarget::TopPinned,
        ))
        .unwrap();
    commit_bool(&mut controller, InteractionSeedTarget::SidePinned, true);
    let (_, incident) = fail_current_replace(&mut controller, "temporary");
    assert!(matches!(
        controller.update_seeds(test_seeds(true, false)),
        UpdateSeedsResult::StagedBehindBarrier { .. }
    ));
    assert!(matches!(
        controller.update_seeds(test_seeds(false, false)),
        UpdateSeedsResult::StagedBehindBarrier {
            replaced_older_staged_reload: true,
            ..
        }
    ));

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
    controller.submit_persistence_recovery_io(RecoveryIoCompletion {
        controller_id: controller.id(),
        incident,
        barrier: write.barrier,
        attempt: write.attempt,
        command_id: write.command_id,
        result: RecoveryIoResult::SourceMutation(SourceMutationResult::Applied {
            id: request.id,
            applied_through: request.accepted_through,
            new_source: present_revision("recovered-after-seed-reversal"),
            recovery_artifacts: Vec::new(),
        }),
    });
    assert!(matches!(
        client.completion.try_recv(),
        Some(PersistenceRecoveryResult::Recovered { .. })
    ));
    assert_eq!(
        controller
            .seeds()
            .state(&InteractionSeedTarget::TopPinned)
            .unwrap()
            .generation(),
        3
    );

    let values = RuntimeUiMutationValues::one(
        InteractionSeedTarget::TopPinned,
        InteractionSeedValue::Bool(true),
    )
    .unwrap();
    assert!(matches!(
        controller.commit(stale, values),
        CommitResult::RejectedSeedChanged { targets }
            if targets == vec![InteractionSeedTarget::TopPinned]
    ));
    assert!(controller.take_source_mutation().is_none());
}

#[test]
fn accepted_mutation_uses_controller_seed_and_exact_source_revision() {
    let mut controller = controller();
    let through = commit_bool(&mut controller, InteractionSeedTarget::TopPinned, true);

    let saved = controller
        .model()
        .get(&InteractionSeedTarget::TopPinned)
        .expect("runtime override");
    assert_eq!(saved.seed, InteractionSeedValue::Bool(false));
    assert_eq!(saved.value, InteractionSeedValue::Bool(true));

    let request = controller.take_source_mutation().expect("source mutation");
    assert_eq!(request.expected_source, missing_revision());
    assert_eq!(request.accepted_through, through);
    assert!(matches!(request.kind, SourceMutationKind::Replace(_)));

    let revision = present_revision("r1");
    assert!(matches!(
        apply_request(&mut controller, &request, revision.clone()),
        SubmitSourceMutationResult::Integrated { .. }
    ));
    assert_eq!(
        controller.receipt(through),
        Some(&DurabilityOutcome::Persisted { source: revision })
    );
}

#[test]
fn relevant_seed_change_rejects_without_revision_or_write() {
    let mut controller = controller();
    let permit = controller
        .begin_mutation(RuntimeUiMutationScope::one(
            InteractionSeedTarget::TopPinned,
        ))
        .unwrap();

    let result = controller.update_seeds(test_seeds(true, false));
    assert!(matches!(result, UpdateSeedsResult::Applied { .. }));
    let accepted_before = controller.pipeline().latest_accepted();
    let result = controller.commit(
        permit,
        RuntimeUiMutationValues::one(
            InteractionSeedTarget::TopPinned,
            InteractionSeedValue::Bool(true),
        )
        .unwrap(),
    );

    assert!(matches!(
        result,
        CommitResult::RejectedSeedChanged { targets }
            if targets == vec![InteractionSeedTarget::TopPinned]
    ));
    assert_eq!(controller.pipeline().latest_accepted(), accepted_before);
    assert!(controller.take_source_mutation().is_none());
}

#[test]
fn seed_reversal_still_rejects_an_old_permit() {
    let mut controller = controller();
    let permit = controller
        .begin_mutation(RuntimeUiMutationScope::one(
            InteractionSeedTarget::TopPinned,
        ))
        .unwrap();
    let initial_generation = controller
        .seeds()
        .state(&InteractionSeedTarget::TopPinned)
        .unwrap()
        .generation();

    controller.update_seeds(test_seeds(true, false));
    controller.update_seeds(test_seeds(false, false));

    assert_eq!(
        controller
            .seeds()
            .state(&InteractionSeedTarget::TopPinned)
            .unwrap()
            .generation(),
        initial_generation + 2
    );
    assert!(matches!(
        controller.commit(
            permit,
            RuntimeUiMutationValues::one(
                InteractionSeedTarget::TopPinned,
                InteractionSeedValue::Bool(true),
            )
            .unwrap(),
        ),
        CommitResult::RejectedSeedChanged { .. }
    ));
}

#[test]
fn unrelated_seed_change_keeps_permit_valid() {
    let mut controller = controller();
    let permit = controller
        .begin_mutation(RuntimeUiMutationScope::one(
            InteractionSeedTarget::TopPinned,
        ))
        .unwrap();

    controller.update_seeds(test_seeds(false, true));

    assert!(matches!(
        controller.commit(
            permit,
            RuntimeUiMutationValues::one(
                InteractionSeedTarget::TopPinned,
                InteractionSeedValue::Bool(true),
            )
            .unwrap(),
        ),
        CommitResult::Accepted { .. }
    ));
}

#[test]
fn atomic_batch_rejects_when_one_guard_changes() {
    let mut controller = controller();
    let permit = controller
        .begin_mutation(RuntimeUiMutationScope::batch([
            InteractionSeedTarget::TopPinned,
            InteractionSeedTarget::SidePinned,
        ]))
        .unwrap();
    controller.update_seeds(test_seeds(false, true));

    let result = controller.commit(
        permit,
        RuntimeUiMutationValues::batch([
            (
                InteractionSeedTarget::TopPinned,
                InteractionSeedValue::Bool(true),
            ),
            (
                InteractionSeedTarget::SidePinned,
                InteractionSeedValue::Bool(false),
            ),
        ])
        .unwrap(),
    );

    assert!(matches!(result, CommitResult::RejectedSeedChanged { .. }));
    assert!(controller.model().is_empty());
    assert!(controller.take_source_mutation().is_none());
}

#[test]
fn removed_and_reused_board_target_rejects_old_permit() {
    let mut controller = controller();
    let target = InteractionSeedTarget::BoardPin("board-6".to_string());
    let permit = controller
        .begin_mutation(RuntimeUiMutationScope::one(target.clone()))
        .unwrap();
    let old_generation = controller.seeds().state(&target).unwrap().generation();

    let mut removed = test_seeds(false, false);
    removed.remove(&target);
    controller.update_seeds(removed);
    controller.update_seeds(test_seeds(false, false));

    assert_eq!(
        controller.seeds().state(&target).unwrap().generation(),
        old_generation + 2
    );
    assert!(matches!(
        controller.commit(
            permit,
            RuntimeUiMutationValues::one(target, InteractionSeedValue::Bool(true)).unwrap(),
        ),
        CommitResult::RejectedSeedChanged { .. }
    ));
}

#[test]
fn config_position_permit_is_rejected_only_by_relevant_seed_reload() {
    let mut controller = controller();
    let top = controller
        .begin_config_interaction(ConfigPositionTarget::Top)
        .unwrap();
    controller.update_seeds(test_seeds(false, true));
    assert_eq!(
        controller.validate_config_interaction(top),
        ValidateConfigInteractionResult::Accepted(ConfigPositionTarget::Top)
    );

    let side = controller
        .begin_config_interaction(ConfigPositionTarget::Side)
        .unwrap();
    let mut changed = test_seeds(false, true);
    changed
        .insert(
            InteractionSeedTarget::SidePosition,
            InteractionSeedValue::Position(ToolbarPositionSeed::new(31.0, 41.0).unwrap()),
        )
        .unwrap();
    controller.update_seeds(changed);
    assert_eq!(
        controller.validate_config_interaction(side),
        ValidateConfigInteractionResult::RejectedSeedChanged
    );
}
