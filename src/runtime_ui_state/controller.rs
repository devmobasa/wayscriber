use std::cell::Cell;
use std::collections::{BTreeSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use super::*;

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
pub(crate) enum BeginConfigInteractionError {
    ControllerBusy(ControllerBarrierId),
    ShuttingDown,
    Seed(SeedRegistryError),
    MutationIdExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ValidateConfigInteractionResult {
    Accepted(ConfigPositionTarget),
    RejectedWrongController,
    RejectedShuttingDown,
    RejectedStaleAuthority,
    RejectedSeedChanged,
    RejectedControllerBusy(ControllerBarrierId),
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
    Integrated,
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
    Captured(SupportedResetAuthoritySnapshot),
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

impl RuntimeUiStateController {
    pub(crate) fn new(
        seeds: ValidatedInteractionSeeds,
        stable_source: RuntimeStateSourceRevision,
        file_status: RuntimeUiFileStatus,
    ) -> Self {
        Self::new_with_authority(
            seeds,
            stable_source,
            file_status,
            RuntimeUiWireState::default(),
        )
    }

    pub(crate) fn new_with_authority(
        seeds: ValidatedInteractionSeeds,
        stable_source: RuntimeStateSourceRevision,
        file_status: RuntimeUiFileStatus,
        acknowledged: RuntimeUiWireState,
    ) -> Self {
        debug_assert_eq!(
            stable_source.bytes().is_none(),
            matches!(file_status, RuntimeUiFileStatus::Missing),
            "startup file status must match the exact source revision"
        );
        debug_assert!(
            matches!(file_status, RuntimeUiFileStatus::Supported)
                || (acknowledged.model.is_empty() && acknowledged.passthrough.values().is_empty()),
            "missing, unsupported, and invalid startup authorities cannot carry decoded V1 state"
        );
        let acknowledged = if matches!(file_status, RuntimeUiFileStatus::Supported) {
            acknowledged
        } else {
            RuntimeUiWireState::default()
        };
        let id = ControllerId(
            NEXT_CONTROLLER_ID
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                    current.checked_add(1)
                })
                .expect("controller id space exhausted"),
        );
        let seeds = InteractionSeedRegistry::from_validated(seeds);
        let mut model = acknowledged.model.clone();
        let passthrough = acknowledged.passthrough.clone();
        let needs_cleanup = model.reconcile(&seeds);
        let live_only_overlay = RuntimeUiLiveOnlyOverlay::default();
        let live_state = RuntimeUiLiveState::rebuild(&seeds, &model, &live_only_overlay);
        let canonical = RuntimeUiWireState {
            model: model.clone(),
            passthrough: passthrough.clone(),
        };
        let mut pipeline = PersistencePipeline::new(stable_source, acknowledged);
        if needs_cleanup {
            pipeline
                .accept_replace(canonical, 1)
                .expect("fresh startup pipeline must accept reconciliation cleanup");
        }
        let (lifecycle_tx, lifecycle_rx) = channel();
        Self {
            id,
            authority_epoch: 1,
            next_mutation_id: Cell::new(1),
            next_preview_session_id: Cell::new(1),
            next_barrier_id: 1,
            next_incident_id: 1,
            next_recovery_attempt_id: 1,
            next_recovery_handle_id: 1,
            next_recovery_command_id: 1,
            next_recovery_lease_nonce: 1,
            next_unsupported_reset_confirmation_id: 1,
            seeds,
            model,
            passthrough,
            live_only_overlay,
            live_state,
            file_status,
            pipeline,
            active_barrier: None,
            staged_reload: None,
            supported_reset: None,
            pending_unsupported_reset_confirmation: None,
            external_reconciliation: None,
            incident: None,
            abandoned_previews: Vec::new(),
            preview_resolution_outbox: Vec::new(),
            active_recovery: None,
            recovery_outbox: VecDeque::new(),
            integrated_recovery_commands: BTreeSet::new(),
            integrated_recovery_command_order: VecDeque::new(),
            rejected_recovery_completions: VecDeque::new(),
            cancelled_read_only_commands: BTreeSet::new(),
            cancelled_read_only_command_order: VecDeque::new(),
            lifecycle_tx,
            lifecycle_rx,
            shutting_down: false,
        }
    }

    pub(crate) fn new_startup_unhealthy(
        seeds: ValidatedInteractionSeeds,
        observed: RuntimeStateSourceObservation,
        error: RuntimeStateIoError,
        recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
        path_effect: RuntimeStateFailurePathEffect,
    ) -> (Self, PersistenceIncidentId) {
        let mut controller = Self::new(
            seeds,
            observed.revision.clone(),
            RuntimeUiFileStatus::Invalid,
        );
        let incident = controller.enter_persistence_incident(
            error,
            Some(observed),
            recovery_artifacts,
            path_effect,
            None,
        );
        controller
            .active_barrier
            .as_mut()
            .expect("startup incident installs a barrier")
            .operation = ControllerBarrierOperation::StartupPersistenceRecovery;
        (controller, incident)
    }

    pub(crate) fn id(&self) -> ControllerId {
        self.id
    }

    pub(crate) fn authority_epoch(&self) -> u64 {
        self.authority_epoch
    }

    pub(crate) fn seeds(&self) -> &InteractionSeedRegistry {
        &self.seeds
    }

    pub(crate) fn model(&self) -> &RuntimeUiModel {
        &self.model
    }

    pub(crate) fn live_state(&self) -> &RuntimeUiLiveState {
        &self.live_state
    }

    pub(crate) fn active_barrier(&self) -> Option<&ActiveControllerBarrier> {
        self.active_barrier.as_ref()
    }

    pub(crate) fn take_preview_resolutions(&mut self) -> Vec<AbandonedPreviewResolution> {
        std::mem::take(&mut self.preview_resolution_outbox)
    }

    pub(super) fn close_barrier_and_resolve_previews(
        &mut self,
        barrier: ControllerBarrierId,
        reason: AbandonedPreviewResolutionReason,
    ) {
        self.close_barrier_and_resolve_previews_after_seed_changes(barrier, reason, None);
    }

    pub(super) fn close_barrier_and_resolve_previews_after_seed_changes(
        &mut self,
        barrier: ControllerBarrierId,
        reason: AbandonedPreviewResolutionReason,
        changed_targets: Option<&BTreeSet<InteractionSeedTarget>>,
    ) {
        self.resolve_previews_while_barrier_retained(barrier, reason, changed_targets);
        if self
            .active_barrier
            .as_ref()
            .is_some_and(|active| active.id == barrier)
        {
            self.active_barrier = None;
        }
    }

    pub(super) fn resolve_previews_while_barrier_retained(
        &mut self,
        barrier: ControllerBarrierId,
        reason: AbandonedPreviewResolutionReason,
        changed_targets: Option<&BTreeSet<InteractionSeedTarget>>,
    ) {
        let resolved = self.resolve_abandoned_previews(barrier, reason, changed_targets);
        self.preview_resolution_outbox.extend(resolved);
    }

    pub(crate) fn pipeline(&self) -> &PersistencePipeline {
        &self.pipeline
    }

    pub(crate) fn begin_mutation(
        &self,
        scope: RuntimeUiMutationScope,
    ) -> Result<RuntimeUiMutationPermit, BeginMutationError> {
        if self.shutting_down {
            return Err(BeginMutationError::ShuttingDown);
        }
        if let Some(barrier) = &self.active_barrier {
            return Err(BeginMutationError::ControllerBusy(barrier.id));
        }
        if matches!(
            self.file_status,
            RuntimeUiFileStatus::UnsupportedReadOnly { .. } | RuntimeUiFileStatus::Invalid
        ) {
            return Err(BeginMutationError::UnsupportedVersion);
        }
        let targets = scope
            .canonical_targets()
            .map_err(BeginMutationError::InvalidScope)?;
        let guards = self
            .seeds
            .guards(&targets)
            .map_err(BeginMutationError::Seed)?;
        Ok(RuntimeUiMutationPermit {
            controller_id: self.id,
            authority_epoch: self.authority_epoch,
            mutation_id: self
                .allocate_mutation_id()
                .ok_or(BeginMutationError::MutationIdExhausted)?,
            guards,
        })
    }

    pub(crate) fn commit(
        &mut self,
        permit: RuntimeUiMutationPermit,
        desired_values: RuntimeUiMutationValues,
    ) -> CommitResult {
        self.drain_lifecycle_controls();
        if let Some(barrier) = &self.active_barrier {
            return CommitResult::RejectedControllerBusy {
                permit,
                barrier: barrier.id,
            };
        }
        if self.shutting_down {
            return CommitResult::RejectedShuttingDown;
        }
        if permit.controller_id != self.id {
            return CommitResult::RejectedWrongController;
        }
        if matches!(
            self.file_status,
            RuntimeUiFileStatus::UnsupportedReadOnly { .. } | RuntimeUiFileStatus::Invalid
        ) {
            return CommitResult::RejectedUnsupportedVersion;
        }
        if permit.authority_epoch != self.authority_epoch {
            return CommitResult::RejectedStaleAuthorityEpoch;
        }
        if desired_values.targets() != permit.targets() {
            return CommitResult::RejectedInvalidValues(
                MutationShapeError::ValuesDoNotMatchPermitScope,
            );
        }
        let changed_targets = permit
            .guards
            .iter()
            .filter(|guard| !self.seeds.guard_is_current(guard))
            .map(|guard| guard.target.clone())
            .collect::<Vec<_>>();
        if !changed_targets.is_empty() {
            return CommitResult::RejectedSeedChanged {
                targets: changed_targets,
            };
        }

        let previous_model = self.model.clone();
        if !self.model.apply(&permit.guards, &desired_values) {
            return CommitResult::NoChange;
        }
        self.rebuild_live_state();
        let snapshot = self.canonical_wire();
        match self.pipeline.accept_replace(snapshot, self.authority_epoch) {
            Ok(through) => CommitResult::Accepted { through },
            Err(error) => {
                self.model = previous_model;
                self.rebuild_live_state();
                CommitResult::RejectedPersistence(error)
            }
        }
    }

    pub(crate) fn begin_config_interaction(
        &self,
        target: ConfigPositionTarget,
    ) -> Result<ConfigInteractionPermit, BeginConfigInteractionError> {
        if self.shutting_down {
            return Err(BeginConfigInteractionError::ShuttingDown);
        }
        if let Some(barrier) = &self.active_barrier {
            return Err(BeginConfigInteractionError::ControllerBusy(barrier.id));
        }
        let guard = self
            .seeds
            .guard(&target.seed_target())
            .map_err(BeginConfigInteractionError::Seed)?;
        Ok(ConfigInteractionPermit {
            controller_id: self.id,
            authority_epoch: self.authority_epoch,
            mutation_id: self
                .allocate_mutation_id()
                .ok_or(BeginConfigInteractionError::MutationIdExhausted)?,
            guard,
            target,
        })
    }

    pub(crate) fn validate_config_interaction(
        &self,
        permit: ConfigInteractionPermit,
    ) -> ValidateConfigInteractionResult {
        if permit.controller_id != self.id {
            return ValidateConfigInteractionResult::RejectedWrongController;
        }
        if self.shutting_down {
            return ValidateConfigInteractionResult::RejectedShuttingDown;
        }
        if let Some(barrier) = &self.active_barrier {
            return ValidateConfigInteractionResult::RejectedControllerBusy(barrier.id);
        }
        if permit.authority_epoch != self.authority_epoch {
            return ValidateConfigInteractionResult::RejectedStaleAuthority;
        }
        if !self.seeds.guard_is_current(&permit.guard) {
            return ValidateConfigInteractionResult::RejectedSeedChanged;
        }
        ValidateConfigInteractionResult::Accepted(permit.target)
    }

    pub(crate) fn update_seeds(&mut self, seeds: ValidatedInteractionSeeds) -> UpdateSeedsResult {
        self.drain_lifecycle_controls();
        if self.shutting_down {
            return UpdateSeedsResult::RejectedShuttingDown;
        }
        if let Some(barrier) = &self.active_barrier {
            if let Some(incident) = &mut self.incident {
                let staged = match StagedSeedReload::stage(
                    incident.staged_reload.as_ref(),
                    &self.seeds,
                    seeds,
                ) {
                    Ok(staged) => staged,
                    Err(error) => return UpdateSeedsResult::Rejected(error),
                };
                let replaced_older_staged_reload = incident.staged_reload.replace(staged).is_some();
                incident.cleanup.mark_recompute();
                return UpdateSeedsResult::StagedBehindBarrier {
                    barrier: barrier.id,
                    replaced_older_staged_reload,
                };
            }
            let staged =
                match StagedSeedReload::stage(self.staged_reload.as_ref(), &self.seeds, seeds) {
                    Ok(staged) => staged,
                    Err(error) => return UpdateSeedsResult::Rejected(error),
                };
            let replaced_older_staged_reload = self.staged_reload.replace(staged).is_some();
            return UpdateSeedsResult::StagedBehindBarrier {
                barrier: barrier.id,
                replaced_older_staged_reload,
            };
        }
        self.apply_seed_update(seeds)
    }

    pub(crate) fn request_supported_reset(&mut self) -> RequestResetResult {
        self.request_runtime_ui_reset()
    }

    pub(crate) fn request_runtime_ui_reset(&mut self) -> RequestResetResult {
        self.drain_lifecycle_controls();
        if self.shutting_down {
            return RequestResetResult::RejectedShuttingDown;
        }
        if let Some(barrier) = &self.active_barrier {
            return RequestResetResult::RejectedControllerBusy(barrier.id);
        }
        if let RuntimeUiFileStatus::UnsupportedReadOnly { version } = self.file_status {
            let id = UnsupportedResetConfirmationId(
                match self.next_unsupported_reset_confirmation_id.checked_add(1) {
                    Some(next) => {
                        let current = self.next_unsupported_reset_confirmation_id;
                        self.next_unsupported_reset_confirmation_id = next;
                        current
                    }
                    None => return RequestResetResult::ConfirmationIdExhausted,
                },
            );
            let confirmation = UnsupportedResetConfirmation {
                controller: self.id,
                id,
                observed_version: version,
                revision: self.pipeline.stable_source().clone(),
            };
            self.pending_unsupported_reset_confirmation = Some(confirmation.clone());
            return RequestResetResult::RequiresUnsupportedConfirmation {
                observed_version: version,
                confirmation,
            };
        }
        if matches!(self.file_status, RuntimeUiFileStatus::Invalid) {
            return RequestResetResult::RejectedUnsupportedVersion;
        }
        self.pending_unsupported_reset_confirmation = None;
        let Some(publish_epoch) = self.authority_epoch.checked_add(1) else {
            return RequestResetResult::EpochExhausted;
        };
        let Some(barrier_id) = self.allocate_barrier_id() else {
            return RequestResetResult::BarrierIdExhausted;
        };
        let through = match self.pipeline.allocate_reset_revision() {
            Ok(through) => through,
            Err(error) => return RequestResetResult::Rejected(error),
        };
        let held_by_reset = self.pipeline.hold_trailing_replacements();
        let waiting = self.pipeline.has_source_mutation_in_flight();
        self.active_barrier = Some(ActiveControllerBarrier {
            id: barrier_id,
            operation: ControllerBarrierOperation::ResetSupported,
            phase: if let Some(request) = self.pipeline.source_mutation_in_flight() {
                ControllerBarrierPhase::WaitingForPrerequisite(request.id)
            } else {
                ControllerBarrierPhase::Inspecting
            },
        });
        self.supported_reset = Some(SupportedResetTransaction {
            barrier: barrier_id,
            original_epoch: self.authority_epoch,
            publish_epoch,
            through,
            held_by_reset,
            authority: if waiting {
                SupportedResetAuthorityState::WaitingForPrerequisite
            } else {
                SupportedResetAuthorityState::Captured(self.capture_reset_authority())
            },
        });
        if let Err(error) = self.pipeline.stage_supported_reset(through, publish_epoch) {
            self.active_barrier = None;
            if let Some(transaction) = self.supported_reset.take() {
                let reset_error = RuntimeStateIoError::new("reset dispatch failed");
                if !self.pipeline.cancel_pending_reset(
                    transaction.through,
                    DurabilityOutcome::Failed(reset_error.clone()),
                ) {
                    self.pipeline
                        .settle_failed([transaction.through], reset_error);
                }
                let held_by_reset = transaction.held_by_reset;
                if let Err(restore_error) = self
                    .pipeline
                    .restore_held_replacements(held_by_reset.clone())
                {
                    self.pipeline.settle_held_failed(
                        &held_by_reset,
                        RuntimeStateIoError::new(format!(
                            "failed to restore reset-held state: {restore_error:?}"
                        )),
                    );
                }
            }
            return RequestResetResult::Rejected(error);
        }
        self.refresh_reset_barrier_phase();
        RequestResetResult::Started {
            barrier: barrier_id,
            through,
            publish_epoch,
        }
    }

    pub(crate) fn cancel_unsupported_reset_confirmation(
        &mut self,
        confirmation: UnsupportedResetConfirmation,
    ) -> CancelUnsupportedResetConfirmationResult {
        self.drain_lifecycle_controls();
        if !self.unsupported_reset_confirmation_is_current(&confirmation) {
            return CancelUnsupportedResetConfirmationResult::RejectedToken;
        }
        self.pending_unsupported_reset_confirmation = None;
        CancelUnsupportedResetConfirmationResult::Cancelled
    }

    pub(crate) fn confirm_unsupported_reset(
        &mut self,
        confirmation: UnsupportedResetConfirmation,
    ) -> ConfirmUnsupportedResetResult {
        self.drain_lifecycle_controls();
        if !self.unsupported_reset_confirmation_is_current(&confirmation) {
            return ConfirmUnsupportedResetResult::RejectedToken;
        }
        self.pending_unsupported_reset_confirmation = None;
        if self.shutting_down {
            return ConfirmUnsupportedResetResult::RejectedShuttingDown;
        }
        if let Some(barrier) = &self.active_barrier {
            return ConfirmUnsupportedResetResult::RejectedControllerBusy(barrier.id);
        }
        let Some(publish_epoch) = self.authority_epoch.checked_add(1) else {
            return ConfirmUnsupportedResetResult::EpochExhausted;
        };
        let Some(barrier_id) = self.allocate_barrier_id() else {
            return ConfirmUnsupportedResetResult::BarrierIdExhausted;
        };
        let through = match self.pipeline.allocate_reset_revision() {
            Ok(through) => through,
            Err(error) => return ConfirmUnsupportedResetResult::Rejected(error),
        };
        let held_by_reset = self.pipeline.hold_trailing_replacements();
        let waiting = self.pipeline.has_source_mutation_in_flight();
        self.active_barrier = Some(ActiveControllerBarrier {
            id: barrier_id,
            operation: ControllerBarrierOperation::ConfirmUnsupportedReset,
            phase: if let Some(request) = self.pipeline.source_mutation_in_flight() {
                ControllerBarrierPhase::WaitingForPrerequisite(request.id)
            } else {
                ControllerBarrierPhase::Inspecting
            },
        });
        self.supported_reset = Some(SupportedResetTransaction {
            barrier: barrier_id,
            original_epoch: self.authority_epoch,
            publish_epoch,
            through,
            held_by_reset,
            authority: if waiting {
                SupportedResetAuthorityState::WaitingForPrerequisite
            } else {
                SupportedResetAuthorityState::Captured(self.capture_reset_authority())
            },
        });
        if let Err(error) =
            self.pipeline
                .stage_unsupported_reset(through, publish_epoch, confirmation.revision)
        {
            self.active_barrier = None;
            if let Some(transaction) = self.supported_reset.take() {
                let reset_error = RuntimeStateIoError::new("unsupported reset dispatch failed");
                if !self.pipeline.cancel_pending_reset(
                    transaction.through,
                    DurabilityOutcome::Failed(reset_error.clone()),
                ) {
                    self.pipeline
                        .settle_failed([transaction.through], reset_error);
                }
                let held_by_reset = transaction.held_by_reset;
                if let Err(restore_error) = self
                    .pipeline
                    .restore_held_replacements(held_by_reset.clone())
                {
                    self.pipeline.settle_held_failed(
                        &held_by_reset,
                        RuntimeStateIoError::new(format!(
                            "failed to restore unsupported-reset-held state: {restore_error:?}"
                        )),
                    );
                }
            }
            return ConfirmUnsupportedResetResult::Rejected(error);
        }
        self.refresh_reset_barrier_phase();
        ConfirmUnsupportedResetResult::Started {
            barrier: barrier_id,
            through,
            publish_epoch,
        }
    }

    pub(crate) fn take_source_mutation(&mut self) -> Option<SourceMutationRequest> {
        self.pipeline.take_outbound()
    }

    pub(crate) fn submit_source_mutation(
        &mut self,
        result: SourceMutationResult,
    ) -> SubmitSourceMutationResult {
        self.drain_lifecycle_controls();
        let integrated = match self.pipeline.integrate(result) {
            Ok(integrated) => integrated,
            Err(error) => return SubmitSourceMutationResult::Rejected(error),
        };

        match &integrated.result {
            SourceMutationResult::Applied { .. } => {
                if matches!(&integrated.request.kind, SourceMutationKind::Replace(_)) {
                    self.file_status = RuntimeUiFileStatus::Supported;
                }
                if matches!(
                    &integrated.request.kind,
                    SourceMutationKind::ResetSupported { .. }
                        | SourceMutationKind::ResetUnsupportedIfUnchanged { .. }
                ) {
                    return self.finish_supported_reset_success(integrated);
                }
                if self.supported_reset.is_some() {
                    self.capture_reset_authority_after_prerequisite();
                }
                if let Err(error) = self.pipeline.resume_after_integration() {
                    return SubmitSourceMutationResult::Rejected(error);
                }
                self.refresh_reset_barrier_phase();
                if matches!(
                    self.active_barrier
                        .as_ref()
                        .map(|barrier| &barrier.operation),
                    Some(ControllerBarrierOperation::ExternalAuthorityReconciliation)
                ) && !self.pipeline.has_source_mutation_in_flight()
                    && self.pipeline.pending_replacements() == 0
                {
                    if self.shutting_down {
                        self.settle_external_reconciliation_for_shutdown();
                    } else if let Err(error) = self.finish_external_reconciliation_write() {
                        return SubmitSourceMutationResult::Rejected(error);
                    }
                }
                SubmitSourceMutationResult::Integrated
            }
            SourceMutationResult::SourceChangedBeforeMutation { active, .. } => {
                self.enter_external_reconciliation(
                    active.clone(),
                    Vec::new(),
                    RuntimeStateObservedPathEffect::Untouched,
                );
                let barrier = self.active_barrier.as_ref().expect("barrier installed").id;
                if self.shutting_down {
                    self.settle_external_reconciliation_for_shutdown();
                    return SubmitSourceMutationResult::ExternalReconciliationSettledForShutdown {
                        barrier,
                        active: active.clone(),
                        recovery_artifacts: Vec::new(),
                        path_effect: RuntimeStateObservedPathEffect::Untouched,
                    };
                }
                SubmitSourceMutationResult::ExternalReconciliationRequired {
                    barrier,
                    active: active.clone(),
                    recovery_artifacts: Vec::new(),
                    path_effect: RuntimeStateObservedPathEffect::Untouched,
                }
            }
            SourceMutationResult::ObservationChangedAfterClaim {
                active,
                recovery_artifacts,
                path_effect,
                ..
            } => {
                let observed_effect =
                    RuntimeStateObservedPathEffect::PostClaim(path_effect.clone());
                self.enter_external_reconciliation(
                    active.clone(),
                    recovery_artifacts.clone(),
                    observed_effect.clone(),
                );
                let barrier = self.active_barrier.as_ref().expect("barrier installed").id;
                if self.shutting_down {
                    self.settle_external_reconciliation_for_shutdown();
                    return SubmitSourceMutationResult::ExternalReconciliationSettledForShutdown {
                        barrier,
                        active: active.clone(),
                        recovery_artifacts: recovery_artifacts.clone(),
                        path_effect: observed_effect,
                    };
                }
                SubmitSourceMutationResult::ExternalReconciliationRequired {
                    barrier,
                    active: active.clone(),
                    recovery_artifacts: recovery_artifacts.clone(),
                    path_effect: observed_effect,
                }
            }
            SourceMutationResult::Failed {
                error,
                active,
                recovery_artifacts,
                path_effect,
                ..
            } => {
                let failed_replacement = match &integrated.request.kind {
                    SourceMutationKind::Replace(snapshot) => Some(HeldReplacementStage {
                        snapshot: snapshot.clone(),
                        through: integrated.request.accepted_through,
                        covered: integrated.covered.clone(),
                        authority_epoch: integrated.request.expected_epoch,
                    }),
                    SourceMutationKind::ResetSupported { .. }
                    | SourceMutationKind::ResetUnsupportedIfUnchanged { .. } => {
                        self.pipeline
                            .settle_failed(integrated.covered.iter().copied(), error.clone());
                        None
                    }
                };
                let incident = self.enter_persistence_incident(
                    error.clone(),
                    active.clone(),
                    recovery_artifacts.clone(),
                    path_effect.clone(),
                    failed_replacement,
                );
                let barrier = self.active_barrier.as_ref().expect("barrier installed").id;
                if self.shutting_down {
                    self.settle_incident_for_shutdown();
                    if let Err(error) = self.pipeline.resume_after_integration() {
                        return SubmitSourceMutationResult::Rejected(error);
                    }
                    return SubmitSourceMutationResult::PersistenceFailureSettledForShutdown {
                        barrier,
                        error: error.clone(),
                        active: active.clone(),
                        recovery_artifacts: recovery_artifacts.clone(),
                        path_effect: path_effect.clone(),
                    };
                }
                SubmitSourceMutationResult::PersistenceUnhealthy {
                    barrier,
                    incident,
                    error: error.clone(),
                    active: active.clone(),
                    recovery_artifacts: recovery_artifacts.clone(),
                    path_effect: path_effect.clone(),
                }
            }
        }
    }

    pub(crate) fn install_external_authority(
        &mut self,
        barrier: ControllerBarrierId,
        observation: RuntimeStateSourceObservation,
        file_status: RuntimeUiFileStatus,
        model: RuntimeUiModel,
        passthrough: WirePassthrough,
    ) -> Result<ExternalAuthorityInstallResult, ExternalAuthorityInstallError> {
        if self.shutting_down {
            return Err(ExternalAuthorityInstallError::ShuttingDown);
        }
        if self.external_reconciliation.is_none() {
            return Err(ExternalAuthorityInstallError::NoReconciliationPending);
        }
        if !matches!(
            self.active_barrier.as_ref(),
            Some(active)
                if active.id == barrier
                    && active.operation
                        == ControllerBarrierOperation::ExternalAuthorityReconciliation
        ) {
            return Err(ExternalAuthorityInstallError::WrongBarrier);
        }
        if !observation.is_consistent() {
            return Err(ExternalAuthorityInstallError::InconsistentObservation);
        }
        if matches!(
            observation.envelope,
            RuntimeStateObservedEnvelope::PresentWithoutReadableVersion
        ) {
            let evidence = self
                .external_reconciliation
                .take()
                .expect("external reconciliation validated above");
            self.pipeline.install_acknowledged_authority(
                observation.revision.clone(),
                RuntimeUiWireState::default(),
            );
            self.file_status = RuntimeUiFileStatus::Invalid;
            self.model.clear();
            self.passthrough = WirePassthrough::default();
            self.live_only_overlay.clear();
            self.rebuild_live_state();
            let incident = self.enter_persistence_incident(
                RuntimeStateIoError::new(
                    "external runtime-state authority is malformed or unreadable",
                ),
                Some(observation),
                evidence.recovery_artifacts,
                RuntimeStateFailurePathEffect::Known(evidence.path_effect),
                None,
            );
            return Err(ExternalAuthorityInstallError::InvalidAuthority { incident });
        }
        if !file_status_matches_observation(&file_status, &observation.envelope) {
            return Err(ExternalAuthorityInstallError::FileStatusMismatch);
        }
        if !matches!(file_status, RuntimeUiFileStatus::Supported)
            && (!model.is_empty() || !passthrough.values().is_empty())
        {
            return Err(ExternalAuthorityInstallError::UnexpectedDecodedAuthority);
        }
        let retains_live_only_authority =
            matches!(
                self.file_status,
                RuntimeUiFileStatus::UnsupportedReadOnly { .. }
            ) && matches!(file_status, RuntimeUiFileStatus::UnsupportedReadOnly { .. });
        let next_epoch = if retains_live_only_authority {
            self.authority_epoch
        } else {
            self.authority_epoch
                .checked_add(1)
                .ok_or(ExternalAuthorityInstallError::AuthorityEpochExhausted)?
        };
        let observed_wire = RuntimeUiWireState {
            model: model.clone(),
            passthrough: passthrough.clone(),
        };
        let (next_seeds, changed_targets) = self
            .staged_reload
            .as_ref()
            .cloned()
            .map(StagedSeedReload::into_parts)
            .unwrap_or_else(|| (self.seeds.clone(), BTreeSet::new()));
        let mut canonical_model = model;
        canonical_model.reconcile(&next_seeds);
        let canonical_wire = RuntimeUiWireState {
            model: canonical_model.clone(),
            passthrough: passthrough.clone(),
        };
        let needs_cleanup = matches!(file_status, RuntimeUiFileStatus::Supported)
            && canonical_wire != observed_wire;
        if needs_cleanup {
            self.pipeline
                .preflight_accept_replace()
                .map_err(ExternalAuthorityInstallError::Persistence)?;
        }

        self.staged_reload = None;
        self.seeds = next_seeds;
        self.file_status = file_status;
        self.model = canonical_model;
        self.passthrough = passthrough;
        self.pipeline
            .install_acknowledged_authority(observation.revision.clone(), observed_wire);
        if retains_live_only_authority {
            self.live_only_overlay.reconcile(&changed_targets);
        } else {
            self.live_only_overlay.clear();
        }
        self.authority_epoch = next_epoch;
        if let Some(transaction) = self.supported_reset.take() {
            self.pipeline
                .settle_held_external(&transaction.held_by_reset);
        }
        self.rebuild_live_state();
        let cleanup = needs_cleanup.then(|| {
            self.pipeline
                .accept_replace(canonical_wire, self.authority_epoch)
                .expect("preflighted external-authority cleanup must dispatch")
        });
        if cleanup.is_none() {
            if let Some(barrier) = self.active_barrier.as_ref().map(|barrier| barrier.id) {
                self.close_barrier_and_resolve_previews_after_seed_changes(
                    barrier,
                    if retains_live_only_authority {
                        AbandonedPreviewResolutionReason::CancelledUnderRetainedAuthority
                    } else {
                        AbandonedPreviewResolutionReason::DiscardedForAuthorityChange
                    },
                    Some(&changed_targets),
                );
            }
        } else if let Some(barrier) = &mut self.active_barrier {
            barrier.phase = ControllerBarrierPhase::Writing(
                self.pipeline
                    .source_mutation_in_flight()
                    .expect("cleanup dispatched")
                    .id,
            );
        }
        let evidence = self
            .external_reconciliation
            .take()
            .expect("reconciliation evidence validated before authority installation");
        Ok(ExternalAuthorityInstallResult {
            cleanup_through: cleanup,
            evidence,
        })
    }

    #[cfg(test)]
    pub(crate) fn receipt(&self, revision: AcceptedStateRevision) -> Option<&DurabilityOutcome> {
        self.pipeline.receipt(revision)
    }

    pub(crate) fn take_receipt(
        &mut self,
        revision: AcceptedStateRevision,
    ) -> Option<DurabilityOutcome> {
        self.pipeline.take_receipt(revision)
    }

    pub(crate) fn request_flush(
        &mut self,
        through: AcceptedStateRevision,
    ) -> Result<FlushRequestId, PipelineProtocolError> {
        self.drain_lifecycle_controls();
        if self.shutting_down {
            return Err(PipelineProtocolError::ShuttingDown);
        }
        if let Some(barrier) = &self.active_barrier {
            return Err(PipelineProtocolError::ControllerBarrierActive {
                barrier: barrier.id,
            });
        }
        self.pipeline.request_flush(through)
    }

    #[cfg(test)]
    pub(crate) fn flush_outcome(&self, id: FlushRequestId) -> Option<&FlushOutcome> {
        self.pipeline.flush_outcome(id)
    }

    pub(crate) fn take_flush_outcome(&mut self, id: FlushRequestId) -> Option<FlushOutcome> {
        self.pipeline.take_flush_outcome(id)
    }

    pub(crate) fn request_shutdown(&mut self) -> Result<(), PipelineProtocolError> {
        self.shutting_down = true;
        self.drain_lifecycle_controls();
        self.settle_external_reconciliation_for_shutdown();
        if self.prepare_recovery_shutdown() {
            self.pipeline.request_shutdown()
        } else {
            Ok(())
        }
    }

    pub(crate) fn drain_lifecycle_controls(&mut self) {
        while let Ok(control) = self.lifecycle_rx.try_recv() {
            self.apply_lifecycle_control(control);
        }
    }

    fn apply_seed_update(&mut self, seeds: ValidatedInteractionSeeds) -> UpdateSeedsResult {
        let mut next_seeds = self.seeds.clone();
        let changed_targets = match next_seeds.update(seeds) {
            Ok(changed) => changed,
            Err(error) => return UpdateSeedsResult::Rejected(error),
        };
        let mut next_live_only_overlay = self.live_only_overlay.clone();
        next_live_only_overlay.reconcile(&changed_targets);
        let mut next_model = self.model.clone();
        let pruned = next_model.reconcile(&next_seeds);
        let next_live_state =
            RuntimeUiLiveState::rebuild(&next_seeds, &next_model, &next_live_only_overlay);
        let needs_cleanup = pruned
            && matches!(
                self.file_status,
                RuntimeUiFileStatus::Missing | RuntimeUiFileStatus::Supported
            );
        if needs_cleanup && let Err(error) = self.pipeline.preflight_accept_replace() {
            return UpdateSeedsResult::RejectedPersistence(error);
        }

        self.seeds = next_seeds;
        self.live_only_overlay = next_live_only_overlay;
        self.model = next_model;
        self.live_state = next_live_state;
        let cleanup_through = if needs_cleanup {
            Some(
                self.pipeline
                    .accept_replace(self.canonical_wire(), self.authority_epoch)
                    .expect("preflighted seed-reconciliation cleanup must dispatch"),
            )
        } else {
            None
        };
        UpdateSeedsResult::Applied {
            changed_targets,
            cleanup_through,
        }
    }

    fn finish_external_reconciliation_write(&mut self) -> Result<(), PipelineProtocolError> {
        let Some(staged) = self.staged_reload.as_ref().cloned() else {
            let barrier = self.active_barrier.as_ref().expect("barrier checked").id;
            self.close_barrier_and_resolve_previews(
                barrier,
                AbandonedPreviewResolutionReason::DiscardedForAuthorityChange,
            );
            return Ok(());
        };
        let (next_seeds, changed_targets) = staged.into_parts();
        let mut next_live_only_overlay = self.live_only_overlay.clone();
        next_live_only_overlay.reconcile(&changed_targets);
        let mut next_model = self.model.clone();
        next_model.reconcile(&next_seeds);
        let canonical = RuntimeUiWireState {
            model: next_model.clone(),
            passthrough: self.passthrough.clone(),
        };
        let needs_cleanup = matches!(self.file_status, RuntimeUiFileStatus::Supported)
            && canonical != *self.pipeline.acknowledged_wire();
        if needs_cleanup {
            self.pipeline.preflight_accept_replace()?;
        }

        self.staged_reload = None;
        self.seeds = next_seeds;
        self.live_only_overlay = next_live_only_overlay;
        self.model = next_model;
        self.rebuild_live_state();

        if needs_cleanup {
            self.pipeline
                .accept_replace(canonical, self.authority_epoch)
                .expect("preflighted external reconciliation cleanup must dispatch");
            if let Some(barrier) = &mut self.active_barrier {
                barrier.phase = ControllerBarrierPhase::Writing(
                    self.pipeline
                        .source_mutation_in_flight()
                        .expect("cleanup dispatched")
                        .id,
                );
            }
        } else {
            let barrier = self.active_barrier.as_ref().expect("barrier checked").id;
            self.close_barrier_and_resolve_previews(
                barrier,
                AbandonedPreviewResolutionReason::DiscardedForAuthorityChange,
            );
        }
        Ok(())
    }

    fn canonical_wire(&self) -> RuntimeUiWireState {
        RuntimeUiWireState {
            model: self.model.clone(),
            passthrough: self.passthrough.clone(),
        }
    }

    fn rebuild_live_state(&mut self) {
        self.live_state =
            RuntimeUiLiveState::rebuild(&self.seeds, &self.model, &self.live_only_overlay);
    }

    fn allocate_mutation_id(&self) -> Option<u64> {
        let current = self.next_mutation_id.get();
        self.next_mutation_id.set(current.checked_add(1)?);
        Some(current)
    }

    fn allocate_barrier_id(&mut self) -> Option<ControllerBarrierId> {
        let current = self.next_barrier_id;
        self.next_barrier_id = current.checked_add(1)?;
        Some(ControllerBarrierId(current))
    }

    fn unsupported_reset_confirmation_is_current(
        &self,
        confirmation: &UnsupportedResetConfirmation,
    ) -> bool {
        confirmation.controller == self.id
            && self.pending_unsupported_reset_confirmation.as_ref() == Some(confirmation)
            && self.pipeline.stable_source() == &confirmation.revision
            && matches!(
                self.file_status,
                RuntimeUiFileStatus::UnsupportedReadOnly { version }
                    if version == confirmation.observed_version
            )
    }

    fn capture_reset_authority(&self) -> SupportedResetAuthoritySnapshot {
        SupportedResetAuthoritySnapshot {
            source: self.pipeline.stable_source().clone(),
            file_status: self.file_status.clone(),
            model: self.model.clone(),
            passthrough: self.passthrough.clone(),
            seeds: self.seeds.clone(),
            live_state: self.live_state.clone(),
        }
    }

    fn capture_reset_authority_after_prerequisite(&mut self) {
        let snapshot = self.capture_reset_authority();
        if let Some(transaction) = &mut self.supported_reset {
            transaction.authority = SupportedResetAuthorityState::Captured(snapshot);
        }
    }

    fn refresh_reset_barrier_phase(&mut self) {
        let Some(transaction) = &self.supported_reset else {
            return;
        };
        let Some(barrier) = &mut self.active_barrier else {
            return;
        };
        if let Some(request) = self.pipeline.source_mutation_in_flight() {
            barrier.phase = if matches!(
                request.kind,
                SourceMutationKind::ResetSupported { .. }
                    | SourceMutationKind::ResetUnsupportedIfUnchanged { .. }
            ) {
                ControllerBarrierPhase::Writing(request.id)
            } else {
                ControllerBarrierPhase::WaitingForPrerequisite(request.id)
            };
        } else {
            let _ = transaction;
            barrier.phase = ControllerBarrierPhase::Inspecting;
        }
    }

    fn finish_supported_reset_success(
        &mut self,
        integrated: IntegratedSourceMutation,
    ) -> SubmitSourceMutationResult {
        let recovery_artifacts = match &integrated.result {
            SourceMutationResult::Applied {
                recovery_artifacts, ..
            } => recovery_artifacts.clone(),
            _ => unreachable!("reset success requires an applied result"),
        };
        let transaction = self
            .supported_reset
            .take()
            .expect("matching reset acknowledgement requires transaction");
        debug_assert_eq!(transaction.through, integrated.request.accepted_through);
        self.model.clear();
        self.passthrough = WirePassthrough::default();
        self.live_only_overlay.clear();
        if let Some(reload) = self.staged_reload.take() {
            self.seeds = reload.into_parts().0;
        }
        self.rebuild_live_state();
        self.pipeline
            .settle_held_superseded(&transaction.held_by_reset, transaction.through);
        self.authority_epoch = transaction.publish_epoch;
        self.file_status = RuntimeUiFileStatus::Missing;
        if let Err(error) = self.pipeline.resume_after_integration() {
            return SubmitSourceMutationResult::Rejected(error);
        }
        self.close_barrier_and_resolve_previews(
            transaction.barrier,
            AbandonedPreviewResolutionReason::DiscardedForAuthorityChange,
        );
        SubmitSourceMutationResult::ResetCompleted {
            barrier: transaction.barrier,
            published_epoch: transaction.publish_epoch,
            recovery_artifacts,
        }
    }

    fn enter_external_reconciliation(
        &mut self,
        active: RuntimeStateSourceObservation,
        recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
        path_effect: RuntimeStateObservedPathEffect,
    ) {
        self.external_reconciliation = Some(ExternalReconciliationEvidence {
            writer_observation: active,
            recovery_artifacts,
            path_effect,
        });
        self.pipeline.discard_pending_for_external_authority();
        if self.active_barrier.is_none() {
            let id = self
                .allocate_barrier_id()
                .expect("barrier id exhausted during external reconciliation");
            self.active_barrier = Some(ActiveControllerBarrier {
                id,
                operation: ControllerBarrierOperation::ExternalAuthorityReconciliation,
                phase: ControllerBarrierPhase::Reinspecting,
            });
        } else if let Some(barrier) = &mut self.active_barrier {
            barrier.operation = ControllerBarrierOperation::ExternalAuthorityReconciliation;
            barrier.phase = ControllerBarrierPhase::Reinspecting;
        }
        if let Some(transaction) = self.supported_reset.take() {
            self.pipeline
                .settle_held_external(&transaction.held_by_reset);
        }
    }

    fn settle_external_reconciliation_for_shutdown(&mut self) {
        let barrier = self.active_barrier.as_ref().and_then(|barrier| {
            (barrier.operation == ControllerBarrierOperation::ExternalAuthorityReconciliation
                && !self.pipeline.has_source_mutation_in_flight())
            .then_some(barrier.id)
        });
        let Some(barrier) = barrier else {
            return;
        };
        let resolution_reason = if self.external_reconciliation.is_some() {
            AbandonedPreviewResolutionReason::CancelledUnderRetainedAuthority
        } else {
            AbandonedPreviewResolutionReason::DiscardedForAuthorityChange
        };
        self.external_reconciliation = None;
        self.staged_reload = None;
        self.close_barrier_and_resolve_previews(barrier, resolution_reason);
        let _ = self.pipeline.resume_after_integration();
    }
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
