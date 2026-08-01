use super::*;

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
                || (acknowledged.model.is_empty() && acknowledged.passthrough.is_empty()),
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
        let mut passthrough = acknowledged.passthrough.clone();
        let needs_cleanup = model.reconcile(&seeds) | passthrough.reconcile_entries(&model);
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

    pub(crate) fn file_status(&self) -> &RuntimeUiFileStatus {
        &self.file_status
    }

    pub(crate) fn active_barrier(&self) -> Option<&ActiveControllerBarrier> {
        self.active_barrier.as_ref()
    }

    /// Whether writer work is queued or in flight that can advance the
    /// active barrier: a pipeline source mutation, or an active recovery
    /// attempt (whose current command is queued in the recovery outbox or
    /// already with the writer). False for resting states that only a
    /// controller-side decision can advance — an unhealthy incident with
    /// no attempt running waits on the user, not the writer.
    pub(crate) fn barrier_settling_work_in_flight(&self) -> bool {
        self.pipeline.has_source_mutation_in_flight()
            || !self.recovery_outbox.is_empty()
            || self.active_recovery.is_some()
    }

    pub(crate) fn take_preview_resolutions(&mut self) -> Vec<AbandonedPreviewResolution> {
        std::mem::take(&mut self.preview_resolution_outbox)
    }

    pub(in crate::runtime_ui_state) fn close_barrier_and_resolve_previews(
        &mut self,
        barrier: ControllerBarrierId,
        reason: AbandonedPreviewResolutionReason,
    ) {
        self.close_barrier_and_resolve_previews_after_seed_changes(barrier, reason, None);
    }

    pub(in crate::runtime_ui_state) fn close_barrier_and_resolve_previews_after_seed_changes(
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

    pub(in crate::runtime_ui_state) fn resolve_previews_while_barrier_retained(
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
}
