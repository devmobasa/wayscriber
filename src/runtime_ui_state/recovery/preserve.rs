use super::*;

impl RuntimeUiStateController {
    pub(super) fn integrate_preserve_invalid_source_mutation(
        &mut self,
        result: SourceMutationResult,
    ) -> SubmitPersistenceRecoveryResult {
        let cancel_requested = self
            .active_recovery
            .as_ref()
            .is_some_and(|active| active.cancel_requested);
        if let Err(error) = validate_source_mutation_evidence(&result) {
            let active = source_mutation_observation_for_protocol_error(
                &result,
                RuntimeStateObservedEnvelope::PresentWithoutReadableVersion,
            );
            self.retain_recovery_mutation_evidence(&result);
            if cancel_requested {
                if self.shutting_down {
                    return self.finish_recovery_shutdown();
                }
                let attempt = self.active_recovery.as_ref().expect("active attempt").id;
                self.finish_cancelled_attempt(active);
                return SubmitPersistenceRecoveryResult::Terminal { attempt };
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
                _ => unreachable!("preserve-invalid result requires an in-flight confirmation"),
            };
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
                if let Some(incident) = &mut self.incident {
                    merge_artifacts(&mut incident.recovery_artifacts, recovery_artifacts);
                }
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
                        let attempt = self.active_recovery.as_ref().expect("active attempt").id;
                        self.finish_cancelled_attempt(Some(observation));
                        return SubmitPersistenceRecoveryResult::Terminal { attempt };
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
                        let attempt = self.active_recovery.as_ref().expect("active attempt").id;
                        self.finish_cancelled_attempt(Some(observation));
                        return SubmitPersistenceRecoveryResult::Terminal { attempt };
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
                    self.install_preserved_invalid_authority();
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
                    self.install_preserved_invalid_authority();
                    if self.shutting_down {
                        return self.finish_recovery_shutdown();
                    }
                    let attempt = self.active_recovery.as_ref().expect("active attempt").id;
                    self.finish_cancelled_attempt(Some(observation));
                    return SubmitPersistenceRecoveryResult::Terminal { attempt };
                }
                self.finish_preserved_invalid(observation, recovery_path)
            }
            SourceMutationResult::SourceChangedBeforeMutation { active, .. } => {
                if cancel_requested {
                    if self.shutting_down {
                        return self.finish_recovery_shutdown();
                    }
                    let attempt = self.active_recovery.as_ref().expect("active attempt").id;
                    self.finish_cancelled_attempt(Some(active));
                    return SubmitPersistenceRecoveryResult::Terminal { attempt };
                }
                self.dispatch_preserve_invalid_reinspection(
                    active,
                    RuntimeStateObservedPathEffect::Untouched,
                    confirmed_revision,
                )
            }
            SourceMutationResult::ObservationChangedAfterClaim {
                active,
                recovery_artifacts,
                path_effect,
                ..
            } => {
                if let Some(incident) = &mut self.incident {
                    merge_artifacts(&mut incident.recovery_artifacts, recovery_artifacts);
                    incident
                        .path_effect_history
                        .push(RuntimeStateFailurePathEffect::Known(
                            RuntimeStateObservedPathEffect::PostClaim(path_effect.clone()),
                        ));
                }
                if cancel_requested {
                    if self.shutting_down {
                        return self.finish_recovery_shutdown();
                    }
                    let attempt = self.active_recovery.as_ref().expect("active attempt").id;
                    self.finish_cancelled_attempt(Some(active));
                    return SubmitPersistenceRecoveryResult::Terminal { attempt };
                }
                self.dispatch_preserve_invalid_reinspection(
                    active,
                    RuntimeStateObservedPathEffect::PostClaim(path_effect),
                    confirmed_revision,
                )
            }
            SourceMutationResult::Failed {
                error,
                active,
                recovery_artifacts,
                path_effect,
                ..
            } => {
                if let Some(incident) = &mut self.incident {
                    merge_artifacts(&mut incident.recovery_artifacts, recovery_artifacts);
                    incident.path_effect_history.push(path_effect);
                }
                if cancel_requested {
                    if self.shutting_down {
                        return self.finish_recovery_shutdown();
                    }
                    let attempt = self.active_recovery.as_ref().expect("active attempt").id;
                    self.finish_cancelled_attempt(active);
                    return SubmitPersistenceRecoveryResult::Terminal { attempt };
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
        if let Some(active) = &mut self.active_recovery
            && let RecoveryAttemptKind::ReinspectExternalAuthority {
                preserve_invalid_confirmed,
                ..
            } = &mut active.kind
        {
            *preserve_invalid_confirmed = Some(confirmed_revision);
        }
        result
    }
}
