use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::{
    AcceptedStateRevision, ControllerBarrierId, FlushRequestId, RuntimeStateFailurePathEffect,
    RuntimeStateIoError, RuntimeStateObservedPathEffect, RuntimeStatePostClaimPathEffect,
    RuntimeStateRecoveryArtifact, RuntimeStateSourceObservation, RuntimeStateSourceRevision,
    RuntimeUiWireState, SourceMutationId,
};

mod integration;
mod queue;
mod staging;

pub(crate) use integration::validate_source_mutation_evidence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceMutationRequest {
    pub(crate) id: SourceMutationId,
    pub(crate) accepted_through: AcceptedStateRevision,
    pub(crate) expected_source: RuntimeStateSourceRevision,
    pub(crate) expected_epoch: u64,
    pub(crate) kind: SourceMutationKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceMutationKind {
    Replace(RuntimeUiWireState),
    ResetSupported {
        publish_epoch: u64,
    },
    ResetUnsupportedIfUnchanged {
        publish_epoch: u64,
        confirmation_revision: RuntimeStateSourceRevision,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceMutationResult {
    Applied {
        id: SourceMutationId,
        applied_through: AcceptedStateRevision,
        new_source: RuntimeStateSourceRevision,
        recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
    },
    SourceChangedBeforeMutation {
        id: SourceMutationId,
        active: RuntimeStateSourceObservation,
    },
    ObservationChangedAfterClaim {
        id: SourceMutationId,
        active: RuntimeStateSourceObservation,
        recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
        path_effect: RuntimeStatePostClaimPathEffect,
    },
    Failed {
        id: SourceMutationId,
        error: RuntimeStateIoError,
        active: Option<RuntimeStateSourceObservation>,
        recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
        path_effect: RuntimeStateFailurePathEffect,
    },
}

impl SourceMutationResult {
    pub(crate) fn id(&self) -> SourceMutationId {
        match self {
            Self::Applied { id, .. }
            | Self::SourceChangedBeforeMutation { id, .. }
            | Self::ObservationChangedAfterClaim { id, .. }
            | Self::Failed { id, .. } => *id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DurabilityOutcome {
    Persisted {
        source: RuntimeStateSourceRevision,
    },
    SupersededByReset {
        reset_through: AcceptedStateRevision,
    },
    ExternalSourceWon,
    ObservationChangedAfterClaim {
        active: RuntimeStateSourceObservation,
        recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
        path_effect: RuntimeStatePostClaimPathEffect,
    },
    Failed(RuntimeStateIoError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FlushOutcome {
    Settled { source: RuntimeStateSourceRevision },
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PipelineProtocolError {
    NoMutationInFlight,
    MutationNotDispatched,
    MutationAlreadyInFlight,
    WrongMutationId {
        expected: SourceMutationId,
        received: SourceMutationId,
    },
    WrongAppliedRevision {
        expected: AcceptedStateRevision,
        received: AcceptedStateRevision,
    },
    AppliedSourcePathMismatch,
    InconsistentSourceObservation,
    ConflictMatchedExpectedSource,
    DuplicateRecoveryArtifactPath,
    RetainedRecoveryArtifactMissing,
    UntouchedResultReportedRecoveryArtifacts,
    ContradictoryPostClaimObservation,
    ReplaceDidNotProducePresentSource,
    ResetDidNotProduceMissingSource,
    InvalidPublishEpoch,
    RevisionExhausted,
    MutationIdExhausted,
    FlushIdExhausted,
    FlushBeyondAccepted {
        requested: AcceptedStateRevision,
        latest: AcceptedStateRevision,
    },
    ControllerBarrierActive {
        barrier: ControllerBarrierId,
    },
    ShuttingDown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeldReplacementStage {
    pub(crate) snapshot: RuntimeUiWireState,
    pub(crate) through: AcceptedStateRevision,
    pub(crate) covered: Vec<AcceptedStateRevision>,
    pub(crate) authority_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IntegratedSourceMutation {
    pub(crate) request: SourceMutationRequest,
    pub(crate) covered: Vec<AcceptedStateRevision>,
    pub(crate) result: SourceMutationResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbandonedSourceMutation {
    pub(crate) request: SourceMutationRequest,
    pub(crate) covered: Vec<AcceptedStateRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingReplacement {
    snapshot: RuntimeUiWireState,
    through: AcceptedStateRevision,
    covered: Vec<AcceptedStateRevision>,
    authority_epoch: u64,
}

impl From<PendingReplacement> for HeldReplacementStage {
    fn from(stage: PendingReplacement) -> Self {
        Self {
            snapshot: stage.snapshot,
            through: stage.through,
            covered: stage.covered,
            authority_epoch: stage.authority_epoch,
        }
    }
}

impl From<HeldReplacementStage> for PendingReplacement {
    fn from(stage: HeldReplacementStage) -> Self {
        Self {
            snapshot: stage.snapshot,
            through: stage.through,
            covered: stage.covered,
            authority_epoch: stage.authority_epoch,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingStage {
    Replace(PendingReplacement),
    Flush {
        id: FlushRequestId,
        through: AcceptedStateRevision,
    },
    ResetSupported {
        through: AcceptedStateRevision,
        publish_epoch: u64,
    },
    ResetUnsupportedIfUnchanged {
        through: AcceptedStateRevision,
        publish_epoch: u64,
        confirmation_revision: RuntimeStateSourceRevision,
    },
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InFlightSourceMutation {
    request: SourceMutationRequest,
    covered: Vec<AcceptedStateRevision>,
}

#[derive(Debug)]
pub(crate) struct PersistencePipeline {
    stable_source: RuntimeStateSourceRevision,
    acknowledged_wire: RuntimeUiWireState,
    settled_through: AcceptedStateRevision,
    next_accepted: AcceptedStateRevision,
    next_mutation_id: u64,
    next_flush_id: u64,
    in_flight: Option<InFlightSourceMutation>,
    outbound: Option<SourceMutationRequest>,
    pending: VecDeque<PendingStage>,
    receipts: BTreeMap<AcceptedStateRevision, Option<DurabilityOutcome>>,
    flushes: BTreeMap<FlushRequestId, Option<FlushOutcome>>,
    consumed_terminal_receipts: BTreeSet<AcceptedStateRevision>,
    earliest_consumed_non_durable: Option<AcceptedStateRevision>,
    shutdown_complete: bool,
    shutdown_requested: bool,
}

impl PersistencePipeline {
    pub(crate) fn new(
        stable_source: RuntimeStateSourceRevision,
        acknowledged_wire: RuntimeUiWireState,
    ) -> Self {
        Self {
            stable_source,
            acknowledged_wire,
            settled_through: AcceptedStateRevision(0),
            next_accepted: AcceptedStateRevision(0),
            next_mutation_id: 1,
            next_flush_id: 1,
            in_flight: None,
            outbound: None,
            pending: VecDeque::new(),
            receipts: BTreeMap::new(),
            flushes: BTreeMap::new(),
            consumed_terminal_receipts: BTreeSet::new(),
            earliest_consumed_non_durable: None,
            shutdown_complete: false,
            shutdown_requested: false,
        }
    }

    pub(crate) fn stable_source(&self) -> &RuntimeStateSourceRevision {
        &self.stable_source
    }

    pub(crate) fn acknowledged_wire(&self) -> &RuntimeUiWireState {
        &self.acknowledged_wire
    }

    pub(crate) fn settled_through(&self) -> AcceptedStateRevision {
        self.settled_through
    }

    pub(crate) fn latest_accepted(&self) -> AcceptedStateRevision {
        self.next_accepted
    }

    pub(crate) fn has_source_mutation_in_flight(&self) -> bool {
        self.in_flight.is_some()
    }

    pub(crate) fn source_mutation_in_flight(&self) -> Option<&SourceMutationRequest> {
        self.in_flight.as_ref().map(|in_flight| &in_flight.request)
    }

    pub(crate) fn abandon_in_flight_for_reinspection(&mut self) -> Option<AbandonedSourceMutation> {
        self.outbound = None;
        self.in_flight
            .take()
            .map(|in_flight| AbandonedSourceMutation {
                request: in_flight.request,
                covered: in_flight.covered,
            })
    }
}
