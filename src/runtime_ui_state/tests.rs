use std::sync::Arc;

use super::*;
use crate::config::{ToolbarItemOrderGroup, toolbar_item_ids as item_ids};

fn path() -> RuntimeStatePathIdentity {
    RuntimeStatePathIdentity::direct("/tmp/wayscriber-runtime-ui-test.toml")
}

fn missing_revision() -> RuntimeStateSourceRevision {
    RuntimeStateSourceRevision::missing(path())
}

fn present_revision(label: &str) -> RuntimeStateSourceRevision {
    present_revision_at(path(), label)
}

fn present_revision_at(path: RuntimeStatePathIdentity, label: &str) -> RuntimeStateSourceRevision {
    RuntimeStateSourceRevision::present(
        path,
        Arc::<[u8]>::from(label.as_bytes().to_vec().into_boxed_slice()),
    )
}

fn observation(revision: RuntimeStateSourceRevision) -> RuntimeStateSourceObservation {
    RuntimeStateSourceObservation {
        envelope: if revision.bytes().is_some() {
            RuntimeStateObservedEnvelope::Version(1)
        } else {
            RuntimeStateObservedEnvelope::Missing
        },
        revision,
    }
}

fn inspected(observation: RuntimeStateSourceObservation) -> RecoveryInspection {
    let supported_wire = matches!(
        observation.envelope,
        RuntimeStateObservedEnvelope::Version(1)
    )
    .then(RuntimeUiWireState::default);
    RecoveryInspection::new(observation, supported_wire)
}

fn invalid_observation(label: &str) -> RuntimeStateSourceObservation {
    RuntimeStateSourceObservation {
        revision: present_revision(label),
        envelope: RuntimeStateObservedEnvelope::PresentWithoutReadableVersion,
    }
}

fn test_seeds(top_pinned: bool, side_pinned: bool) -> ValidatedInteractionSeeds {
    let mut seeds = ValidatedInteractionSeeds::new();
    seeds
        .insert(
            InteractionSeedTarget::TopPinned,
            InteractionSeedValue::Bool(top_pinned),
        )
        .unwrap();
    seeds
        .insert(
            InteractionSeedTarget::SidePinned,
            InteractionSeedValue::Bool(side_pinned),
        )
        .unwrap();
    seeds
        .insert(
            InteractionSeedTarget::ItemOrder(ToolbarItemOrderGroup::TopTools),
            InteractionSeedValue::ItemOrder(vec![
                item_ids::TOP_TOOL_PEN,
                item_ids::TOP_TOOL_MARKER,
            ]),
        )
        .unwrap();
    seeds
        .insert(
            InteractionSeedTarget::BoardPin("board-6".to_string()),
            InteractionSeedValue::Bool(false),
        )
        .unwrap();
    seeds
        .insert(
            InteractionSeedTarget::TopPosition,
            InteractionSeedValue::Position(ToolbarPositionSeed::new(10.0, 20.0).unwrap()),
        )
        .unwrap();
    seeds
        .insert(
            InteractionSeedTarget::SidePosition,
            InteractionSeedValue::Position(ToolbarPositionSeed::new(30.0, 40.0).unwrap()),
        )
        .unwrap();
    seeds
}

fn controller() -> RuntimeUiStateController {
    RuntimeUiStateController::new(
        test_seeds(false, false),
        missing_revision(),
        RuntimeUiFileStatus::Missing,
    )
}

fn commit_bool(
    controller: &mut RuntimeUiStateController,
    target: InteractionSeedTarget,
    value: bool,
) -> AcceptedStateRevision {
    let permit = controller
        .begin_mutation(RuntimeUiMutationScope::one(target.clone()))
        .unwrap();
    let values = RuntimeUiMutationValues::one(target, InteractionSeedValue::Bool(value)).unwrap();
    match controller.commit(permit, values) {
        CommitResult::Accepted { through } => through,
        result => panic!("expected accepted mutation, got {result:?}"),
    }
}

fn apply_request(
    controller: &mut RuntimeUiStateController,
    request: &SourceMutationRequest,
    revision: RuntimeStateSourceRevision,
) -> SubmitSourceMutationResult {
    controller.submit_source_mutation(SourceMutationResult::Applied {
        id: request.id,
        applied_through: request.accepted_through,
        new_source: revision,
        recovery_artifacts: Vec::new(),
    })
}

fn wire_with_top_pinned(value: bool) -> RuntimeUiWireState {
    let mut source = controller();
    commit_bool(&mut source, InteractionSeedTarget::TopPinned, value);
    let request = source.take_source_mutation().expect("wire source mutation");
    let SourceMutationKind::Replace(wire) = request.kind else {
        panic!("expected replacement wire");
    };
    wire
}

fn fail_current_replace(
    controller: &mut RuntimeUiStateController,
    message: &str,
) -> (AcceptedStateRevision, PersistenceIncidentId) {
    let request = controller.take_source_mutation().expect("replace request");
    let through = request.accepted_through;
    let incident = match controller.submit_source_mutation(SourceMutationResult::Failed {
        id: request.id,
        error: RuntimeStateIoError::new(message),
        active: Some(observation(request.expected_source)),
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    }) {
        SubmitSourceMutationResult::PersistenceUnhealthy { incident, .. } => incident,
        result => panic!("unexpected failure result: {result:?}"),
    };
    (through, incident)
}

fn begin_recovery(
    controller: &mut RuntimeUiStateController,
    incident: PersistenceIncidentId,
    action: PersistenceRecoveryAction,
) -> (RecoveryAttemptClient, RecoveryIoCommand) {
    let handle = match controller.checkout_persistence_recovery_handle(incident) {
        CheckoutPersistenceRecoveryHandleResult::CheckedOut(handle) => handle,
        result => panic!("recovery checkout failed: {result:?}"),
    };
    let client = match controller.begin_persistence_recovery(PersistenceRecoveryRequest {
        recovery: handle,
        action,
    }) {
        BeginPersistenceRecoveryResult::Started { client, .. } => client,
        result => panic!("recovery begin failed: {result:?}"),
    };
    let command = controller
        .take_recovery_io_command()
        .expect("recovery command");
    (client, command)
}

fn begin_confirmed_invalid_preserve() -> (
    RuntimeUiStateController,
    AcceptedStateRevision,
    PersistenceIncidentId,
    RecoveryAttemptClient,
    RecoveryIoCommand,
    SourceMutationId,
) {
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
    let mutation_id = match &preserve.operation {
        RecoveryIoOperation::PreserveInvalidIfUnchanged { mutation_id, .. } => *mutation_id,
        operation => panic!("unexpected operation: {operation:?}"),
    };
    (
        controller,
        through,
        incident,
        confirm_client,
        preserve,
        mutation_id,
    )
}

mod invalid_recovery;
mod permits;
mod pipeline;
mod recovery_basics;
mod recovery_cleanup;
mod recovery_conflicts;
mod recovery_lifecycle;
mod reset_preview;
