use std::collections::BTreeSet;
use std::sync::mpsc::{Receiver, Sender};

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PersistenceAuthoritySnapshot {
    pub(crate) expected_source: RuntimeStateSourceRevision,
    pub(crate) file_status: RuntimeUiFileStatus,
    pub(crate) authority_epoch: u64,
    pub(crate) model: RuntimeUiModel,
    pub(crate) passthrough: WirePassthrough,
    pub(crate) seeds: InteractionSeedRegistry,
    pub(crate) live_state: RuntimeUiLiveState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecoveryHandleAvailability {
    Available,
    CheckedOut(RecoveryLeaseNonce),
    InAttempt(RecoveryAttemptId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryHandleState {
    pub(crate) id: RecoveryHandleId,
    pub(crate) availability: RecoveryHandleAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecoveryCleanupState {
    NeedsRecompute,
    Clean,
    Pending {
        through: AcceptedStateRevision,
    },
    InFlight {
        through: AcceptedStateRevision,
        command: RecoveryCommandId,
        recompute_after_ack: bool,
    },
}

impl RecoveryCleanupState {
    pub(crate) fn mark_recompute(&mut self) {
        match self {
            Self::InFlight {
                recompute_after_ack,
                ..
            } => *recompute_after_ack = true,
            Self::Pending { .. } => {}
            _ => *self = Self::NeedsRecompute,
        }
    }
}

#[derive(Debug)]
pub(crate) struct PersistenceIncident {
    pub(crate) id: PersistenceIncidentId,
    pub(crate) barrier: ControllerBarrierId,
    pub(crate) retained_authority: PersistenceAuthoritySnapshot,
    pub(crate) held_replacements: Vec<HeldReplacementStage>,
    pub(crate) retry_desired_through: Option<AcceptedStateRevision>,
    pub(crate) staged_reload: Option<StagedSeedReload>,
    pub(crate) applied_reload_changed_targets: BTreeSet<InteractionSeedTarget>,
    pub(crate) last_safe_active: Option<RuntimeStateSourceObservation>,
    pub(crate) cleanup: RecoveryCleanupState,
    pub(crate) recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
    pub(crate) path_effect_history: Vec<RuntimeStateFailurePathEffect>,
    pub(crate) handle: RecoveryHandleState,
}

#[derive(Debug)]
pub(crate) struct PersistenceRecoveryHandle {
    controller_id: ControllerId,
    incident: PersistenceIncidentId,
    barrier: ControllerBarrierId,
    handle_id: RecoveryHandleId,
    lease: RecoveryLeaseNonce,
    lifecycle: Sender<LifecycleControl>,
    armed: bool,
}

impl PersistenceRecoveryHandle {
    pub(crate) fn incident(&self) -> PersistenceIncidentId {
        self.incident
    }

    pub(crate) fn handle_id(&self) -> RecoveryHandleId {
        self.handle_id
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PersistenceRecoveryHandle {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.lifecycle.send(LifecycleControl::ReturnRecoveryHandle {
                controller: self.controller_id,
                incident: self.incident,
                barrier: self.barrier,
                handle: self.handle_id,
                lease: self.lease,
            });
        }
    }
}

#[derive(Debug)]
pub(crate) enum LifecycleControl {
    ReturnRecoveryHandle {
        controller: ControllerId,
        incident: PersistenceIncidentId,
        barrier: ControllerBarrierId,
        handle: RecoveryHandleId,
        lease: RecoveryLeaseNonce,
    },
    CancelAttempt {
        controller: ControllerId,
        incident: PersistenceIncidentId,
        barrier: ControllerBarrierId,
        attempt: RecoveryAttemptId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InvalidStateResetConfirmation {
    controller: ControllerId,
    incident: PersistenceIncidentId,
    barrier: ControllerBarrierId,
    handle: RecoveryHandleId,
    revision: RuntimeStateSourceRevision,
    envelope: RuntimeStateObservedEnvelope,
}

#[derive(Debug)]
pub(crate) enum PersistenceRecoveryAction {
    RetryPending,
    DiscardPendingAndAdoptObserved,
    RequestPreserveInvalidReset,
    ConfirmPreserveInvalidReset(InvalidStateResetConfirmation),
}

#[derive(Debug)]
pub(crate) struct PersistenceRecoveryRequest {
    pub(crate) recovery: PersistenceRecoveryHandle,
    pub(crate) action: PersistenceRecoveryAction,
}

#[derive(Debug)]
pub(crate) struct RecoveryCancellation {
    controller_id: ControllerId,
    incident: PersistenceIncidentId,
    barrier: ControllerBarrierId,
    attempt: RecoveryAttemptId,
    lifecycle: Sender<LifecycleControl>,
    armed: bool,
}

impl RecoveryCancellation {
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for RecoveryCancellation {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.lifecycle.send(LifecycleControl::CancelAttempt {
                controller: self.controller_id,
                incident: self.incident,
                barrier: self.barrier,
                attempt: self.attempt,
            });
        }
    }
}

#[derive(Debug)]
pub(crate) struct RecoveryCompletionReceiver {
    receiver: Receiver<PersistenceRecoveryResult>,
}

impl RecoveryCompletionReceiver {
    pub(crate) fn try_recv(&self) -> Option<PersistenceRecoveryResult> {
        self.receiver.try_recv().ok()
    }
}

#[derive(Debug)]
pub(crate) struct RecoveryAttemptClient {
    pub(crate) cancellation: RecoveryCancellation,
    pub(crate) completion: RecoveryCompletionReceiver,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecoveryCommandExpectation {
    Inspection,
    SourceMutation {
        mutation_id: SourceMutationId,
        kind: RecoverySourceMutationKind,
        accepted_through: Option<AcceptedStateRevision>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecoverySourceMutationKind {
    PersistCanonical {
        purpose: RecoveryCanonicalWritePurpose,
    },
    PreserveInvalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecoveryCanonicalWritePurpose {
    pub(crate) retry_desired_through: Option<AcceptedStateRevision>,
    pub(crate) cleanup_through: Option<AcceptedStateRevision>,
}

impl RecoveryCanonicalWritePurpose {
    pub(crate) fn is_valid(self) -> bool {
        self.retry_desired_through.is_some() || self.cleanup_through.is_some()
    }
}

#[derive(Debug)]
pub(crate) enum RecoveryAttemptKind {
    RetryPending,
    DiscardPendingAndAdoptObserved,
    RequestPreserveInvalidReset,
    ConfirmPreserveInvalidReset {
        confirmation: InvalidStateResetConfirmation,
    },
    ConfirmPreserveInvalidResetInFlight {
        confirmation: InvalidStateResetConfirmation,
    },
    ReinspectExternalAuthority {
        writer_observation: Option<RuntimeStateSourceObservation>,
        path_effect: RuntimeStateObservedPathEffect,
        preserve_invalid_confirmed: Option<RuntimeStateSourceRevision>,
    },
    ExternalAuthorityCleanup {
        writer_observation: Option<RuntimeStateSourceObservation>,
        authority: RuntimeStateSourceObservation,
        path_effect: RuntimeStateObservedPathEffect,
    },
    ProtocolFailureReinspection,
}

#[derive(Debug)]
pub(crate) struct ActiveRecoveryCommand {
    pub(crate) id: RecoveryCommandId,
    pub(crate) expected: RecoveryCommandExpectation,
}

#[derive(Debug)]
pub(crate) struct ActiveRecoveryAttempt {
    pub(crate) id: RecoveryAttemptId,
    pub(crate) incident: PersistenceIncidentId,
    pub(crate) barrier: ControllerBarrierId,
    pub(crate) kind: RecoveryAttemptKind,
    pub(crate) current_command: ActiveRecoveryCommand,
    pub(crate) protocol_failure_pending: bool,
    pub(crate) cancel_requested: bool,
    pub(crate) completion: Sender<PersistenceRecoveryResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryIoCommand {
    pub(crate) controller_id: ControllerId,
    pub(crate) incident: PersistenceIncidentId,
    pub(crate) barrier: ControllerBarrierId,
    pub(crate) attempt: RecoveryAttemptId,
    pub(crate) command_id: RecoveryCommandId,
    pub(crate) operation: RecoveryIoOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecoveryIoOperation {
    Inspect,
    PreserveInvalidIfUnchanged {
        mutation_id: SourceMutationId,
        confirmation: InvalidStateResetConfirmation,
    },
    PersistCanonicalIfUnchanged {
        request: SourceMutationRequest,
        purpose: RecoveryCanonicalWritePurpose,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryInspection {
    pub(crate) observation: RuntimeStateSourceObservation,
    pub(crate) supported_wire: Option<RuntimeUiWireState>,
}

impl RecoveryInspection {
    pub(crate) fn new(
        observation: RuntimeStateSourceObservation,
        supported_wire: Option<RuntimeUiWireState>,
    ) -> Self {
        Self {
            observation,
            supported_wire,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecoveryIoResult {
    Inspected(Result<RecoveryInspection, RuntimeStateInspectionError>),
    SourceMutation(SourceMutationResult),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoveryIoCompletion {
    pub(crate) controller_id: ControllerId,
    pub(crate) incident: PersistenceIncidentId,
    pub(crate) barrier: ControllerBarrierId,
    pub(crate) attempt: RecoveryAttemptId,
    pub(crate) command_id: RecoveryCommandId,
    pub(crate) result: RecoveryIoResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecoveryBeginRejection {
    WrongController,
    StaleHandle,
    NotUnhealthy,
    AttemptAlreadyRunning,
    InvalidActionOrConfirmation,
    ShuttingDown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecoveryCompletionProtocolError {
    WrongIncidentOrBarrier,
    UnknownAttempt,
    UnknownCommand,
    UnexpectedResultKind,
    UnexpectedSourceMutationIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecoveryCancellationRejection {
    WrongIncidentOrBarrier,
    UnknownOrCompletedAttempt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PersistenceRecoveryEvidence {
    pub(crate) recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
    pub(crate) path_effect_history: Vec<RuntimeStateFailurePathEffect>,
}

#[derive(Debug)]
pub(crate) enum PersistenceRecoveryResult {
    Recovered {
        incident: PersistenceIncidentId,
        final_source: RuntimeStateSourceObservation,
        settled_through: AcceptedStateRevision,
        evidence: PersistenceRecoveryEvidence,
    },
    ExternalAuthorityInstalled {
        incident: PersistenceIncidentId,
        writer_observation: Option<RuntimeStateSourceObservation>,
        authority: RuntimeStateSourceObservation,
        evidence: PersistenceRecoveryEvidence,
        path_effect: RuntimeStateObservedPathEffect,
    },
    InvalidSourcePreservedAndReset {
        incident: PersistenceIncidentId,
        new_source: RuntimeStateSourceObservation,
        evidence: PersistenceRecoveryEvidence,
        path_effect: RuntimeStatePostClaimPathEffect,
    },
    RequiresInvalidResetConfirmation {
        recovery: PersistenceRecoveryHandle,
        observed: RuntimeStateSourceObservation,
        confirmation: InvalidStateResetConfirmation,
        evidence: PersistenceRecoveryEvidence,
    },
    ObservationChanged {
        recovery: PersistenceRecoveryHandle,
        confirmed: RuntimeStateSourceObservation,
        active: RuntimeStateSourceObservation,
        evidence: PersistenceRecoveryEvidence,
        path_effect: RuntimeStateObservedPathEffect,
    },
    StillUnhealthy {
        recovery: PersistenceRecoveryHandle,
        attempt: RecoveryAttemptId,
        error: RuntimeStateIoError,
        active: Option<RuntimeStateSourceObservation>,
        evidence: PersistenceRecoveryEvidence,
    },
    Cancelled {
        recovery: PersistenceRecoveryHandle,
        attempt: RecoveryAttemptId,
        active: Option<RuntimeStateSourceObservation>,
        evidence: PersistenceRecoveryEvidence,
    },
    Shutdown {
        incident: PersistenceIncidentId,
        evidence: PersistenceRecoveryEvidence,
    },
}

#[derive(Debug)]
pub(crate) enum BeginPersistenceRecoveryResult {
    Started {
        client: RecoveryAttemptClient,
        dispatched: RecoveryCommandId,
    },
    Rejected {
        request: PersistenceRecoveryRequest,
        reason: RecoveryBeginRejection,
    },
}

#[derive(Debug)]
pub(crate) enum CheckoutPersistenceRecoveryHandleResult {
    CheckedOut(PersistenceRecoveryHandle),
    AlreadyCheckedOut,
    RejectedWrongControllerOrIncident,
    RejectedNotUnhealthy,
}

#[derive(Debug)]
pub(crate) enum SubmitPersistenceRecoveryResult {
    Continue {
        dispatched: RecoveryCommandId,
    },
    Terminal {
        attempt: RecoveryAttemptId,
    },
    RerouteWrongController {
        completion: RecoveryIoCompletion,
    },
    IgnoredCancelledReadOnly {
        command_id: RecoveryCommandId,
    },
    IgnoredDuplicateAlreadyIntegrated {
        command_id: RecoveryCommandId,
    },
    RejectedNoActiveRecovery {
        completion: RecoveryIoCompletion,
    },
    BlockedProtocolFailure {
        reason: RecoveryCompletionProtocolError,
        evidence: PersistenceRecoveryEvidence,
        reinspection_dispatched: Option<RecoveryCommandId>,
    },
}

#[derive(Debug)]
pub(crate) enum CancelPersistenceRecoveryResult {
    Cancelled,
    PendingIrrevocableIo {
        attempt: RecoveryAttemptId,
        command_id: RecoveryCommandId,
    },
    RerouteWrongController {
        cancellation: RecoveryCancellation,
    },
    RejectedInert {
        reason: RecoveryCancellationRejection,
    },
}

mod attempt;
mod authority;
mod lifecycle;
mod preserve;
mod protocol;

use protocol::{
    allocate_counter, completion_protocol_error, evidence, merge_artifacts,
    observation_matches_file_status, reinspection_writer_observation,
    source_mutation_observation_for_protocol_error,
};
