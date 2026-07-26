use super::*;

impl RuntimeUiStateController {
    pub(super) fn validate_active_recovery_state(&self) -> Result<(), RecoveryStateFailure> {
        let active = self
            .active_recovery
            .as_ref()
            .ok_or(RecoveryStateFailure::MissingActiveAttempt)?;
        let incident = self
            .incident
            .as_ref()
            .ok_or(RecoveryStateFailure::MissingIncident)?;
        if incident.id != active.incident {
            return Err(RecoveryStateFailure::IncidentMismatch);
        }
        if incident.handle.availability != RecoveryHandleAvailability::InAttempt(active.id) {
            return Err(RecoveryStateFailure::HandleStateMismatch);
        }
        let barrier = self
            .active_barrier
            .as_ref()
            .ok_or(RecoveryStateFailure::MissingBarrier)?;
        if barrier.id != active.barrier || barrier.id != incident.barrier {
            return Err(RecoveryStateFailure::BarrierMismatch);
        }
        Ok(())
    }

    pub(super) fn block_protocol_failure(
        &mut self,
        reason: RecoveryCompletionProtocolError,
        completion: RecoveryIoCompletion,
    ) -> SubmitPersistenceRecoveryResult {
        if let Err(error) = self.validate_active_recovery_state() {
            return self.finish_recovery_state_failure(error);
        }
        self.record_rejected_recovery_completion(completion.clone());
        let source_mutation_kind = self.active_recovery.as_ref().and_then(|active| {
            match &active.current_command.expected {
                RecoveryCommandExpectation::SourceMutation { kind, .. } => Some(kind.clone()),
                RecoveryCommandExpectation::Inspection => None,
            }
        });
        let source_mutation_in_flight = source_mutation_kind.is_some();
        let Some(incident_state) = self.incident.as_ref() else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::MissingIncident);
        };
        let incident = incident_state.id;
        let Some(active_identity) = self.active_recovery.as_ref().map(|active| {
            (
                active.id,
                active.current_command.id,
                active.incident,
                active.barrier,
            )
        }) else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::MissingActiveAttempt);
        };
        if active_identity.2 != incident {
            return self.finish_recovery_state_failure(RecoveryStateFailure::IncidentMismatch);
        }
        let owns_active_command = completion.incident == active_identity.2
            && completion.barrier == active_identity.3
            && completion.attempt == active_identity.0
            && completion.command_id == active_identity.1;
        if let RecoveryIoResult::SourceMutation(result) = &completion.result {
            let retained = if owns_active_command
                && matches!(
                    &source_mutation_kind,
                    Some(RecoverySourceMutationKind::PreserveInvalid)
                ) {
                self.retain_recovery_mutation_evidence_for_kind(
                    result,
                    &RecoverySourceMutationKind::PreserveInvalid,
                )
            } else {
                self.retain_recovery_mutation_evidence(result)
            };
            if let Err(error) = retained {
                return self.finish_recovery_state_failure(error);
            }
        }
        let attempt = active_identity.0;
        if source_mutation_in_flight {
            let Some(incident_state) = &mut self.incident else {
                return self.finish_recovery_state_failure(RecoveryStateFailure::MissingIncident);
            };
            if !matches!(
                incident_state.path_effect_history.last(),
                Some(RuntimeStateFailurePathEffect::UnknownAfterMutation)
            ) {
                incident_state
                    .path_effect_history
                    .push(RuntimeStateFailurePathEffect::UnknownAfterMutation);
            }
            if owns_active_command {
                let active_observation = match &completion.result {
                    RecoveryIoResult::SourceMutation(result) => {
                        source_mutation_observation_for_protocol_error(
                            result,
                            if matches!(
                                &source_mutation_kind,
                                Some(RecoverySourceMutationKind::PreserveInvalid)
                            ) {
                                RuntimeStateObservedEnvelope::PresentWithoutReadableVersion
                            } else {
                                RuntimeStateObservedEnvelope::Version(1)
                            },
                        )
                    }
                    RecoveryIoResult::Inspected(_) => None,
                };
                self.record_integrated_recovery_command(active_identity.1);
                if !matches!(
                    &source_mutation_kind,
                    Some(RecoverySourceMutationKind::PreserveInvalid)
                ) {
                    let integration_error = match &completion.result {
                        RecoveryIoResult::SourceMutation(result) => {
                            match self.pipeline.preflight_integrate(result) {
                                Ok(()) => PipelineProtocolError::UnexpectedMutationResult,
                                Err(error) => error,
                            }
                        }
                        RecoveryIoResult::Inspected(_) => {
                            PipelineProtocolError::UnexpectedMutationResult
                        }
                    };
                    if let Err(error) =
                        self.abandon_recovery_source_mutation_for_reinspection(integration_error)
                    {
                        return self.finish_recovery_state_failure(error);
                    }
                }
                if self.shutting_down {
                    return self.finish_recovery_shutdown();
                }
                if self
                    .active_recovery
                    .as_ref()
                    .is_some_and(|active| active.cancel_requested)
                {
                    return self.finish_current_attempt_cancelled(active_observation);
                }
                let command_id = match self.dispatch_protocol_failure_reinspection() {
                    Ok(command) => command,
                    Err(error) => return self.finish_recovery_state_failure(error),
                };
                let Some(incident_state) = self.incident.as_ref() else {
                    return self
                        .finish_recovery_state_failure(RecoveryStateFailure::MissingIncident);
                };
                return SubmitPersistenceRecoveryResult::BlockedProtocolFailure {
                    reason,
                    evidence: evidence(incident_state),
                    reinspection_dispatched: Some(command_id),
                };
            }
            let command = active_identity.1;
            let Some(active) = self.active_recovery.as_mut() else {
                return self
                    .finish_recovery_state_failure(RecoveryStateFailure::MissingActiveAttempt);
            };
            active.protocol_failure_pending = true;
            let Some(active_barrier) = &mut self.active_barrier else {
                return self.finish_recovery_state_failure(RecoveryStateFailure::MissingBarrier);
            };
            active_barrier.phase = ControllerBarrierPhase::Recovering {
                incident,
                attempt,
                step: RecoveryAttemptStep::ProtocolFailureAwaitingSourceMutation(command),
            };
            let Some(incident_state) = self.incident.as_ref() else {
                return self.finish_recovery_state_failure(RecoveryStateFailure::MissingIncident);
            };
            return SubmitPersistenceRecoveryResult::BlockedProtocolFailure {
                reason,
                evidence: evidence(incident_state),
                reinspection_dispatched: None,
            };
        }

        let active_command = active_identity.1;
        self.record_integrated_recovery_command(active_command);
        let command_id = match self.dispatch_protocol_failure_reinspection() {
            Ok(command) => command,
            Err(error) => return self.finish_recovery_state_failure(error),
        };
        let Some(incident_state) = self.incident.as_ref() else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::MissingIncident);
        };
        SubmitPersistenceRecoveryResult::BlockedProtocolFailure {
            reason,
            evidence: evidence(incident_state),
            reinspection_dispatched: Some(command_id),
        }
    }

    pub(super) fn dispatch_protocol_failure_reinspection(
        &mut self,
    ) -> Result<RecoveryCommandId, RecoveryStateFailure> {
        self.validate_active_recovery_state()?;
        let command_id = allocate_counter(&mut self.next_recovery_command_id)
            .map(RecoveryCommandId)
            .ok_or(RecoveryStateFailure::CommandIdExhausted)?;
        let active = self
            .active_recovery
            .as_mut()
            .ok_or(RecoveryStateFailure::MissingActiveAttempt)?;
        active.protocol_failure_pending = false;
        active.kind = RecoveryAttemptKind::ProtocolFailureReinspection;
        active.current_command = ActiveRecoveryCommand {
            id: command_id,
            expected: RecoveryCommandExpectation::Inspection,
        };
        self.recovery_outbox.push_back(RecoveryIoCommand {
            controller_id: self.id,
            incident: active.incident,
            barrier: active.barrier,
            attempt: active.id,
            command_id,
            operation: RecoveryIoOperation::Inspect,
        });
        let barrier = self
            .active_barrier
            .as_mut()
            .ok_or(RecoveryStateFailure::MissingBarrier)?;
        barrier.phase = ControllerBarrierPhase::Recovering {
            incident: active.incident,
            attempt: active.id,
            step: RecoveryAttemptStep::Inspecting,
        };
        Ok(command_id)
    }

    pub(super) fn retain_recovery_mutation_evidence(
        &mut self,
        result: &SourceMutationResult,
    ) -> Result<(), RecoveryStateFailure> {
        let completion_evidence = self.recovery_mutation_evidence(result, None);
        self.retain_recovery_evidence(completion_evidence)
    }

    pub(super) fn retain_recovery_mutation_evidence_for_kind(
        &mut self,
        result: &SourceMutationResult,
        kind: &RecoverySourceMutationKind,
    ) -> Result<(), RecoveryStateFailure> {
        let completion_evidence = self.recovery_mutation_evidence(result, Some(kind));
        self.retain_recovery_evidence(completion_evidence)
    }

    pub(super) fn recovery_mutation_evidence(
        &self,
        result: &SourceMutationResult,
        kind: Option<&RecoverySourceMutationKind>,
    ) -> PersistenceRecoveryEvidence {
        let retain_path_effect = validate_source_mutation_evidence(result).is_ok();
        let recovery_artifacts = match result {
            SourceMutationResult::Applied {
                recovery_artifacts, ..
            }
            | SourceMutationResult::ObservationChangedAfterClaim {
                recovery_artifacts, ..
            }
            | SourceMutationResult::Failed {
                recovery_artifacts, ..
            } => recovery_artifacts.clone(),
            SourceMutationResult::SourceChangedBeforeMutation { .. } => Vec::new(),
        };
        let mut path_effect_history = Vec::new();
        if retain_path_effect {
            match result {
                SourceMutationResult::Applied {
                    recovery_artifacts, ..
                } if matches!(kind, Some(RecoverySourceMutationKind::PreserveInvalid)) => {
                    let confirmation =
                        self.active_recovery
                            .as_ref()
                            .and_then(|active| match &active.kind {
                                RecoveryAttemptKind::ConfirmPreserveInvalidResetInFlight {
                                    confirmation,
                                } => Some(confirmation),
                                _ => None,
                            });
                    if let Some(recovery_path) = confirmation.and_then(|confirmation| {
                        recovery_artifacts
                            .iter()
                            .find(|artifact| {
                                artifact.observation.envelope == confirmation.envelope
                                    && artifact.observation.revision == confirmation.revision
                            })
                            .map(|artifact| artifact.path.clone())
                    }) {
                        path_effect_history.push(RuntimeStateFailurePathEffect::Known(
                            RuntimeStateObservedPathEffect::PostClaim(
                                RuntimeStatePostClaimPathEffect::QuarantinedAndRetained {
                                    recovery_path,
                                },
                            ),
                        ));
                    }
                }
                SourceMutationResult::Applied { .. }
                | SourceMutationResult::SourceChangedBeforeMutation { .. } => {}
                SourceMutationResult::ObservationChangedAfterClaim { path_effect, .. } => {
                    path_effect_history.push(RuntimeStateFailurePathEffect::Known(
                        RuntimeStateObservedPathEffect::PostClaim(path_effect.clone()),
                    ));
                }
                SourceMutationResult::Failed { path_effect, .. } => {
                    path_effect_history.push(path_effect.clone());
                }
            }
        }
        PersistenceRecoveryEvidence {
            recovery_artifacts,
            path_effect_history,
        }
    }

    pub(super) fn retain_recovery_evidence(
        &mut self,
        completion_evidence: PersistenceRecoveryEvidence,
    ) -> Result<(), RecoveryStateFailure> {
        let Some(incident) = &mut self.incident else {
            return Err(RecoveryStateFailure::MissingIncident);
        };
        merge_artifacts(
            &mut incident.recovery_artifacts,
            completion_evidence.recovery_artifacts,
        );
        incident
            .path_effect_history
            .extend(completion_evidence.path_effect_history);
        Ok(())
    }

    pub(super) fn abandon_recovery_source_mutation_for_reinspection(
        &mut self,
        integration_error: PipelineProtocolError,
    ) -> Result<(), RecoveryStateFailure> {
        self.pipeline
            .abandon_in_flight_for_reinspection()
            .ok_or(RecoveryStateFailure::Pipeline(integration_error))?;
        let incident = self
            .incident
            .as_mut()
            .ok_or(RecoveryStateFailure::MissingIncident)?;
        incident.cleanup = match incident.cleanup {
            RecoveryCleanupState::InFlight { through, .. } => {
                RecoveryCleanupState::Pending { through }
            }
            ref state => state.clone(),
        };
        Ok(())
    }

    pub(super) fn rotate_and_checkout_handle(
        &mut self,
    ) -> Result<PersistenceRecoveryHandle, RecoveryStateFailure> {
        if self.incident.is_none() {
            return Err(RecoveryStateFailure::MissingIncident);
        }
        let next_handle_id = self
            .next_recovery_handle_id
            .checked_add(1)
            .ok_or(RecoveryStateFailure::HandleIdExhausted)?;
        let next_lease_nonce = self
            .next_recovery_lease_nonce
            .checked_add(1)
            .ok_or(RecoveryStateFailure::LeaseNonceExhausted)?;
        let handle_id = RecoveryHandleId(self.next_recovery_handle_id);
        let lease = RecoveryLeaseNonce(self.next_recovery_lease_nonce);
        self.next_recovery_handle_id = next_handle_id;
        self.next_recovery_lease_nonce = next_lease_nonce;
        let incident = self
            .incident
            .as_mut()
            .ok_or(RecoveryStateFailure::MissingIncident)?;
        incident.handle = RecoveryHandleState {
            id: handle_id,
            availability: RecoveryHandleAvailability::CheckedOut(lease),
        };
        Ok(PersistenceRecoveryHandle {
            controller_id: self.id,
            incident: incident.id,
            barrier: incident.barrier,
            handle_id,
            lease,
            lifecycle: self.lifecycle_tx.clone(),
            armed: true,
        })
    }

    pub(super) fn deliver_terminal(
        &mut self,
        result: PersistenceRecoveryResult,
    ) -> SubmitPersistenceRecoveryResult {
        let Some(active) = self.active_recovery.take() else {
            return SubmitPersistenceRecoveryResult::BlockedControllerState {
                reason: RecoveryStateFailure::MissingActiveAttempt,
            };
        };
        let attempt = active.id;
        if let Err(undelivered) = active.completion.send(result) {
            drop(undelivered.0);
        }
        SubmitPersistenceRecoveryResult::Terminal { attempt }
    }
}

pub(super) fn completion_protocol_error(
    result: &RecoveryIoResult,
    expectation: &RecoveryCommandExpectation,
) -> Option<RecoveryCompletionProtocolError> {
    match (result, expectation) {
        (RecoveryIoResult::Inspected(_), RecoveryCommandExpectation::Inspection) => None,
        (
            RecoveryIoResult::SourceMutation(result),
            RecoveryCommandExpectation::SourceMutation { mutation_id, .. },
        ) if result.id() == *mutation_id => None,
        (
            RecoveryIoResult::SourceMutation(_),
            RecoveryCommandExpectation::SourceMutation { .. },
        ) => Some(RecoveryCompletionProtocolError::UnexpectedSourceMutationIdentity),
        _ => Some(RecoveryCompletionProtocolError::UnexpectedResultKind),
    }
}

pub(super) fn observation_matches_file_status(
    status: &RuntimeUiFileStatus,
    envelope: &RuntimeStateObservedEnvelope,
) -> bool {
    match (status, envelope) {
        (RuntimeUiFileStatus::Missing, RuntimeStateObservedEnvelope::Missing)
        | (RuntimeUiFileStatus::Supported, RuntimeStateObservedEnvelope::Version(1))
        | (
            RuntimeUiFileStatus::Invalid,
            RuntimeStateObservedEnvelope::PresentWithoutReadableVersion,
        ) => true,
        (
            RuntimeUiFileStatus::UnsupportedReadOnly { version },
            RuntimeStateObservedEnvelope::Version(observed),
        ) => *observed != 1 && version.is_none_or(|version| version == *observed),
        _ => false,
    }
}

pub(super) fn reinspection_writer_observation(
    kind: &RecoveryAttemptKind,
) -> Option<RuntimeStateSourceObservation> {
    match kind {
        RecoveryAttemptKind::ReinspectExternalAuthority {
            writer_observation, ..
        } => writer_observation.clone(),
        _ => None,
    }
}

pub(super) fn evidence(incident: &PersistenceIncident) -> PersistenceRecoveryEvidence {
    PersistenceRecoveryEvidence {
        recovery_artifacts: incident.recovery_artifacts.clone(),
        path_effect_history: incident.path_effect_history.clone(),
    }
}

pub(super) fn merge_artifacts(
    current: &mut Vec<RuntimeStateRecoveryArtifact>,
    discovered: Vec<RuntimeStateRecoveryArtifact>,
) {
    let mut paths = current
        .iter()
        .map(|artifact| artifact.path.clone())
        .collect::<BTreeSet<_>>();
    for artifact in discovered {
        if paths.insert(artifact.path.clone()) {
            current.push(artifact);
        }
    }
}

pub(super) fn allocate_counter(counter: &mut u64) -> Option<u64> {
    let current = *counter;
    *counter = current.checked_add(1)?;
    Some(current)
}

pub(super) fn source_mutation_observation_for_protocol_error(
    result: &SourceMutationResult,
    applied_present_envelope: RuntimeStateObservedEnvelope,
) -> Option<RuntimeStateSourceObservation> {
    match result {
        SourceMutationResult::Applied { new_source, .. } => Some(RuntimeStateSourceObservation {
            envelope: if new_source.bytes().is_none() {
                RuntimeStateObservedEnvelope::Missing
            } else {
                applied_present_envelope
            },
            revision: new_source.clone(),
        }),
        SourceMutationResult::SourceChangedBeforeMutation { active, .. }
        | SourceMutationResult::ObservationChangedAfterClaim { active, .. } => Some(active.clone()),
        SourceMutationResult::Failed { active, .. } => active.clone(),
    }
}
