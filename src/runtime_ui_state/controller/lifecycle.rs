use super::*;

impl RuntimeUiStateController {
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

    pub(crate) fn shutdown_complete(&self) -> bool {
        self.pipeline.shutdown_complete()
    }

    pub(crate) fn drain_lifecycle_controls(&mut self) {
        while let Ok(control) = self.lifecycle_rx.try_recv() {
            self.apply_lifecycle_control(control);
        }
    }

    pub(super) fn apply_seed_update(
        &mut self,
        seeds: ValidatedInteractionSeeds,
    ) -> UpdateSeedsResult {
        let mut next_seeds = self.seeds.clone();
        let changed_targets = match next_seeds.update(seeds) {
            Ok(changed) => changed,
            Err(error) => return UpdateSeedsResult::Rejected(error),
        };
        let mut next_live_only_overlay = self.live_only_overlay.clone();
        next_live_only_overlay.reconcile(&changed_targets);
        let mut next_model = self.model.clone();
        let mut next_passthrough = self.passthrough.clone();
        let pruned =
            next_model.reconcile(&next_seeds) | next_passthrough.reconcile_entries(&next_model);
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
        self.passthrough = next_passthrough;
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

    pub(super) fn finish_external_reconciliation_write(
        &mut self,
    ) -> Result<(), PipelineProtocolError> {
        let Some(staged) = self.staged_reload.as_ref().cloned() else {
            let barrier = self.active_barrier.as_ref().expect("barrier checked").id;
            self.close_barrier_and_resolve_previews(
                barrier,
                AbandonedPreviewResolutionReason::DiscardedForAuthorityChange,
            );
            return Ok(());
        };
        let (next_seeds, changed_targets, tombstoned_targets) = staged.into_parts();
        let mut next_live_only_overlay = self.live_only_overlay.clone();
        next_live_only_overlay.reconcile(&changed_targets);
        let mut next_model = self.model.clone();
        next_model.remove_targets(&tombstoned_targets);
        next_model.reconcile(&next_seeds);
        let mut next_passthrough = self.passthrough.clone();
        next_passthrough.reconcile_entries(&next_model);
        let canonical = RuntimeUiWireState {
            model: next_model.clone(),
            passthrough: next_passthrough.clone(),
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
        self.passthrough = next_passthrough;
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
}
