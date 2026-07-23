use super::*;

impl RuntimeUiStateController {
    pub(super) fn block_protocol_failure(
        &mut self,
        reason: RecoveryCompletionProtocolError,
        completion: RecoveryIoCompletion,
    ) -> SubmitPersistenceRecoveryResult {
        self.record_rejected_recovery_completion(completion.clone());
        if let RecoveryIoResult::SourceMutation(result) = &completion.result {
            self.retain_recovery_mutation_evidence(result);
        }
        let source_mutation_in_flight = matches!(
            self.active_recovery
                .as_ref()
                .map(|active| &active.current_command.expected),
            Some(RecoveryCommandExpectation::SourceMutation { .. })
        );
        let incident = self.incident.as_ref().expect("incident").id;
        let attempt = self
            .active_recovery
            .as_ref()
            .map_or(RecoveryAttemptId(0), |active| active.id);
        if source_mutation_in_flight {
            if let Some(incident) = &mut self.incident
                && !matches!(
                    incident.path_effect_history.last(),
                    Some(RuntimeStateFailurePathEffect::UnknownAfterMutation)
                )
            {
                incident
                    .path_effect_history
                    .push(RuntimeStateFailurePathEffect::UnknownAfterMutation);
            }
            let command = self
                .active_recovery
                .as_ref()
                .expect("source mutation belongs to an active attempt")
                .current_command
                .id;
            self.active_recovery
                .as_mut()
                .expect("source mutation belongs to an active attempt")
                .protocol_failure_pending = true;
            if let Some(active_barrier) = &mut self.active_barrier {
                active_barrier.phase = ControllerBarrierPhase::Recovering {
                    incident,
                    attempt,
                    step: RecoveryAttemptStep::ProtocolFailureAwaitingSourceMutation(command),
                };
            }
            return SubmitPersistenceRecoveryResult::BlockedProtocolFailure {
                reason,
                evidence: evidence(self.incident.as_ref().expect("incident")),
                reinspection_dispatched: None,
            };
        }

        let active_command = self
            .active_recovery
            .as_ref()
            .expect("protocol failure belongs to an active attempt")
            .current_command
            .id;
        self.record_integrated_recovery_command(active_command);
        let command_id = self.dispatch_protocol_failure_reinspection();
        SubmitPersistenceRecoveryResult::BlockedProtocolFailure {
            reason,
            evidence: evidence(self.incident.as_ref().expect("incident")),
            reinspection_dispatched: Some(command_id),
        }
    }

    pub(super) fn dispatch_protocol_failure_reinspection(&mut self) -> RecoveryCommandId {
        let command_id = RecoveryCommandId(
            allocate_counter(&mut self.next_recovery_command_id)
                .expect("recovery command id exhausted"),
        );
        let active = self.active_recovery.as_mut().expect("active attempt");
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
        if let Some(barrier) = &mut self.active_barrier {
            barrier.phase = ControllerBarrierPhase::Recovering {
                incident: active.incident,
                attempt: active.id,
                step: RecoveryAttemptStep::Inspecting,
            };
        }
        command_id
    }

    pub(super) fn retain_recovery_mutation_evidence(&mut self, result: &SourceMutationResult) {
        let retain_path_effect = validate_source_mutation_evidence(result).is_ok();
        let Some(incident) = &mut self.incident else {
            return;
        };
        match result {
            SourceMutationResult::Applied {
                recovery_artifacts, ..
            } => merge_artifacts(&mut incident.recovery_artifacts, recovery_artifacts.clone()),
            SourceMutationResult::ObservationChangedAfterClaim {
                recovery_artifacts,
                path_effect,
                ..
            } => {
                merge_artifacts(&mut incident.recovery_artifacts, recovery_artifacts.clone());
                if retain_path_effect {
                    incident
                        .path_effect_history
                        .push(RuntimeStateFailurePathEffect::Known(
                            RuntimeStateObservedPathEffect::PostClaim(path_effect.clone()),
                        ));
                }
            }
            SourceMutationResult::Failed {
                recovery_artifacts,
                path_effect,
                ..
            } => {
                merge_artifacts(&mut incident.recovery_artifacts, recovery_artifacts.clone());
                if retain_path_effect {
                    incident.path_effect_history.push(path_effect.clone());
                }
            }
            SourceMutationResult::SourceChangedBeforeMutation { .. } => {}
        }
    }

    pub(super) fn abandon_recovery_source_mutation_for_reinspection(&mut self) {
        let _ = self.pipeline.abandon_in_flight_for_reinspection();
        if let Some(incident) = &mut self.incident {
            incident.cleanup = match incident.cleanup {
                RecoveryCleanupState::InFlight { through, .. } => {
                    RecoveryCleanupState::Pending { through }
                }
                ref state => state.clone(),
            };
        }
    }

    pub(super) fn rotate_and_checkout_handle(&mut self) -> PersistenceRecoveryHandle {
        let handle_id = RecoveryHandleId(
            allocate_counter(&mut self.next_recovery_handle_id).expect("handle id exhausted"),
        );
        let lease = RecoveryLeaseNonce(
            allocate_counter(&mut self.next_recovery_lease_nonce).expect("lease exhausted"),
        );
        let incident = self.incident.as_mut().expect("incident");
        incident.handle = RecoveryHandleState {
            id: handle_id,
            availability: RecoveryHandleAvailability::CheckedOut(lease),
        };
        PersistenceRecoveryHandle {
            controller_id: self.id,
            incident: incident.id,
            barrier: incident.barrier,
            handle_id,
            lease,
            lifecycle: self.lifecycle_tx.clone(),
            armed: true,
        }
    }

    pub(super) fn deliver_terminal(
        &mut self,
        result: PersistenceRecoveryResult,
    ) -> SubmitPersistenceRecoveryResult {
        let active = self
            .active_recovery
            .take()
            .expect("active recovery attempt");
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
