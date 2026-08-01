use std::cell::Cell;
use std::collections::{BTreeSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use super::*;

mod construction;
mod internals;
mod lifecycle;
mod mutations;
mod resets;
mod source_mutation;

static NEXT_CONTROLLER_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BeginMutationError {
    ControllerBusy(ControllerBarrierId),
    UnsupportedVersion,
    ShuttingDown,
    InvalidScope(MutationShapeError),
    Seed(SeedRegistryError),
    MutationIdExhausted,
}

#[derive(Debug)]
pub(crate) enum CommitResult {
    Accepted {
        through: AcceptedStateRevision,
    },
    NoChange,
    RejectedStaleAuthorityEpoch,
    RejectedSeedChanged {
        targets: Vec<InteractionSeedTarget>,
    },
    RejectedWrongController,
    RejectedUnsupportedVersion,
    RejectedShuttingDown,
    RejectedInvalidValues(MutationShapeError),
    RejectedControllerBusy {
        permit: RuntimeUiMutationPermit,
        barrier: ControllerBarrierId,
    },
    RejectedPersistence(PipelineProtocolError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UpdateSeedsResult {
    Applied {
        changed_targets: BTreeSet<InteractionSeedTarget>,
        cleanup_through: Option<AcceptedStateRevision>,
    },
    StagedBehindBarrier {
        barrier: ControllerBarrierId,
        replaced_older_staged_reload: bool,
    },
    RejectedShuttingDown,
    Rejected(SeedRegistryError),
    RejectedPersistence(PipelineProtocolError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RequestResetResult {
    Started {
        barrier: ControllerBarrierId,
        through: AcceptedStateRevision,
        publish_epoch: u64,
    },
    RequiresUnsupportedConfirmation {
        observed_version: Option<u64>,
        confirmation: UnsupportedResetConfirmation,
    },
    RejectedControllerBusy(ControllerBarrierId),
    RejectedUnsupportedVersion,
    RejectedShuttingDown,
    Rejected(PipelineProtocolError),
    EpochExhausted,
    BarrierIdExhausted,
    ConfirmationIdExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsupportedResetConfirmation {
    controller: ControllerId,
    id: UnsupportedResetConfirmationId,
    observed_version: Option<u64>,
    revision: RuntimeStateSourceRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConfirmUnsupportedResetResult {
    Started {
        barrier: ControllerBarrierId,
        through: AcceptedStateRevision,
        publish_epoch: u64,
    },
    RejectedToken,
    RejectedControllerBusy(ControllerBarrierId),
    RejectedShuttingDown,
    Rejected(PipelineProtocolError),
    EpochExhausted,
    BarrierIdExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CancelUnsupportedResetConfirmationResult {
    Cancelled,
    RejectedToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SubmitSourceMutationResult {
    Integrated {
        recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
    },
    ResetCompleted {
        barrier: ControllerBarrierId,
        published_epoch: u64,
        recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
    },
    ExternalReconciliationRequired {
        barrier: ControllerBarrierId,
        active: RuntimeStateSourceObservation,
        recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
        path_effect: RuntimeStateObservedPathEffect,
    },
    PersistenceUnhealthy {
        barrier: ControllerBarrierId,
        incident: PersistenceIncidentId,
        error: RuntimeStateIoError,
        active: Option<RuntimeStateSourceObservation>,
        recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
        path_effect: RuntimeStateFailurePathEffect,
    },
    PersistenceFailureSettledForShutdown {
        barrier: ControllerBarrierId,
        error: RuntimeStateIoError,
        active: Option<RuntimeStateSourceObservation>,
        recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
        path_effect: RuntimeStateFailurePathEffect,
    },
    ExternalReconciliationSettledForShutdown {
        barrier: ControllerBarrierId,
        active: RuntimeStateSourceObservation,
        recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
        path_effect: RuntimeStateObservedPathEffect,
    },
    Rejected(PipelineProtocolError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SupportedResetAuthoritySnapshot {
    pub(crate) source: RuntimeStateSourceRevision,
    pub(crate) file_status: RuntimeUiFileStatus,
    pub(crate) model: RuntimeUiModel,
    pub(crate) passthrough: WirePassthrough,
    pub(crate) seeds: InteractionSeedRegistry,
    pub(crate) live_state: RuntimeUiLiveState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SupportedResetAuthorityState {
    WaitingForPrerequisite,
    Captured(Box<SupportedResetAuthoritySnapshot>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SupportedResetTransaction {
    pub(crate) barrier: ControllerBarrierId,
    pub(crate) original_epoch: u64,
    pub(crate) publish_epoch: u64,
    pub(crate) through: AcceptedStateRevision,
    pub(crate) held_by_reset: Vec<HeldReplacementStage>,
    pub(crate) authority: SupportedResetAuthorityState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalReconciliationEvidence {
    pub(crate) writer_observation: RuntimeStateSourceObservation,
    pub(crate) recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
    pub(crate) path_effect: RuntimeStateObservedPathEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalAuthorityInstallResult {
    pub(crate) cleanup_through: Option<AcceptedStateRevision>,
    pub(crate) evidence: ExternalReconciliationEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExternalAuthorityInstallError {
    ShuttingDown,
    NoReconciliationPending,
    WrongBarrier,
    InconsistentObservation,
    InvalidAuthority { incident: PersistenceIncidentId },
    FileStatusMismatch,
    UnexpectedDecodedAuthority,
    AuthorityEpochExhausted,
    Seed(SeedRegistryError),
    Persistence(PipelineProtocolError),
}

#[derive(Debug)]
pub(crate) struct RuntimeUiStateController {
    pub(super) id: ControllerId,
    pub(super) authority_epoch: u64,
    next_mutation_id: Cell<u64>,
    pub(super) next_preview_session_id: Cell<u64>,
    pub(super) next_barrier_id: u64,
    pub(super) next_incident_id: u64,
    pub(super) next_recovery_attempt_id: u64,
    pub(super) next_recovery_handle_id: u64,
    pub(super) next_recovery_command_id: u64,
    pub(super) next_recovery_lease_nonce: u64,
    pub(super) next_unsupported_reset_confirmation_id: u64,
    pub(super) seeds: InteractionSeedRegistry,
    pub(super) model: RuntimeUiModel,
    pub(super) passthrough: WirePassthrough,
    pub(super) live_only_overlay: RuntimeUiLiveOnlyOverlay,
    pub(super) live_state: RuntimeUiLiveState,
    pub(super) file_status: RuntimeUiFileStatus,
    pub(super) pipeline: PersistencePipeline,
    pub(super) active_barrier: Option<ActiveControllerBarrier>,
    pub(super) staged_reload: Option<StagedSeedReload>,
    pub(super) supported_reset: Option<SupportedResetTransaction>,
    pub(super) pending_unsupported_reset_confirmation: Option<UnsupportedResetConfirmation>,
    pub(super) external_reconciliation: Option<ExternalReconciliationEvidence>,
    pub(super) incident: Option<PersistenceIncident>,
    pub(super) abandoned_previews: Vec<BarrierAbandonedPreview>,
    pub(super) preview_resolution_outbox: Vec<AbandonedPreviewResolution>,
    pub(super) active_recovery: Option<ActiveRecoveryAttempt>,
    pub(super) recovery_outbox: VecDeque<RecoveryIoCommand>,
    pub(super) integrated_recovery_commands: BTreeSet<RecoveryCommandId>,
    pub(super) integrated_recovery_command_order: VecDeque<RecoveryCommandId>,
    pub(super) rejected_recovery_completions: VecDeque<RecoveryIoCompletion>,
    pub(super) cancelled_read_only_commands: BTreeSet<RecoveryCommandId>,
    pub(super) cancelled_read_only_command_order: VecDeque<RecoveryCommandId>,
    pub(super) lifecycle_tx: Sender<LifecycleControl>,
    lifecycle_rx: Receiver<LifecycleControl>,
    pub(super) shutting_down: bool,
}

fn file_status_matches_observation(
    status: &RuntimeUiFileStatus,
    envelope: &RuntimeStateObservedEnvelope,
) -> bool {
    matches!(
        (status, envelope),
        (
            RuntimeUiFileStatus::Missing,
            RuntimeStateObservedEnvelope::Missing
        ) | (
            RuntimeUiFileStatus::Supported,
            RuntimeStateObservedEnvelope::Version(1),
        )
    ) || matches!(
        (status, envelope),
        (
            RuntimeUiFileStatus::UnsupportedReadOnly {
                version: Some(status_version),
            },
            RuntimeStateObservedEnvelope::Version(observed_version),
        ) if status_version == observed_version && *observed_version != 1
    )
}
