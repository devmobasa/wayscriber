use super::*;

impl PersistencePipeline {
    pub(crate) fn integrate(
        &mut self,
        result: SourceMutationResult,
    ) -> Result<IntegratedSourceMutation, PipelineProtocolError> {
        if self.outbound.is_some() {
            return Err(PipelineProtocolError::MutationNotDispatched);
        }
        let in_flight = self
            .in_flight
            .take()
            .ok_or(PipelineProtocolError::NoMutationInFlight)?;
        if result.id() != in_flight.request.id {
            let expected = in_flight.request.id;
            self.in_flight = Some(in_flight);
            return Err(PipelineProtocolError::WrongMutationId {
                expected,
                received: result.id(),
            });
        }
        if let SourceMutationResult::Applied {
            applied_through, ..
        } = &result
            && *applied_through != in_flight.request.accepted_through
        {
            let expected = in_flight.request.accepted_through;
            let received = *applied_through;
            self.in_flight = Some(in_flight);
            return Err(PipelineProtocolError::WrongAppliedRevision { expected, received });
        }
        if let SourceMutationResult::Applied { new_source, .. } = &result
            && new_source.path_identity() != in_flight.request.expected_source.path_identity()
        {
            self.in_flight = Some(in_flight);
            return Err(PipelineProtocolError::AppliedSourcePathMismatch);
        }
        if let SourceMutationResult::Applied { new_source, .. } = &result
            && matches!(&in_flight.request.kind, SourceMutationKind::Replace(_))
            && new_source.bytes().is_none()
        {
            self.in_flight = Some(in_flight);
            return Err(PipelineProtocolError::ReplaceDidNotProducePresentSource);
        }
        if let SourceMutationResult::Applied { new_source, .. } = &result
            && matches!(
                &in_flight.request.kind,
                SourceMutationKind::ResetSupported { .. }
                    | SourceMutationKind::ResetUnsupportedIfUnchanged { .. }
            )
            && new_source.bytes().is_some()
        {
            self.in_flight = Some(in_flight);
            return Err(PipelineProtocolError::ResetDidNotProduceMissingSource);
        }
        if let Err(error) = validate_source_mutation_evidence(&result) {
            self.in_flight = Some(in_flight);
            return Err(error);
        }
        if let SourceMutationResult::SourceChangedBeforeMutation { active, .. } = &result
            && active.revision == in_flight.request.expected_source
        {
            self.in_flight = Some(in_flight);
            return Err(PipelineProtocolError::ConflictMatchedExpectedSource);
        }

        match &result {
            SourceMutationResult::Applied { new_source, .. } => {
                self.stable_source = new_source.clone();
                if let SourceMutationKind::Replace(snapshot) = &in_flight.request.kind {
                    self.acknowledged_wire = snapshot.clone();
                } else {
                    self.acknowledged_wire = RuntimeUiWireState::default();
                }
                for revision in &in_flight.covered {
                    self.settle_receipt(
                        *revision,
                        DurabilityOutcome::Persisted {
                            source: new_source.clone(),
                        },
                    );
                }
            }
            SourceMutationResult::SourceChangedBeforeMutation { .. } => {
                for revision in &in_flight.covered {
                    self.settle_receipt(*revision, DurabilityOutcome::ExternalSourceWon);
                }
            }
            SourceMutationResult::ObservationChangedAfterClaim {
                active,
                recovery_artifacts,
                path_effect,
                ..
            } => {
                for revision in &in_flight.covered {
                    self.settle_receipt(
                        *revision,
                        DurabilityOutcome::ObservationChangedAfterClaim {
                            active: active.clone(),
                            recovery_artifacts: recovery_artifacts.clone(),
                            path_effect: path_effect.clone(),
                        },
                    );
                }
            }
            SourceMutationResult::Failed { .. } => {
                // Failure ownership transfers to the controller. A failed
                // replacement may still be retried by an unhealthy incident,
                // while a failed reset is terminal. Settling either case here
                // would lose that distinction.
            }
        }
        self.advance_settled_through();

        Ok(IntegratedSourceMutation {
            request: in_flight.request,
            covered: in_flight.covered,
            result,
        })
    }

    #[cfg(test)]
    pub(crate) fn receipt(&self, revision: AcceptedStateRevision) -> Option<&DurabilityOutcome> {
        self.receipts.get(&revision).and_then(Option::as_ref)
    }

    /// Consume a terminal durability outcome after it has been delivered.
    /// Pending receipts remain registered and return `None`.
    pub(crate) fn take_receipt(
        &mut self,
        revision: AcceptedStateRevision,
    ) -> Option<DurabilityOutcome> {
        if !self.receipts.get(&revision).is_some_and(Option::is_some) {
            return None;
        }
        let outcome = self
            .receipts
            .remove(&revision)
            .and_then(|outcome| outcome)
            .expect("terminal receipt was checked above");
        if revision > self.settled_through {
            self.consumed_terminal_receipts.insert(revision);
        }
        if !durability_satisfies_flush(&outcome) {
            self.earliest_consumed_non_durable = Some(
                self.earliest_consumed_non_durable
                    .map_or(revision, |earliest| earliest.min(revision)),
            );
        }
        Some(outcome)
    }

    pub(crate) fn settle_failed(
        &mut self,
        revisions: impl IntoIterator<Item = AcceptedStateRevision>,
        error: RuntimeStateIoError,
    ) {
        for revision in revisions {
            self.settle_receipt(revision, DurabilityOutcome::Failed(error.clone()));
        }
        self.advance_settled_through();
    }

    pub(crate) fn settle_persisted(
        &mut self,
        revisions: impl IntoIterator<Item = AcceptedStateRevision>,
        source: RuntimeStateSourceRevision,
    ) {
        for revision in revisions {
            self.settle_receipt(
                revision,
                DurabilityOutcome::Persisted {
                    source: source.clone(),
                },
            );
        }
        self.advance_settled_through();
    }

    pub(crate) fn settle_external(
        &mut self,
        revisions: impl IntoIterator<Item = AcceptedStateRevision>,
    ) {
        for revision in revisions {
            self.settle_receipt(revision, DurabilityOutcome::ExternalSourceWon);
        }
        self.advance_settled_through();
    }

    pub(crate) fn settle_held_failed(
        &mut self,
        stages: &[HeldReplacementStage],
        error: RuntimeStateIoError,
    ) {
        self.settle_failed(
            stages
                .iter()
                .flat_map(|stage| stage.covered.iter().copied()),
            error,
        );
    }

    #[cfg(test)]
    pub(crate) fn flush_outcome(&self, id: FlushRequestId) -> Option<&FlushOutcome> {
        self.flushes.get(&id).and_then(Option::as_ref)
    }

    /// Consume a terminal flush outcome after it has been delivered.
    /// Pending flushes remain registered and return `None`.
    pub(crate) fn take_flush_outcome(&mut self, id: FlushRequestId) -> Option<FlushOutcome> {
        if !self.flushes.get(&id).is_some_and(Option::is_some) {
            return None;
        }
        self.flushes.remove(&id).and_then(|outcome| outcome)
    }

    pub(super) fn settle_receipt(
        &mut self,
        revision: AcceptedStateRevision,
        outcome: DurabilityOutcome,
    ) {
        let Some(slot) = self.receipts.get_mut(&revision) else {
            debug_assert!(
                revision <= self.settled_through
                    || self.consumed_terminal_receipts.contains(&revision),
                "controller may settle only an allocated revision"
            );
            return;
        };
        debug_assert!(slot.is_none(), "durability receipts are terminal");
        if slot.is_none() {
            *slot = Some(outcome);
        }
    }

    pub(super) fn advance_settled_through(&mut self) {
        while let Some(next) = self.settled_through.0.checked_add(1) {
            let revision = AcceptedStateRevision(next);
            let terminal = self.receipts.get(&revision).is_some_and(Option::is_some)
                || self.consumed_terminal_receipts.remove(&revision);
            if terminal {
                self.settled_through = revision;
            } else {
                break;
            }
        }
    }
}

pub(super) fn durability_satisfies_flush(outcome: &DurabilityOutcome) -> bool {
    matches!(
        outcome,
        DurabilityOutcome::Persisted { .. } | DurabilityOutcome::SupersededByReset { .. }
    )
}

pub(crate) fn validate_source_mutation_evidence(
    result: &SourceMutationResult,
) -> Result<(), PipelineProtocolError> {
    let (active, recovery_artifacts, path_effect) = match result {
        SourceMutationResult::Applied {
            recovery_artifacts, ..
        } => (None, recovery_artifacts.as_slice(), None),
        SourceMutationResult::SourceChangedBeforeMutation { active, .. } => {
            if !active.is_consistent() {
                return Err(PipelineProtocolError::InconsistentSourceObservation);
            }
            (Some(active), &[][..], None)
        }
        SourceMutationResult::ObservationChangedAfterClaim {
            active,
            recovery_artifacts,
            path_effect,
            ..
        } => {
            if !active.is_consistent() {
                return Err(PipelineProtocolError::InconsistentSourceObservation);
            }
            (
                Some(active),
                recovery_artifacts.as_slice(),
                Some(path_effect),
            )
        }
        SourceMutationResult::Failed {
            active,
            recovery_artifacts,
            path_effect,
            ..
        } => {
            if active
                .as_ref()
                .is_some_and(|active| !active.is_consistent())
            {
                return Err(PipelineProtocolError::InconsistentSourceObservation);
            }
            if matches!(
                path_effect,
                RuntimeStateFailurePathEffect::Known(RuntimeStateObservedPathEffect::Untouched)
            ) && !recovery_artifacts.is_empty()
            {
                return Err(PipelineProtocolError::UntouchedResultReportedRecoveryArtifacts);
            }
            let post_claim = match path_effect {
                RuntimeStateFailurePathEffect::Known(
                    RuntimeStateObservedPathEffect::PostClaim(effect),
                ) => Some(effect),
                RuntimeStateFailurePathEffect::Known(RuntimeStateObservedPathEffect::Untouched)
                | RuntimeStateFailurePathEffect::UnknownAfterMutation => None,
            };
            (active.as_ref(), recovery_artifacts.as_slice(), post_claim)
        }
    };

    let mut paths = BTreeSet::new();
    for artifact in recovery_artifacts {
        if !artifact.observation.is_consistent() {
            return Err(PipelineProtocolError::InconsistentSourceObservation);
        }
        if !paths.insert(&artifact.path) {
            return Err(PipelineProtocolError::DuplicateRecoveryArtifactPath);
        }
    }

    match path_effect {
        Some(RuntimeStatePostClaimPathEffect::QuarantinedThenRestored { restored_source }) => {
            if active.map(|active| &active.revision) != Some(restored_source) {
                return Err(PipelineProtocolError::ContradictoryPostClaimObservation);
            }
            if recovery_artifacts
                .iter()
                .any(|artifact| artifact.path == restored_source.path_identity().source_path())
            {
                return Err(PipelineProtocolError::ContradictoryPostClaimObservation);
            }
        }
        Some(RuntimeStatePostClaimPathEffect::QuarantinedAndRetained { recovery_path })
            if !recovery_artifacts
                .iter()
                .any(|artifact| &artifact.path == recovery_path) =>
        {
            return Err(PipelineProtocolError::RetainedRecoveryArtifactMissing);
        }
        Some(RuntimeStatePostClaimPathEffect::QuarantinedAndRetained { .. }) => {}
        None => {}
    }
    Ok(())
}
