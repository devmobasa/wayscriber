use super::*;

impl RuntimeUiStateController {
    pub(super) fn integrate_preserve_invalid_source_mutation(
        &mut self,
        result: SourceMutationResult,
    ) -> SubmitPersistenceRecoveryResult {
        if let Err(error) = self.validate_active_recovery_state() {
            return self.finish_recovery_state_failure(error);
        }
        let cancel_requested = self
            .active_recovery
            .as_ref()
            .is_some_and(|active| active.cancel_requested);
        if let Err(error) = validate_source_mutation_evidence(&result) {
            let active = source_mutation_observation_for_protocol_error(
                &result,
                RuntimeStateObservedEnvelope::PresentWithoutReadableVersion,
            );
            if let Err(state_error) = self.retain_recovery_mutation_evidence_for_kind(
                &result,
                &RecoverySourceMutationKind::PreserveInvalid,
            ) {
                return self.finish_recovery_state_failure(state_error);
            }
            if cancel_requested {
                if self.shutting_down {
                    return self.finish_recovery_shutdown();
                }
                return self.finish_current_attempt_cancelled(active);
            }
            return self.finish_still_unhealthy(
                RuntimeStateIoError::new(format!(
                    "preserve-invalid acknowledgement carried invalid evidence: {error:?}"
                )),
                active,
            );
        }
        let (confirmed_revision, confirmed_envelope) =
            match self.active_recovery.as_ref().map(|active| &active.kind) {
                Some(RecoveryAttemptKind::ConfirmPreserveInvalidResetInFlight { confirmation }) => {
                    (confirmation.revision.clone(), confirmation.envelope.clone())
                }
                Some(_) => {
                    return self
                        .finish_recovery_state_failure(RecoveryStateFailure::InvalidAttemptPhase);
                }
                None => {
                    return self
                        .finish_recovery_state_failure(RecoveryStateFailure::MissingActiveAttempt);
                }
            };
        if let Err(error) = self.retain_recovery_mutation_evidence_for_kind(
            &result,
            &RecoverySourceMutationKind::PreserveInvalid,
        ) {
            return self.finish_recovery_state_failure(error);
        }
        match result {
            SourceMutationResult::Applied {
                new_source,
                recovery_artifacts,
                ..
            } => {
                let recovery_path = recovery_artifacts
                    .iter()
                    .find(|artifact| {
                        artifact.observation.envelope == confirmed_envelope
                            && artifact.observation.revision == confirmed_revision
                    })
                    .map(|artifact| artifact.path.clone());
                let observation = if new_source.bytes().is_none() {
                    RuntimeStateSourceObservation {
                        revision: new_source.clone(),
                        envelope: RuntimeStateObservedEnvelope::Missing,
                    }
                } else {
                    RuntimeStateSourceObservation {
                        revision: new_source,
                        envelope: RuntimeStateObservedEnvelope::PresentWithoutReadableVersion,
                    }
                };
                if observation.revision.path_identity() != confirmed_revision.path_identity() {
                    if cancel_requested {
                        if self.shutting_down {
                            return self.finish_recovery_shutdown();
                        }
                        return self.finish_current_attempt_cancelled(Some(observation));
                    }
                    return self.finish_still_unhealthy(
                        RuntimeStateIoError::new(
                            "preserve-invalid mutation reported a different managed path identity",
                        ),
                        Some(observation),
                    );
                }
                if !matches!(observation.envelope, RuntimeStateObservedEnvelope::Missing) {
                    if cancel_requested {
                        if self.shutting_down {
                            return self.finish_recovery_shutdown();
                        }
                        return self.finish_current_attempt_cancelled(Some(observation));
                    }
                    return self.finish_still_unhealthy(
                        RuntimeStateIoError::new(
                            "preserve-invalid mutation did not leave the runtime-state source missing",
                        ),
                        Some(observation),
                    );
                }
                self.pipeline.install_acknowledged_authority(
                    observation.revision.clone(),
                    RuntimeUiWireState::default(),
                );
                let Some(recovery_path) = recovery_path else {
                    if let Err(error) = self.install_preserved_invalid_authority() {
                        return self.finish_recovery_state_failure(error);
                    }
                    if self.shutting_down {
                        return self.finish_recovery_shutdown();
                    }
                    return self.finish_still_unhealthy(
                        RuntimeStateIoError::new(
                            "preserve-invalid mutation did not report an artifact matching the confirmed invalid source",
                        ),
                        Some(observation),
                    );
                };
                if cancel_requested {
                    if let Err(error) = self.install_preserved_invalid_authority() {
                        return self.finish_recovery_state_failure(error);
                    }
                    if self.shutting_down {
                        return self.finish_recovery_shutdown();
                    }
                    return self.finish_current_attempt_cancelled(Some(observation));
                }
                self.finish_preserved_invalid(observation, recovery_path)
            }
            SourceMutationResult::SourceChangedBeforeMutation { active, .. } => {
                if cancel_requested {
                    if self.shutting_down {
                        return self.finish_recovery_shutdown();
                    }
                    return self.finish_current_attempt_cancelled(Some(active));
                }
                self.dispatch_preserve_invalid_reinspection(
                    active,
                    RuntimeStateObservedPathEffect::Untouched,
                    confirmed_revision,
                )
            }
            SourceMutationResult::ObservationChangedAfterClaim {
                active,
                path_effect,
                ..
            } => {
                if cancel_requested {
                    if self.shutting_down {
                        return self.finish_recovery_shutdown();
                    }
                    return self.finish_current_attempt_cancelled(Some(active));
                }
                self.dispatch_preserve_invalid_reinspection(
                    active,
                    RuntimeStateObservedPathEffect::PostClaim(path_effect),
                    confirmed_revision,
                )
            }
            SourceMutationResult::Failed { error, active, .. } => {
                if cancel_requested {
                    if self.shutting_down {
                        return self.finish_recovery_shutdown();
                    }
                    return self.finish_current_attempt_cancelled(active);
                }
                self.finish_still_unhealthy(error, active)
            }
        }
    }

    pub(super) fn dispatch_preserve_invalid_reinspection(
        &mut self,
        writer_observation: RuntimeStateSourceObservation,
        path_effect: RuntimeStateObservedPathEffect,
        confirmed_revision: RuntimeStateSourceRevision,
    ) -> SubmitPersistenceRecoveryResult {
        let result = self.dispatch_external_authority_reinspection(writer_observation, path_effect);
        if !matches!(&result, SubmitPersistenceRecoveryResult::Continue { .. }) {
            return result;
        }
        let Some(active) = &mut self.active_recovery else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::MissingActiveAttempt);
        };
        let RecoveryAttemptKind::ReinspectExternalAuthority {
            preserve_invalid_confirmed,
            ..
        } = &mut active.kind
        else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::InvalidAttemptPhase);
        };
        *preserve_invalid_confirmed = Some(confirmed_revision);
        result
    }
}
