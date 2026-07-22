use super::integration::durability_satisfies_flush;
use super::*;

impl PersistencePipeline {
    pub(crate) fn request_flush(
        &mut self,
        through: AcceptedStateRevision,
    ) -> Result<FlushRequestId, PipelineProtocolError> {
        if self.shutdown_requested {
            return Err(PipelineProtocolError::ShuttingDown);
        }
        if through > self.next_accepted {
            return Err(PipelineProtocolError::FlushBeyondAccepted {
                requested: through,
                latest: self.next_accepted,
            });
        }
        let id = FlushRequestId(self.next_flush_id);
        self.next_flush_id = self
            .next_flush_id
            .checked_add(1)
            .ok_or(PipelineProtocolError::FlushIdExhausted)?;
        self.flushes.insert(id, None);
        self.pending.push_back(PendingStage::Flush { id, through });
        self.drive()?;
        Ok(id)
    }

    pub(crate) fn request_shutdown(&mut self) -> Result<(), PipelineProtocolError> {
        if self.shutdown_requested {
            return Ok(());
        }
        self.shutdown_requested = true;
        self.pending.push_back(PendingStage::Shutdown);
        self.drive()
    }

    pub(crate) fn take_outbound(&mut self) -> Option<SourceMutationRequest> {
        self.outbound.take()
    }

    pub(crate) fn resume_after_integration(&mut self) -> Result<(), PipelineProtocolError> {
        self.drive()
    }

    pub(crate) fn shutdown_complete(&self) -> bool {
        self.shutdown_complete
    }

    pub(crate) fn pending_replacements(&self) -> usize {
        self.pending
            .iter()
            .filter(|stage| matches!(stage, PendingStage::Replace(_)))
            .count()
    }

    pub(super) fn allocate_accepted_revision(
        &mut self,
    ) -> Result<AcceptedStateRevision, PipelineProtocolError> {
        let next = self
            .next_accepted
            .0
            .checked_add(1)
            .ok_or(PipelineProtocolError::RevisionExhausted)?;
        let revision = AcceptedStateRevision(next);
        self.next_accepted = revision;
        self.receipts.insert(revision, None);
        Ok(revision)
    }

    fn allocate_mutation_id(&mut self) -> Result<SourceMutationId, PipelineProtocolError> {
        let id = SourceMutationId(self.next_mutation_id);
        self.next_mutation_id = self
            .next_mutation_id
            .checked_add(1)
            .ok_or(PipelineProtocolError::MutationIdExhausted)?;
        Ok(id)
    }

    pub(super) fn dispatch(
        &mut self,
        accepted_through: AcceptedStateRevision,
        expected_epoch: u64,
        kind: SourceMutationKind,
        covered: Vec<AcceptedStateRevision>,
    ) -> Result<(), PipelineProtocolError> {
        if self.in_flight.is_some() || self.outbound.is_some() {
            return Err(PipelineProtocolError::MutationAlreadyInFlight);
        }
        let request = SourceMutationRequest {
            id: self.allocate_mutation_id()?,
            accepted_through,
            expected_source: self.stable_source.clone(),
            expected_epoch,
            kind,
        };
        self.outbound = Some(request.clone());
        self.in_flight = Some(InFlightSourceMutation { request, covered });
        Ok(())
    }

    pub(super) fn drive(&mut self) -> Result<(), PipelineProtocolError> {
        if self.in_flight.is_some() || self.outbound.is_some() {
            return Ok(());
        }

        loop {
            let Some(stage) = self.pending.pop_front() else {
                return Ok(());
            };
            match stage {
                PendingStage::Replace(stage) => {
                    let dispatched = self.dispatch(
                        stage.through,
                        stage.authority_epoch,
                        SourceMutationKind::Replace(stage.snapshot.clone()),
                        stage.covered.clone(),
                    );
                    if let Err(error) = dispatched {
                        self.pending.push_front(PendingStage::Replace(stage));
                        return Err(error);
                    }
                    return Ok(());
                }
                PendingStage::Flush { id, through } => {
                    let complete = through <= self.settled_through;
                    if !complete {
                        self.pending.push_front(PendingStage::Flush { id, through });
                        return Ok(());
                    }
                    let durable = self
                        .earliest_consumed_non_durable
                        .is_none_or(|revision| revision > through)
                        && self.receipts.range(..=through).all(|(_, outcome)| {
                            outcome.as_ref().is_some_and(durability_satisfies_flush)
                        });
                    self.flushes.insert(
                        id,
                        Some(if durable {
                            FlushOutcome::Settled {
                                source: self.stable_source.clone(),
                            }
                        } else {
                            FlushOutcome::Failed
                        }),
                    );
                }
                PendingStage::ResetSupported {
                    through,
                    publish_epoch,
                } => {
                    let dispatched = self.dispatch(
                        through,
                        publish_epoch - 1,
                        SourceMutationKind::ResetSupported { publish_epoch },
                        vec![through],
                    );
                    if let Err(error) = dispatched {
                        self.pending.push_front(PendingStage::ResetSupported {
                            through,
                            publish_epoch,
                        });
                        return Err(error);
                    }
                    return Ok(());
                }
                PendingStage::ResetUnsupportedIfUnchanged {
                    through,
                    publish_epoch,
                    confirmation_revision,
                } => {
                    let dispatched = self.dispatch(
                        through,
                        publish_epoch - 1,
                        SourceMutationKind::ResetUnsupportedIfUnchanged {
                            publish_epoch,
                            confirmation_revision: confirmation_revision.clone(),
                        },
                        vec![through],
                    );
                    if let Err(error) = dispatched {
                        self.pending
                            .push_front(PendingStage::ResetUnsupportedIfUnchanged {
                                through,
                                publish_epoch,
                                confirmation_revision,
                            });
                        return Err(error);
                    }
                    return Ok(());
                }
                PendingStage::Shutdown => {
                    self.shutdown_complete = true;
                    return Ok(());
                }
            }
        }
    }
}
