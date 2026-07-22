use super::*;

impl PersistencePipeline {
    pub(crate) fn accept_replace(
        &mut self,
        snapshot: RuntimeUiWireState,
        authority_epoch: u64,
    ) -> Result<AcceptedStateRevision, PipelineProtocolError> {
        self.preflight_accept_replace()?;
        let revision = self.allocate_accepted_revision()?;
        let replacement = PendingReplacement {
            snapshot,
            through: revision,
            covered: vec![revision],
            authority_epoch,
        };

        match self.pending.back_mut() {
            Some(PendingStage::Replace(tail)) if tail.authority_epoch == authority_epoch => {
                tail.snapshot = replacement.snapshot;
                tail.through = revision;
                tail.covered.push(revision);
            }
            _ => self.pending.push_back(PendingStage::Replace(replacement)),
        }
        self.drive()?;
        Ok(revision)
    }

    pub(crate) fn preflight_accept_replace(&self) -> Result<(), PipelineProtocolError> {
        if self.shutdown_requested {
            return Err(PipelineProtocolError::ShuttingDown);
        }
        self.next_accepted
            .0
            .checked_add(1)
            .ok_or(PipelineProtocolError::RevisionExhausted)?;
        self.next_mutation_id
            .checked_add(1)
            .ok_or(PipelineProtocolError::MutationIdExhausted)?;
        Ok(())
    }

    pub(crate) fn allocate_reset_revision(
        &mut self,
    ) -> Result<AcceptedStateRevision, PipelineProtocolError> {
        if self.shutdown_requested {
            return Err(PipelineProtocolError::ShuttingDown);
        }
        self.allocate_accepted_revision()
    }

    pub(crate) fn reserve_revision(
        &mut self,
    ) -> Result<AcceptedStateRevision, PipelineProtocolError> {
        if self.shutdown_requested {
            return Err(PipelineProtocolError::ShuttingDown);
        }
        self.allocate_accepted_revision()
    }

    pub(crate) fn stage_supported_reset(
        &mut self,
        through: AcceptedStateRevision,
        publish_epoch: u64,
    ) -> Result<(), PipelineProtocolError> {
        if self.shutdown_requested {
            return Err(PipelineProtocolError::ShuttingDown);
        }
        if publish_epoch == 0 {
            return Err(PipelineProtocolError::InvalidPublishEpoch);
        }
        self.pending.push_back(PendingStage::ResetSupported {
            through,
            publish_epoch,
        });
        self.drive()
    }

    pub(crate) fn stage_unsupported_reset(
        &mut self,
        through: AcceptedStateRevision,
        publish_epoch: u64,
        confirmation_revision: RuntimeStateSourceRevision,
    ) -> Result<(), PipelineProtocolError> {
        if self.shutdown_requested {
            return Err(PipelineProtocolError::ShuttingDown);
        }
        if publish_epoch == 0 {
            return Err(PipelineProtocolError::InvalidPublishEpoch);
        }
        self.pending
            .push_back(PendingStage::ResetUnsupportedIfUnchanged {
                through,
                publish_epoch,
                confirmation_revision,
            });
        self.drive()
    }

    pub(crate) fn hold_trailing_replacements(&mut self) -> Vec<HeldReplacementStage> {
        let mut held = Vec::new();
        while matches!(self.pending.back(), Some(PendingStage::Replace(_))) {
            let PendingStage::Replace(stage) = self.pending.pop_back().expect("tail checked")
            else {
                unreachable!();
            };
            held.push(stage.into());
        }
        held.reverse();
        held
    }

    pub(crate) fn hold_all_pending_replacements(&mut self) -> Vec<HeldReplacementStage> {
        let mut held = Vec::new();
        let mut retained = VecDeque::new();
        while let Some(stage) = self.pending.pop_front() {
            match stage {
                PendingStage::Replace(stage) => held.push(stage.into()),
                stage => retained.push_back(stage),
            }
        }
        self.pending = retained;
        held
    }

    pub(crate) fn restore_held_replacements(
        &mut self,
        stages: Vec<HeldReplacementStage>,
    ) -> Result<(), PipelineProtocolError> {
        if !stages.is_empty() {
            self.preflight_accept_replace()?;
        }
        for stage in stages {
            self.pending.push_back(PendingStage::Replace(stage.into()));
        }
        self.drive()
    }

    pub(crate) fn settle_held_superseded(
        &mut self,
        stages: &[HeldReplacementStage],
        reset_through: AcceptedStateRevision,
    ) {
        for revision in stages.iter().flat_map(|stage| &stage.covered) {
            self.settle_receipt(
                *revision,
                DurabilityOutcome::SupersededByReset { reset_through },
            );
        }
        self.advance_settled_through();
    }

    pub(crate) fn settle_held_external(&mut self, stages: &[HeldReplacementStage]) {
        for revision in stages.iter().flat_map(|stage| &stage.covered) {
            self.settle_receipt(*revision, DurabilityOutcome::ExternalSourceWon);
        }
        self.advance_settled_through();
    }

    pub(crate) fn settle_held_persisted(
        &mut self,
        stages: &[HeldReplacementStage],
        source: &RuntimeStateSourceRevision,
    ) {
        for revision in stages.iter().flat_map(|stage| &stage.covered) {
            self.settle_receipt(
                *revision,
                DurabilityOutcome::Persisted {
                    source: source.clone(),
                },
            );
        }
        self.advance_settled_through();
    }

    pub(crate) fn cancel_pending_reset(
        &mut self,
        through: AcceptedStateRevision,
        outcome: DurabilityOutcome,
    ) -> bool {
        let Some(index) = self.pending.iter().position(|stage| {
            matches!(
                stage,
                PendingStage::ResetSupported {
                    through: staged, ..
                } | PendingStage::ResetUnsupportedIfUnchanged {
                    through: staged, ..
                } if *staged == through
            )
        }) else {
            return false;
        };
        self.pending.remove(index);
        self.settle_receipt(through, outcome);
        self.advance_settled_through();
        true
    }

    pub(crate) fn dispatch_recovery_replace(
        &mut self,
        snapshot: RuntimeUiWireState,
        through: AcceptedStateRevision,
        covered: Vec<AcceptedStateRevision>,
        authority_epoch: u64,
    ) -> Result<SourceMutationRequest, PipelineProtocolError> {
        self.preflight_recovery_replace()?;
        self.dispatch(
            through,
            authority_epoch,
            SourceMutationKind::Replace(snapshot),
            covered,
        )?;
        Ok(self
            .take_outbound()
            .expect("recovery replacement dispatched synchronously"))
    }

    pub(crate) fn preflight_recovery_replace(&self) -> Result<(), PipelineProtocolError> {
        if self.shutdown_requested {
            return Err(PipelineProtocolError::ShuttingDown);
        }
        if self.in_flight.is_some() || self.outbound.is_some() {
            return Err(PipelineProtocolError::MutationAlreadyInFlight);
        }
        self.next_mutation_id
            .checked_add(1)
            .ok_or(PipelineProtocolError::MutationIdExhausted)?;
        Ok(())
    }

    pub(crate) fn install_acknowledged_authority(
        &mut self,
        source: RuntimeStateSourceRevision,
        wire: RuntimeUiWireState,
    ) {
        debug_assert!(self.in_flight.is_none());
        self.stable_source = source;
        self.acknowledged_wire = wire;
    }

    pub(crate) fn discard_pending_for_external_authority(&mut self) {
        let mut retain_shutdown = false;
        while let Some(stage) = self.pending.pop_front() {
            match stage {
                PendingStage::Replace(stage) => {
                    for revision in stage.covered {
                        self.settle_receipt(revision, DurabilityOutcome::ExternalSourceWon);
                    }
                }
                PendingStage::Flush { id, .. } => {
                    self.flushes.insert(id, Some(FlushOutcome::Failed));
                }
                PendingStage::ResetSupported { through, .. }
                | PendingStage::ResetUnsupportedIfUnchanged { through, .. } => {
                    self.settle_receipt(through, DurabilityOutcome::ExternalSourceWon);
                }
                PendingStage::Shutdown => retain_shutdown = true,
            }
        }
        if retain_shutdown {
            self.pending.push_back(PendingStage::Shutdown);
        }
        self.advance_settled_through();
    }
}
