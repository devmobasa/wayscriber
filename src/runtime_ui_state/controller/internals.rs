use super::*;

impl RuntimeUiStateController {
    pub(super) fn canonical_wire(&self) -> RuntimeUiWireState {
        RuntimeUiWireState {
            model: self.model.clone(),
            passthrough: self.passthrough.clone(),
        }
    }

    pub(super) fn rebuild_live_state(&mut self) {
        self.live_state =
            RuntimeUiLiveState::rebuild(&self.seeds, &self.model, &self.live_only_overlay);
    }

    pub(super) fn allocate_mutation_id(&self) -> Option<u64> {
        let current = self.next_mutation_id.get();
        self.next_mutation_id.set(current.checked_add(1)?);
        Some(current)
    }

    pub(super) fn allocate_barrier_id(&mut self) -> Option<ControllerBarrierId> {
        let current = self.next_barrier_id;
        self.next_barrier_id = current.checked_add(1)?;
        Some(ControllerBarrierId(current))
    }

    pub(super) fn unsupported_reset_confirmation_is_current(
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

    pub(super) fn capture_reset_authority(&self) -> SupportedResetAuthoritySnapshot {
        SupportedResetAuthoritySnapshot {
            source: self.pipeline.stable_source().clone(),
            file_status: self.file_status.clone(),
            model: self.model.clone(),
            passthrough: self.passthrough.clone(),
            seeds: self.seeds.clone(),
            live_state: self.live_state.clone(),
        }
    }

    pub(super) fn capture_reset_authority_after_prerequisite(&mut self) {
        let snapshot = self.capture_reset_authority();
        if let Some(transaction) = &mut self.supported_reset {
            transaction.authority = SupportedResetAuthorityState::Captured(Box::new(snapshot));
        }
    }

    pub(super) fn refresh_reset_barrier_phase(&mut self) {
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

    pub(super) fn finish_supported_reset_success(
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

    pub(super) fn enter_external_reconciliation(
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

    pub(super) fn settle_external_reconciliation_for_shutdown(&mut self) {
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
