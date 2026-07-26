use super::*;

impl RuntimeUiStateController {
    pub(super) fn integrate_recovery_inspection(
        &mut self,
        result: Result<RecoveryInspection, RuntimeStateInspectionError>,
    ) -> SubmitPersistenceRecoveryResult {
        if let Err(error) = self.validate_active_recovery_state() {
            return self.finish_recovery_state_failure(error);
        }
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
                    return self.finish_current_attempt_cancelled(writer_observation);
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
            let active = inspection
                .observation
                .is_consistent()
                .then_some(inspection.observation)
                .or(writer_observation);
            return self.finish_current_attempt_cancelled(active);
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
        let Some(kind) = self
            .active_recovery
            .as_ref()
            .map(|active| active.kind.clone())
        else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::MissingActiveAttempt);
        };
        match &kind {
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
                .finish_recovery_state_failure(RecoveryStateFailure::InvalidAttemptPhase),
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
            RecoveryAttemptKind::ExternalAuthorityCleanup { .. } => self
                .finish_recovery_state_failure(RecoveryStateFailure::InvalidAttemptPhase),
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
        if let Err(error) = self.validate_active_recovery_state() {
            return self.finish_recovery_state_failure(error);
        }
        self.apply_incident_staged_reload();
        let canonical = RuntimeUiWireState {
            model: self.model.clone(),
            passthrough: self.passthrough.clone(),
        };
        let dirty = canonical != *self.pipeline.acknowledged_wire();
        let recovery_command_id = if dirty {
            if let Err(error) = self.pipeline.preflight_recovery_replace() {
                return self.finish_recovery_state_failure(RecoveryStateFailure::Pipeline(error));
            }
            let Some(command_id) = allocate_counter(&mut self.next_recovery_command_id) else {
                return self
                    .finish_recovery_state_failure(RecoveryStateFailure::CommandIdExhausted);
            };
            Some(RecoveryCommandId(command_id))
        } else {
            None
        };
        let retry = self
            .incident
            .as_ref()
            .and_then(|incident| incident.retry_desired_through);
        let Some(incident) = self.incident.as_ref() else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::MissingIncident);
        };
        let cleanup = match &incident.cleanup {
            RecoveryCleanupState::Pending { through } => Some(*through),
            RecoveryCleanupState::NeedsRecompute => match self.pipeline.reserve_revision() {
                Ok(through) => Some(through),
                Err(error) => {
                    return self
                        .finish_recovery_state_failure(RecoveryStateFailure::Pipeline(error));
                }
            },
            RecoveryCleanupState::Clean => None,
            RecoveryCleanupState::InFlight { .. } => {
                return self
                    .finish_recovery_state_failure(RecoveryStateFailure::InvalidAttemptPhase);
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
            return self.finish_recovery_state_failure(RecoveryStateFailure::InvalidAttemptPhase);
        };
        let Some(incident) = self.incident.as_ref() else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::MissingIncident);
        };
        let mut covered = incident
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
            Err(error) => {
                return self.finish_recovery_state_failure(RecoveryStateFailure::Pipeline(error));
            }
        };
        let purpose = RecoveryCanonicalWritePurpose {
            retry_desired_through: retry,
            cleanup_through: cleanup,
        };
        if !purpose.is_valid() {
            return self.finish_recovery_state_failure(RecoveryStateFailure::InvalidAttemptPhase);
        }
        let Some(recovery_command_id) = recovery_command_id else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::CommandIdExhausted);
        };
        let result = self.dispatch_recovery_source_command(recovery_command_id, request, purpose);
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
        if let Err(error) = self.validate_active_recovery_state() {
            return self.finish_recovery_state_failure(error);
        }
        let (attempt, confirmation) = match self.active_recovery.as_ref() {
            Some(ActiveRecoveryAttempt {
                id,
                kind: RecoveryAttemptKind::ConfirmPreserveInvalidReset { confirmation },
                ..
            }) => (*id, confirmation.clone()),
            Some(_) => {
                return self
                    .finish_recovery_state_failure(RecoveryStateFailure::InvalidAttemptPhase);
            }
            None => {
                return self
                    .finish_recovery_state_failure(RecoveryStateFailure::MissingActiveAttempt);
            }
        };
        let Some(command_id) =
            allocate_counter(&mut self.next_recovery_command_id).map(RecoveryCommandId)
        else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::CommandIdExhausted);
        };
        let mutation_id = SourceMutationId(command_id.0);
        let Some(active) = self.active_recovery.as_mut() else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::MissingActiveAttempt);
        };
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
        let Some(barrier) = &mut self.active_barrier else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::MissingBarrier);
        };
        barrier.phase = ControllerBarrierPhase::Recovering {
            incident: active.incident,
            attempt: active.id,
            step: RecoveryAttemptStep::SourceMutationInFlight(command_id),
        };
        SubmitPersistenceRecoveryResult::Continue {
            dispatched: command_id,
        }
    }

    pub(super) fn dispatch_recovery_source_command(
        &mut self,
        command_id: RecoveryCommandId,
        request: SourceMutationRequest,
        purpose: RecoveryCanonicalWritePurpose,
    ) -> SubmitPersistenceRecoveryResult {
        if let Err(error) = self.validate_active_recovery_state() {
            return self.finish_recovery_state_failure(error);
        }
        let kind = RecoverySourceMutationKind::PersistCanonical { purpose };
        let active = match self.active_recovery.as_mut() {
            Some(active) => active,
            None => {
                return self
                    .finish_recovery_state_failure(RecoveryStateFailure::MissingActiveAttempt);
            }
        };
        active.current_command = ActiveRecoveryCommand {
            id: command_id,
            expected: RecoveryCommandExpectation::SourceMutation {
                mutation_id: request.id,
                kind,
                accepted_through: Some(request.accepted_through),
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
        let Some(barrier) = &mut self.active_barrier else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::MissingBarrier);
        };
        barrier.phase = ControllerBarrierPhase::Recovering {
            incident: active.incident,
            attempt: active.id,
            step: RecoveryAttemptStep::SourceMutationInFlight(command_id),
        };
        SubmitPersistenceRecoveryResult::Continue {
            dispatched: command_id,
        }
    }

    pub(super) fn integrate_recovery_source_mutation(
        &mut self,
        result: SourceMutationResult,
    ) -> SubmitPersistenceRecoveryResult {
        if let Err(error) = self.validate_active_recovery_state() {
            return self.finish_recovery_state_failure(error);
        }
        let purpose = match self.active_recovery.as_ref() {
            Some(active)
                if matches!(
                    (&active.kind, &active.current_command.expected),
                    (
                        RecoveryAttemptKind::ConfirmPreserveInvalidResetInFlight { .. },
                        RecoveryCommandExpectation::SourceMutation {
                            kind: RecoverySourceMutationKind::PreserveInvalid,
                            ..
                        }
                    )
                ) =>
            {
                return self.integrate_preserve_invalid_source_mutation(result);
            }
            Some(ActiveRecoveryAttempt {
                current_command:
                    ActiveRecoveryCommand {
                        expected:
                            RecoveryCommandExpectation::SourceMutation {
                                kind: RecoverySourceMutationKind::PersistCanonical { purpose },
                                ..
                            },
                        ..
                    },
                ..
            }) => Some(*purpose),
            Some(_) => {
                return self
                    .finish_recovery_state_failure(RecoveryStateFailure::InvalidAttemptPhase);
            }
            None => {
                return self
                    .finish_recovery_state_failure(RecoveryStateFailure::MissingActiveAttempt);
            }
        };
        let cancel_requested = self
            .active_recovery
            .as_ref()
            .is_some_and(|active| active.cancel_requested);
        let integrated = match self.pipeline.integrate(&result) {
            Ok(integrated) => integrated,
            Err(error) => {
                let active = source_mutation_observation_for_protocol_error(
                    &result,
                    RuntimeStateObservedEnvelope::Version(1),
                );
                if let Err(state_error) = self.retain_recovery_mutation_evidence(&result) {
                    return self.finish_recovery_state_failure(state_error);
                }
                if let Err(state_error) =
                    self.abandon_recovery_source_mutation_for_reinspection(error.clone())
                {
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
                if let Err(error) = self.pipeline.resume_after_integration() {
                    return self
                        .finish_recovery_state_failure(RecoveryStateFailure::Pipeline(error));
                }
                if matches!(&integrated.request.kind, SourceMutationKind::Replace(_)) {
                    self.file_status = RuntimeUiFileStatus::Supported;
                }
                if let Err(error) = self.note_recovery_write_applied(purpose) {
                    return self.finish_recovery_state_failure(error);
                }
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
                    return self.finish_current_attempt_cancelled(Some(observation.clone()));
                }
                if matches!(
                    integrated.request.kind,
                    SourceMutationKind::ResetUnsupportedIfUnchanged { .. }
                ) {
                    let Some(recovery_path) = recovery_path else {
                        if let Err(error) = self.install_preserved_invalid_authority() {
                            return self.finish_recovery_state_failure(error);
                        }
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
                if let Err(error) = self.note_recovery_write_external() {
                    return self.finish_recovery_state_failure(error);
                }
                if cancel_requested {
                    if self.shutting_down {
                        return self.finish_recovery_shutdown();
                    }
                    return self.finish_current_attempt_cancelled(Some(active));
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
                if let Err(error) = self.note_recovery_write_external() {
                    return self.finish_recovery_state_failure(error);
                }
                if cancel_requested {
                    if self.shutting_down {
                        return self.finish_recovery_shutdown();
                    }
                    return self.finish_current_attempt_cancelled(Some(active));
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
                if let Err(state_error) = self.note_recovery_write_failed(purpose) {
                    return self.finish_recovery_state_failure(state_error);
                }
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

    pub(super) fn integrate_protocol_failure_source_completion(
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
        let (preserve_invalid, purpose) = match self.active_recovery.as_ref() {
            Some(ActiveRecoveryAttempt {
                kind: RecoveryAttemptKind::ConfirmPreserveInvalidResetInFlight { .. },
                current_command:
                    ActiveRecoveryCommand {
                        expected:
                            RecoveryCommandExpectation::SourceMutation {
                                kind: RecoverySourceMutationKind::PreserveInvalid,
                                ..
                            },
                        ..
                    },
                ..
            }) => (true, None),
            Some(ActiveRecoveryAttempt {
                current_command:
                    ActiveRecoveryCommand {
                        expected:
                            RecoveryCommandExpectation::SourceMutation {
                                kind: RecoverySourceMutationKind::PersistCanonical { purpose },
                                ..
                            },
                        ..
                    },
                ..
            }) => (false, Some(*purpose)),
            Some(_) => {
                return self
                    .finish_recovery_state_failure(RecoveryStateFailure::InvalidAttemptPhase);
            }
            None => {
                return self
                    .finish_recovery_state_failure(RecoveryStateFailure::MissingActiveAttempt);
            }
        };
        let active_observation = source_mutation_observation_for_protocol_error(
            &result,
            if preserve_invalid {
                RuntimeStateObservedEnvelope::PresentWithoutReadableVersion
            } else {
                RuntimeStateObservedEnvelope::Version(1)
            },
        );
        let retained = if preserve_invalid {
            self.retain_recovery_mutation_evidence_for_kind(
                &result,
                &RecoverySourceMutationKind::PreserveInvalid,
            )
        } else {
            self.retain_recovery_mutation_evidence(&result)
        };
        if let Err(error) = retained {
            return self.finish_recovery_state_failure(error);
        }

        if !preserve_invalid {
            match self.pipeline.integrate(&result) {
                Ok(integrated) => match &result {
                    SourceMutationResult::Applied { .. } => {
                        if matches!(&integrated.request.kind, SourceMutationKind::Replace(_)) {
                            self.file_status = RuntimeUiFileStatus::Supported;
                        }
                        if let Err(error) = self.note_recovery_write_applied(purpose) {
                            return self.finish_recovery_state_failure(error);
                        }
                    }
                    SourceMutationResult::SourceChangedBeforeMutation { .. }
                    | SourceMutationResult::ObservationChangedAfterClaim { .. } => {
                        if let Err(error) = self.note_recovery_write_external() {
                            return self.finish_recovery_state_failure(error);
                        }
                    }
                    SourceMutationResult::Failed { error, .. } => {
                        self.pipeline
                            .settle_failed(integrated.covered.iter().copied(), error.clone());
                        if let Err(state_error) = self.note_recovery_write_failed(purpose) {
                            return self.finish_recovery_state_failure(state_error);
                        }
                    }
                },
                Err(error) => {
                    if let Err(state_error) =
                        self.abandon_recovery_source_mutation_for_reinspection(error)
                    {
                        return self.finish_recovery_state_failure(state_error);
                    }
                }
            }
        }

        if cancel_requested {
            if self.shutting_down {
                return self.finish_recovery_shutdown();
            }
            return self.finish_current_attempt_cancelled(active_observation);
        }

        match self.dispatch_protocol_failure_reinspection() {
            Ok(command_id) => SubmitPersistenceRecoveryResult::Continue {
                dispatched: command_id,
            },
            Err(error) => self.finish_recovery_state_failure(error),
        }
    }

    pub(super) fn dispatch_external_authority_reinspection(
        &mut self,
        writer_observation: RuntimeStateSourceObservation,
        path_effect: RuntimeStateObservedPathEffect,
    ) -> SubmitPersistenceRecoveryResult {
        if let Err(error) = self.validate_active_recovery_state() {
            return self.finish_recovery_state_failure(error);
        }
        let Some(command_id) =
            allocate_counter(&mut self.next_recovery_command_id).map(RecoveryCommandId)
        else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::CommandIdExhausted);
        };
        let Some(active) = self.active_recovery.as_mut() else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::MissingActiveAttempt);
        };
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
        let Some(barrier) = &mut self.active_barrier else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::MissingBarrier);
        };
        barrier.phase = ControllerBarrierPhase::Reinspecting;
        SubmitPersistenceRecoveryResult::Continue {
            dispatched: command_id,
        }
    }

    pub(super) fn note_recovery_write_applied(
        &mut self,
        purpose: Option<RecoveryCanonicalWritePurpose>,
    ) -> Result<(), RecoveryStateFailure> {
        let retained_authority = self.capture_persistence_authority();
        let Some(incident) = &mut self.incident else {
            return Err(RecoveryStateFailure::MissingIncident);
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
        Ok(())
    }

    pub(super) fn note_recovery_write_failed(
        &mut self,
        purpose: Option<RecoveryCanonicalWritePurpose>,
    ) -> Result<(), RecoveryStateFailure> {
        let Some(incident) = &mut self.incident else {
            return Err(RecoveryStateFailure::MissingIncident);
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
        Ok(())
    }

    pub(super) fn note_recovery_write_external(&mut self) -> Result<(), RecoveryStateFailure> {
        let Some(incident) = &mut self.incident else {
            return Err(RecoveryStateFailure::MissingIncident);
        };
        incident.held_replacements.clear();
        incident.retry_desired_through = None;
        incident.cleanup = RecoveryCleanupState::Clean;
        Ok(())
    }

    pub(super) fn continue_after_recovery_write(
        &mut self,
        observation: RuntimeStateSourceObservation,
    ) -> SubmitPersistenceRecoveryResult {
        let Some(incident) = self.incident.as_ref() else {
            return self.finish_recovery_state_failure(RecoveryStateFailure::MissingIncident);
        };
        let needs_recompute = incident.staged_reload.is_some()
            || matches!(incident.cleanup, RecoveryCleanupState::NeedsRecompute);
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
