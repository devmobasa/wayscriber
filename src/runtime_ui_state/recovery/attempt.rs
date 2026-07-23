use super::*;

impl RuntimeUiStateController {
    pub(super) fn integrate_recovery_inspection(
        &mut self,
        result: Result<RecoveryInspection, RuntimeStateInspectionError>,
    ) -> SubmitPersistenceRecoveryResult {
        let cancel_requested = self
            .active_recovery
            .as_ref()
            .is_some_and(|active| active.cancel_requested);
        let writer_observation = self
            .active_recovery
            .as_ref()
            .and_then(|active| reinspection_writer_observation(&active.kind));
        let inspection = match result {
            Ok(inspection) => inspection,
            Err(error) => {
                if cancel_requested {
                    if self.shutting_down {
                        return self.finish_recovery_shutdown();
                    }
                    let attempt = self.active_recovery.as_ref().expect("active attempt").id;
                    self.finish_cancelled_attempt(writer_observation);
                    return SubmitPersistenceRecoveryResult::Terminal { attempt };
                }
                return self.finish_still_unhealthy(
                    RuntimeStateIoError::new(format!(
                        "runtime-state inspection failed: {}",
                        error.message()
                    )),
                    writer_observation,
                );
            }
        };
        if cancel_requested {
            if self.shutting_down {
                return self.finish_recovery_shutdown();
            }
            let attempt = self.active_recovery.as_ref().expect("active attempt").id;
            let active = inspection
                .observation
                .is_consistent()
                .then_some(inspection.observation)
                .or(writer_observation);
            self.finish_cancelled_attempt(active);
            return SubmitPersistenceRecoveryResult::Terminal { attempt };
        }
        if !inspection.observation.is_consistent() {
            return self.finish_still_unhealthy(
                RuntimeStateIoError::new(
                    "runtime-state inspection returned an inconsistent observation",
                ),
                Some(inspection.observation),
            );
        }
        let decoded_shape_is_valid = match &inspection.observation.envelope {
            RuntimeStateObservedEnvelope::Version(1) => inspection.supported_wire.is_some(),
            RuntimeStateObservedEnvelope::Missing
            | RuntimeStateObservedEnvelope::Version(_)
            | RuntimeStateObservedEnvelope::PresentWithoutReadableVersion => {
                inspection.supported_wire.is_none()
            }
        };
        if !decoded_shape_is_valid {
            return self.finish_still_unhealthy(
                RuntimeStateIoError::new(
                    "runtime-state inspection returned an invalid decoded authority shape",
                ),
                Some(inspection.observation),
            );
        }
        let observation = inspection.observation;
        let exact_source = self.incident.as_ref().is_some_and(|incident| {
            observation.revision == incident.retained_authority.expected_source
        });
        let prior_effects_are_known = self.incident.as_ref().is_some_and(|incident| {
            incident.path_effect_history.iter().all(|effect| {
                !matches!(effect, RuntimeStateFailurePathEffect::UnknownAfterMutation)
            })
        });
        let kind = &self.active_recovery.as_ref().expect("active attempt").kind;
        match kind {
            RecoveryAttemptKind::RequestPreserveInvalidReset => {
                if !matches!(
                    observation.envelope,
                    RuntimeStateObservedEnvelope::PresentWithoutReadableVersion
                ) {
                    return self.finish_still_unhealthy(
                        RuntimeStateIoError::new("active runtime state is not invalid"),
                        Some(observation),
                    );
                }
                self.finish_confirmation_required(observation)
            }
            RecoveryAttemptKind::ConfirmPreserveInvalidReset { confirmation } => {
                if observation.revision != confirmation.revision
                    || observation.envelope != confirmation.envelope
                {
                    return self.finish_observation_changed(
                        confirmation.revision.clone(),
                        observation,
                        RuntimeStateObservedPathEffect::Untouched,
                    );
                }
                self.dispatch_preserve_invalid(observation)
            }
            RecoveryAttemptKind::RetryPending if !exact_source => self
                .install_recovery_external_authority(
                    RecoveryInspection::new(observation, inspection.supported_wire),
                    None,
                    RuntimeStateObservedPathEffect::Untouched,
                ),
            RecoveryAttemptKind::DiscardPendingAndAdoptObserved => self
                .install_recovery_external_authority(
                    RecoveryInspection::new(observation, inspection.supported_wire),
                    None,
                    RuntimeStateObservedPathEffect::Untouched,
                ),
            RecoveryAttemptKind::RetryPending
                if !matches!(
                    observation.envelope,
                    RuntimeStateObservedEnvelope::Missing
                        | RuntimeStateObservedEnvelope::Version(1)
                ) =>
            {
                self.finish_still_unhealthy(
                    RuntimeStateIoError::new(
                        "active runtime state is not safely writable; adopt or explicitly preserve it",
                    ),
                    Some(observation),
                )
            }
            RecoveryAttemptKind::RetryPending if !prior_effects_are_known => self
                .finish_still_unhealthy(
                    RuntimeStateIoError::new(
                        "runtime-state mutation effects are unknown; pending state cannot be retried",
                    ),
                    Some(observation),
                ),
            RecoveryAttemptKind::RetryPending => self.dispatch_canonical_recovery(observation),
            RecoveryAttemptKind::ConfirmPreserveInvalidResetInFlight { .. } => self
                .finish_still_unhealthy(
                    RuntimeStateIoError::new("invalid recovery attempt phase"),
                    Some(observation),
                ),
            RecoveryAttemptKind::ReinspectExternalAuthority {
                writer_observation,
                path_effect,
                preserve_invalid_confirmed,
            } => {
                if let Some(confirmed) = preserve_invalid_confirmed
                    && matches!(
                        observation.envelope,
                        RuntimeStateObservedEnvelope::PresentWithoutReadableVersion
                    )
                {
                    self.finish_observation_changed(
                        confirmed.clone(),
                        observation,
                        path_effect.clone(),
                    )
                } else {
                    self.install_recovery_external_authority(
                        RecoveryInspection::new(observation, inspection.supported_wire),
                        writer_observation.clone(),
                        path_effect.clone(),
                    )
                }
            }
            RecoveryAttemptKind::ExternalAuthorityCleanup { .. } => self.finish_still_unhealthy(
                RuntimeStateIoError::new("invalid external cleanup inspection phase"),
                Some(observation),
            ),
            RecoveryAttemptKind::ProtocolFailureReinspection => self.finish_still_unhealthy(
                RuntimeStateIoError::new(
                    "recovery protocol failed; the active source was reinspected before retry",
                ),
                Some(observation),
            ),
        }
    }

    pub(super) fn dispatch_canonical_recovery(
        &mut self,
        observation: RuntimeStateSourceObservation,
    ) -> SubmitPersistenceRecoveryResult {
        self.apply_incident_staged_reload();
        let canonical = RuntimeUiWireState {
            model: self.model.clone(),
            passthrough: self.passthrough.clone(),
        };
        let dirty = canonical != *self.pipeline.acknowledged_wire();
        let recovery_command_id = if dirty {
            if let Err(error) = self.pipeline.preflight_recovery_replace() {
                return self.finish_still_unhealthy(
                    RuntimeStateIoError::new(format!(
                        "canonical recovery command cannot be dispatched: {error:?}"
                    )),
                    Some(observation),
                );
            }
            let Some(command_id) = allocate_counter(&mut self.next_recovery_command_id) else {
                return self.finish_still_unhealthy(
                    RuntimeStateIoError::new("recovery command id exhausted"),
                    Some(observation),
                );
            };
            Some(RecoveryCommandId(command_id))
        } else {
            None
        };
        let retry = self
            .incident
            .as_ref()
            .and_then(|incident| incident.retry_desired_through);
        let cleanup = match self.incident.as_ref().expect("incident").cleanup {
            RecoveryCleanupState::Pending { through } => Some(through),
            RecoveryCleanupState::NeedsRecompute => match self.pipeline.reserve_revision() {
                Ok(through) => Some(through),
                Err(_) => {
                    return self.finish_still_unhealthy(
                        RuntimeStateIoError::new("cleanup revision allocation failed"),
                        Some(observation),
                    );
                }
            },
            RecoveryCleanupState::Clean => None,
            RecoveryCleanupState::InFlight { .. } => {
                return self.finish_still_unhealthy(
                    RuntimeStateIoError::new("cleanup command is already in flight"),
                    Some(observation),
                );
            }
        };
        if !dirty {
            if let Some(cleanup) = cleanup {
                self.pipeline
                    .settle_persisted([cleanup], self.pipeline.stable_source().clone());
            }
            if let Some(incident) = &mut self.incident {
                incident.cleanup = RecoveryCleanupState::Clean;
            }
            return self.finish_current_recovery_success(observation);
        }
        let Some(through) = cleanup.or(retry) else {
            return self.finish_still_unhealthy(
                RuntimeStateIoError::new("dirty recovery has no accepted durability watermark"),
                Some(observation),
            );
        };
        let mut covered = self
            .incident
            .as_ref()
            .expect("incident")
            .held_replacements
            .iter()
            .flat_map(|stage| stage.covered.iter().copied())
            .collect::<Vec<_>>();
        if let Some(cleanup) = cleanup {
            covered.push(cleanup);
        }
        covered.sort_unstable();
        covered.dedup();
        let request = match self.pipeline.dispatch_recovery_replace(
            canonical,
            through,
            covered,
            self.authority_epoch,
        ) {
            Ok(request) => request,
            Err(_) => {
                return self.finish_still_unhealthy(
                    RuntimeStateIoError::new("canonical recovery command was not dispatched"),
                    Some(observation),
                );
            }
        };
        let purpose = RecoveryCanonicalWritePurpose {
            retry_desired_through: retry,
            cleanup_through: cleanup,
        };
        debug_assert!(purpose.is_valid());
        let result = self.dispatch_recovery_source_command(
            recovery_command_id.expect("dirty recovery allocated a command id"),
            request,
            RecoverySourceMutationKind::PersistCanonical { purpose },
        );
        if let SubmitPersistenceRecoveryResult::Continue { dispatched } = result
            && let Some(cleanup) = cleanup
            && let Some(incident) = &mut self.incident
        {
            incident.cleanup = RecoveryCleanupState::InFlight {
                through: cleanup,
                command: dispatched,
                recompute_after_ack: false,
            };
        }
        result
    }

    pub(super) fn dispatch_preserve_invalid(
        &mut self,
        _observation: RuntimeStateSourceObservation,
    ) -> SubmitPersistenceRecoveryResult {
        let attempt = self.active_recovery.as_ref().expect("active attempt").id;
        let confirmation = match &self.active_recovery.as_ref().expect("active attempt").kind {
            RecoveryAttemptKind::ConfirmPreserveInvalidReset { confirmation } => {
                confirmation.clone()
            }
            _ => unreachable!(),
        };
        let command_id = RecoveryCommandId(
            allocate_counter(&mut self.next_recovery_command_id)
                .expect("recovery command id exhausted"),
        );
        let mutation_id = SourceMutationId(command_id.0);
        let active = self.active_recovery.as_mut().expect("active attempt");
        active.kind = RecoveryAttemptKind::ConfirmPreserveInvalidResetInFlight {
            confirmation: confirmation.clone(),
        };
        active.current_command = ActiveRecoveryCommand {
            id: command_id,
            expected: RecoveryCommandExpectation::SourceMutation {
                mutation_id,
                kind: RecoverySourceMutationKind::PreserveInvalid,
                accepted_through: None,
            },
        };
        self.recovery_outbox.push_back(RecoveryIoCommand {
            controller_id: self.id,
            incident: active.incident,
            barrier: active.barrier,
            attempt,
            command_id,
            operation: RecoveryIoOperation::PreserveInvalidIfUnchanged {
                mutation_id,
                confirmation,
            },
        });
        if let Some(barrier) = &mut self.active_barrier {
            barrier.phase = ControllerBarrierPhase::Recovering {
                incident: active.incident,
                attempt: active.id,
                step: RecoveryAttemptStep::SourceMutationInFlight(command_id),
            };
        }
        SubmitPersistenceRecoveryResult::Continue {
            dispatched: command_id,
        }
    }

    pub(super) fn dispatch_recovery_source_command(
        &mut self,
        command_id: RecoveryCommandId,
        request: SourceMutationRequest,
        kind: RecoverySourceMutationKind,
    ) -> SubmitPersistenceRecoveryResult {
        let accepted_through = match &kind {
            RecoverySourceMutationKind::PersistCanonical { .. } => Some(request.accepted_through),
            RecoverySourceMutationKind::PreserveInvalid => None,
        };
        let purpose = match &kind {
            RecoverySourceMutationKind::PersistCanonical { purpose } => *purpose,
            RecoverySourceMutationKind::PreserveInvalid => unreachable!(),
        };
        let active = self.active_recovery.as_mut().expect("active attempt");
        active.current_command = ActiveRecoveryCommand {
            id: command_id,
            expected: RecoveryCommandExpectation::SourceMutation {
                mutation_id: request.id,
                kind,
                accepted_through,
            },
        };
        self.recovery_outbox.push_back(RecoveryIoCommand {
            controller_id: self.id,
            incident: active.incident,
            barrier: active.barrier,
            attempt: active.id,
            command_id,
            operation: RecoveryIoOperation::PersistCanonicalIfUnchanged { request, purpose },
        });
        if let Some(barrier) = &mut self.active_barrier {
            barrier.phase = ControllerBarrierPhase::Recovering {
                incident: active.incident,
                attempt: active.id,
                step: RecoveryAttemptStep::SourceMutationInFlight(command_id),
            };
        }
        SubmitPersistenceRecoveryResult::Continue {
            dispatched: command_id,
        }
    }

    pub(super) fn integrate_recovery_source_mutation(
        &mut self,
        result: SourceMutationResult,
    ) -> SubmitPersistenceRecoveryResult {
        if matches!(
            self.active_recovery.as_ref().map(|active| &active.kind),
            Some(RecoveryAttemptKind::ConfirmPreserveInvalidResetInFlight { .. })
        ) {
            return self.integrate_preserve_invalid_source_mutation(result);
        }
        let cancel_requested = self
            .active_recovery
            .as_ref()
            .is_some_and(|active| active.cancel_requested);
        let purpose = self.active_recovery.as_ref().and_then(|active| {
            match &active.current_command.expected {
                RecoveryCommandExpectation::SourceMutation {
                    kind: RecoverySourceMutationKind::PersistCanonical { purpose },
                    ..
                } => Some(*purpose),
                _ => None,
            }
        });
        let integrated = match self.pipeline.integrate(result.clone()) {
            Ok(integrated) => integrated,
            Err(error) => {
                let active = source_mutation_observation_for_protocol_error(
                    &result,
                    RuntimeStateObservedEnvelope::Version(1),
                );
                self.retain_recovery_mutation_evidence(&result);
                self.abandon_recovery_source_mutation_for_reinspection();
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
                        "recovery source acknowledgement was invalid: {error:?}"
                    )),
                    active,
                );
            }
        };
        match result {
            SourceMutationResult::Applied {
                new_source,
                recovery_artifacts,
                ..
            } => {
                let recovery_path = recovery_artifacts
                    .last()
                    .map(|artifact| artifact.path.clone());
                if let Some(incident) = &mut self.incident {
                    merge_artifacts(&mut incident.recovery_artifacts, recovery_artifacts);
                }
                self.pipeline
                    .resume_after_integration()
                    .expect("recovery pipeline resume");
                if matches!(&integrated.request.kind, SourceMutationKind::Replace(_)) {
                    self.file_status = RuntimeUiFileStatus::Supported;
                }
                self.note_recovery_write_applied(purpose);
                let observation = RuntimeStateSourceObservation {
                    envelope: if new_source.bytes().is_some() {
                        RuntimeStateObservedEnvelope::Version(1)
                    } else {
                        RuntimeStateObservedEnvelope::Missing
                    },
                    revision: new_source,
                };
                if cancel_requested {
                    if self.shutting_down {
                        return self.finish_recovery_shutdown();
                    }
                    let attempt = self.active_recovery.as_ref().expect("active attempt").id;
                    self.finish_cancelled_attempt(Some(observation.clone()));
                    return SubmitPersistenceRecoveryResult::Terminal { attempt };
                }
                if matches!(
                    integrated.request.kind,
                    SourceMutationKind::ResetUnsupportedIfUnchanged { .. }
                ) {
                    let Some(recovery_path) = recovery_path else {
                        self.install_preserved_invalid_authority();
                        return self.finish_still_unhealthy(
                            RuntimeStateIoError::new(
                                "unsupported reset did not report the retained recovery artifact",
                            ),
                            Some(observation),
                        );
                    };
                    self.finish_preserved_invalid(observation, recovery_path)
                } else {
                    self.continue_after_recovery_write(observation)
                }
            }
            SourceMutationResult::SourceChangedBeforeMutation { active, .. } => {
                self.note_recovery_write_external();
                if cancel_requested {
                    if self.shutting_down {
                        return self.finish_recovery_shutdown();
                    }
                    let attempt = self.active_recovery.as_ref().expect("active attempt").id;
                    self.finish_cancelled_attempt(Some(active));
                    return SubmitPersistenceRecoveryResult::Terminal { attempt };
                }
                self.dispatch_external_authority_reinspection(
                    active,
                    RuntimeStateObservedPathEffect::Untouched,
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
                self.note_recovery_write_external();
                if cancel_requested {
                    if self.shutting_down {
                        return self.finish_recovery_shutdown();
                    }
                    let attempt = self.active_recovery.as_ref().expect("active attempt").id;
                    self.finish_cancelled_attempt(Some(active));
                    return SubmitPersistenceRecoveryResult::Terminal { attempt };
                }
                self.dispatch_external_authority_reinspection(
                    active,
                    RuntimeStateObservedPathEffect::PostClaim(path_effect),
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
                self.pipeline
                    .settle_failed(integrated.covered.iter().copied(), error.clone());
                self.note_recovery_write_failed(purpose);
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

    pub(super) fn integrate_protocol_failure_source_completion(
        &mut self,
        result: SourceMutationResult,
    ) -> SubmitPersistenceRecoveryResult {
        let cancel_requested = self
            .active_recovery
            .as_ref()
            .is_some_and(|active| active.cancel_requested);
        let preserve_invalid = matches!(
            self.active_recovery.as_ref().map(|active| &active.kind),
            Some(RecoveryAttemptKind::ConfirmPreserveInvalidResetInFlight { .. })
        );
        let purpose = self.active_recovery.as_ref().and_then(|active| {
            match &active.current_command.expected {
                RecoveryCommandExpectation::SourceMutation {
                    kind: RecoverySourceMutationKind::PersistCanonical { purpose },
                    ..
                } => Some(*purpose),
                _ => None,
            }
        });
        let active_observation = source_mutation_observation_for_protocol_error(
            &result,
            if preserve_invalid {
                RuntimeStateObservedEnvelope::PresentWithoutReadableVersion
            } else {
                RuntimeStateObservedEnvelope::Version(1)
            },
        );
        self.retain_recovery_mutation_evidence(&result);

        if !preserve_invalid {
            match self.pipeline.integrate(result.clone()) {
                Ok(integrated) => match &result {
                    SourceMutationResult::Applied { .. } => {
                        if matches!(&integrated.request.kind, SourceMutationKind::Replace(_)) {
                            self.file_status = RuntimeUiFileStatus::Supported;
                        }
                        self.note_recovery_write_applied(purpose);
                    }
                    SourceMutationResult::SourceChangedBeforeMutation { .. }
                    | SourceMutationResult::ObservationChangedAfterClaim { .. } => {
                        self.note_recovery_write_external();
                    }
                    SourceMutationResult::Failed { error, .. } => {
                        self.pipeline
                            .settle_failed(integrated.covered.iter().copied(), error.clone());
                        self.note_recovery_write_failed(purpose);
                    }
                },
                Err(_) => self.abandon_recovery_source_mutation_for_reinspection(),
            }
        }

        if cancel_requested {
            if self.shutting_down {
                return self.finish_recovery_shutdown();
            }
            let attempt = self.active_recovery.as_ref().expect("active attempt").id;
            self.finish_cancelled_attempt(active_observation);
            return SubmitPersistenceRecoveryResult::Terminal { attempt };
        }

        let command_id = self.dispatch_protocol_failure_reinspection();
        SubmitPersistenceRecoveryResult::Continue {
            dispatched: command_id,
        }
    }

    pub(super) fn dispatch_external_authority_reinspection(
        &mut self,
        writer_observation: RuntimeStateSourceObservation,
        path_effect: RuntimeStateObservedPathEffect,
    ) -> SubmitPersistenceRecoveryResult {
        let command_id = RecoveryCommandId(
            allocate_counter(&mut self.next_recovery_command_id)
                .expect("recovery command id exhausted"),
        );
        let active = self.active_recovery.as_mut().expect("active attempt");
        active.kind = RecoveryAttemptKind::ReinspectExternalAuthority {
            writer_observation: Some(writer_observation),
            path_effect,
            preserve_invalid_confirmed: None,
        };
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
            barrier.phase = ControllerBarrierPhase::Reinspecting;
        }
        SubmitPersistenceRecoveryResult::Continue {
            dispatched: command_id,
        }
    }

    pub(super) fn note_recovery_write_applied(
        &mut self,
        purpose: Option<RecoveryCanonicalWritePurpose>,
    ) {
        let retained_authority = self.capture_persistence_authority();
        let Some(incident) = &mut self.incident else {
            return;
        };
        if purpose.is_some_and(|purpose| purpose.retry_desired_through.is_some()) {
            incident.retry_desired_through = None;
            incident.held_replacements.clear();
        }
        incident.cleanup = match incident.cleanup {
            RecoveryCleanupState::InFlight {
                recompute_after_ack: true,
                ..
            } => RecoveryCleanupState::NeedsRecompute,
            RecoveryCleanupState::InFlight { .. } => RecoveryCleanupState::Clean,
            RecoveryCleanupState::Pending { through } => RecoveryCleanupState::Pending { through },
            RecoveryCleanupState::NeedsRecompute => RecoveryCleanupState::NeedsRecompute,
            RecoveryCleanupState::Clean => RecoveryCleanupState::Clean,
        };
        incident.retained_authority = retained_authority;
    }

    pub(super) fn note_recovery_write_failed(
        &mut self,
        purpose: Option<RecoveryCanonicalWritePurpose>,
    ) {
        let Some(incident) = &mut self.incident else {
            return;
        };
        if let Some(purpose) = purpose {
            if purpose.retry_desired_through.is_some() {
                incident.held_replacements.clear();
                incident.retry_desired_through = purpose.retry_desired_through;
            }
            if purpose.cleanup_through.is_some() {
                incident.cleanup = RecoveryCleanupState::NeedsRecompute;
            }
        } else if matches!(incident.cleanup, RecoveryCleanupState::InFlight { .. }) {
            incident.cleanup = RecoveryCleanupState::Clean;
        }
    }

    pub(super) fn note_recovery_write_external(&mut self) {
        let Some(incident) = &mut self.incident else {
            return;
        };
        incident.held_replacements.clear();
        incident.retry_desired_through = None;
        incident.cleanup = RecoveryCleanupState::Clean;
    }

    pub(super) fn continue_after_recovery_write(
        &mut self,
        observation: RuntimeStateSourceObservation,
    ) -> SubmitPersistenceRecoveryResult {
        let needs_recompute = self.incident.as_ref().is_some_and(|incident| {
            incident.staged_reload.is_some()
                || matches!(incident.cleanup, RecoveryCleanupState::NeedsRecompute)
        });
        if needs_recompute {
            return self.dispatch_canonical_recovery(observation);
        }
        self.finish_current_recovery_success(observation)
    }

    pub(super) fn capture_persistence_authority(&self) -> PersistenceAuthoritySnapshot {
        PersistenceAuthoritySnapshot {
            expected_source: self.pipeline.stable_source().clone(),
            file_status: self.file_status.clone(),
            authority_epoch: self.authority_epoch,
            model: self.model.clone(),
            passthrough: self.passthrough.clone(),
            seeds: self.seeds.clone(),
            live_state: self.live_state.clone(),
        }
    }
}
