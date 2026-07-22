use super::*;

impl RuntimeUiStateController {
    pub(super) fn apply_incident_staged_reload(&mut self) {
        let staged = self
            .incident
            .as_mut()
            .and_then(|incident| incident.staged_reload.take())
            .or_else(|| self.staged_reload.take());
        if let Some(staged) = staged {
            let (seeds, changed) = staged.into_parts();
            if let Some(incident) = &mut self.incident {
                incident
                    .applied_reload_changed_targets
                    .extend(changed.iter().cloned());
            }
            self.seeds = seeds;
            self.live_only_overlay.reconcile(&changed);
            self.model.reconcile(&self.seeds);
            self.passthrough.reconcile_entries(&self.model);
            self.live_state =
                RuntimeUiLiveState::rebuild(&self.seeds, &self.model, &self.live_only_overlay);
        }
    }

    pub(super) fn finish_recovered(
        &mut self,
        observation: RuntimeStateSourceObservation,
    ) -> SubmitPersistenceRecoveryResult {
        self.apply_incident_staged_reload();
        if let Some(incident) = &self.incident {
            self.pipeline
                .settle_held_persisted(&incident.held_replacements, &observation.revision);
        }
        if let Err(error) = self.pipeline.resume_after_integration() {
            return self.finish_still_unhealthy(
                RuntimeStateIoError::new(format!("failed to settle recovery waiters: {error:?}")),
                Some(observation),
            );
        }
        let incident = self.incident.take().expect("active incident");
        self.close_barrier_and_resolve_previews_after_seed_changes(
            incident.barrier,
            AbandonedPreviewResolutionReason::CancelledUnderRetainedAuthority,
            Some(&incident.applied_reload_changed_targets),
        );
        let result = PersistenceRecoveryResult::Recovered {
            incident: incident.id,
            final_source: observation,
            settled_through: self.pipeline.settled_through(),
            evidence: evidence(&incident),
        };
        self.deliver_terminal(result)
    }

    pub(super) fn finish_current_recovery_success(
        &mut self,
        observation: RuntimeStateSourceObservation,
    ) -> SubmitPersistenceRecoveryResult {
        let external = self.active_recovery.as_ref().and_then(|active| {
            if let RecoveryAttemptKind::ExternalAuthorityCleanup {
                writer_observation,
                authority,
                path_effect,
            } = &active.kind
            {
                Some((
                    writer_observation.clone(),
                    authority.clone(),
                    path_effect.clone(),
                ))
            } else {
                None
            }
        });
        if let Some((writer_observation, authority, path_effect)) = external {
            self.finish_external_authority_installed(writer_observation, authority, path_effect)
        } else {
            self.finish_recovered(observation)
        }
    }

    pub(super) fn install_recovery_external_authority(
        &mut self,
        inspection: RecoveryInspection,
        writer_observation: Option<RuntimeStateSourceObservation>,
        path_effect: RuntimeStateObservedPathEffect,
    ) -> SubmitPersistenceRecoveryResult {
        let observation = inspection.observation;
        if !observation.is_consistent() {
            return self.finish_still_unhealthy(
                RuntimeStateIoError::new(
                    "runtime-state inspection returned an inconsistent observation",
                ),
                Some(observation),
            );
        }
        if !matches!(
            observation.envelope,
            RuntimeStateObservedEnvelope::Version(1)
        ) && inspection.supported_wire.is_some()
        {
            return self.finish_still_unhealthy(
                RuntimeStateIoError::new(
                    "runtime-state inspection decoded a non-V1 authority as V1",
                ),
                Some(observation),
            );
        }
        let (file_status, acknowledged_wire) = match &observation.envelope {
            RuntimeStateObservedEnvelope::Missing => {
                (RuntimeUiFileStatus::Missing, RuntimeUiWireState::default())
            }
            RuntimeStateObservedEnvelope::Version(1) => {
                let Some(wire) = inspection.supported_wire else {
                    return self.finish_still_unhealthy(
                        RuntimeStateIoError::new(
                            "supported runtime-state inspection omitted decoded authority",
                        ),
                        Some(observation),
                    );
                };
                (RuntimeUiFileStatus::Supported, wire)
            }
            RuntimeStateObservedEnvelope::Version(version) => (
                RuntimeUiFileStatus::UnsupportedReadOnly {
                    version: Some(*version),
                },
                RuntimeUiWireState::default(),
            ),
            RuntimeStateObservedEnvelope::PresentWithoutReadableVersion => {
                return self.finish_still_unhealthy(
                    RuntimeStateIoError::new("observed runtime state remains invalid"),
                    Some(observation),
                );
            }
        };

        let abandoned_cleanup = self.incident.as_ref().and_then(|incident| {
            if let RecoveryCleanupState::Pending { through } = incident.cleanup {
                Some(through)
            } else {
                None
            }
        });
        if let Some(through) = abandoned_cleanup {
            self.pipeline.settle_external([through]);
        }
        if let Some(incident) = &mut self.incident {
            self.pipeline
                .settle_held_external(&incident.held_replacements);
            incident.held_replacements.clear();
            incident.retry_desired_through = None;
            incident.cleanup = RecoveryCleanupState::Clean;
        }
        self.pipeline.discard_pending_for_external_authority();
        self.pipeline.install_acknowledged_authority(
            observation.revision.clone(),
            acknowledged_wire.clone(),
        );
        self.model = acknowledged_wire.model;
        self.passthrough = acknowledged_wire.passthrough;
        self.live_only_overlay.clear();
        self.file_status = file_status;
        self.authority_epoch = self
            .authority_epoch
            .checked_add(1)
            .expect("authority epoch exhausted");
        self.apply_incident_staged_reload();
        if (self.model.reconcile(&self.seeds) | self.passthrough.reconcile_entries(&self.model))
            && matches!(self.file_status, RuntimeUiFileStatus::Supported)
            && let Some(incident) = &mut self.incident
        {
            incident.cleanup.mark_recompute();
        }
        self.live_state =
            RuntimeUiLiveState::rebuild(&self.seeds, &self.model, &self.live_only_overlay);

        if matches!(self.file_status, RuntimeUiFileStatus::Supported)
            && self.canonical_recovery_wire() != *self.pipeline.acknowledged_wire()
        {
            self.active_recovery.as_mut().expect("active attempt").kind =
                RecoveryAttemptKind::ExternalAuthorityCleanup {
                    writer_observation,
                    authority: observation.clone(),
                    path_effect,
                };
            if let Some(incident) = &mut self.incident
                && matches!(incident.cleanup, RecoveryCleanupState::Clean)
            {
                incident.cleanup = RecoveryCleanupState::NeedsRecompute;
            }
            return self.dispatch_canonical_recovery(observation);
        }

        self.finish_external_authority_installed(writer_observation, observation, path_effect)
    }

    pub(super) fn finish_external_authority_installed(
        &mut self,
        writer_observation: Option<RuntimeStateSourceObservation>,
        authority: RuntimeStateSourceObservation,
        path_effect: RuntimeStateObservedPathEffect,
    ) -> SubmitPersistenceRecoveryResult {
        let incident = self.incident.take().expect("active incident");
        self.close_barrier_and_resolve_previews(
            incident.barrier,
            AbandonedPreviewResolutionReason::DiscardedForAuthorityChange,
        );
        let result = PersistenceRecoveryResult::ExternalAuthorityInstalled {
            incident: incident.id,
            writer_observation,
            authority,
            evidence: evidence(&incident),
            path_effect,
        };
        self.deliver_terminal(result)
    }

    pub(super) fn canonical_recovery_wire(&self) -> RuntimeUiWireState {
        RuntimeUiWireState {
            model: self.model.clone(),
            passthrough: self.passthrough.clone(),
        }
    }

    pub(super) fn finish_confirmation_required(
        &mut self,
        observed: RuntimeStateSourceObservation,
    ) -> SubmitPersistenceRecoveryResult {
        self.apply_incident_staged_reload();
        let (incident_id, barrier_id) = {
            let attempt = self.active_recovery.as_ref().expect("active attempt");
            (attempt.incident, attempt.barrier)
        };
        let next_handle = self.rotate_and_checkout_handle();
        let confirmation = InvalidStateResetConfirmation {
            controller: self.id,
            incident: incident_id,
            barrier: barrier_id,
            handle: next_handle.handle_id,
            revision: observed.revision.clone(),
            envelope: observed.envelope.clone(),
        };
        let result = PersistenceRecoveryResult::RequiresInvalidResetConfirmation {
            recovery: next_handle,
            observed,
            confirmation,
            evidence: evidence(self.incident.as_ref().expect("incident")),
        };
        if let Some(barrier) = &mut self.active_barrier {
            barrier.phase = ControllerBarrierPhase::PersistenceUnhealthy {
                incident: incident_id,
            };
        }
        self.deliver_terminal(result)
    }

    pub(super) fn finish_observation_changed(
        &mut self,
        confirmed_revision: RuntimeStateSourceRevision,
        active: RuntimeStateSourceObservation,
        path_effect: RuntimeStateObservedPathEffect,
    ) -> SubmitPersistenceRecoveryResult {
        self.apply_incident_staged_reload();
        let incident_id = self.incident.as_ref().expect("incident").id;
        let recovery = self.rotate_and_checkout_handle();
        let confirmed = RuntimeStateSourceObservation {
            revision: confirmed_revision,
            envelope: RuntimeStateObservedEnvelope::PresentWithoutReadableVersion,
        };
        let result = PersistenceRecoveryResult::ObservationChanged {
            recovery,
            confirmed,
            active,
            evidence: evidence(self.incident.as_ref().expect("incident")),
            path_effect,
        };
        if let Some(barrier) = &mut self.active_barrier {
            barrier.phase = ControllerBarrierPhase::PersistenceUnhealthy {
                incident: incident_id,
            };
        }
        self.deliver_terminal(result)
    }

    pub(super) fn finish_preserved_invalid(
        &mut self,
        observation: RuntimeStateSourceObservation,
        recovery_path: std::path::PathBuf,
    ) -> SubmitPersistenceRecoveryResult {
        self.install_preserved_invalid_authority();
        let incident = self.incident.take().expect("incident");
        self.close_barrier_and_resolve_previews(
            incident.barrier,
            AbandonedPreviewResolutionReason::DiscardedForAuthorityChange,
        );
        let result = PersistenceRecoveryResult::InvalidSourcePreservedAndReset {
            incident: incident.id,
            new_source: observation,
            evidence: evidence(&incident),
            path_effect: RuntimeStatePostClaimPathEffect::QuarantinedAndRetained { recovery_path },
        };
        self.deliver_terminal(result)
    }

    pub(super) fn install_preserved_invalid_authority(&mut self) {
        self.apply_incident_staged_reload();
        let abandoned_cleanup = self.incident.as_ref().and_then(|incident| {
            if let RecoveryCleanupState::Pending { through } = incident.cleanup {
                Some(through)
            } else {
                None
            }
        });
        if let Some(through) = abandoned_cleanup {
            self.pipeline.settle_external([through]);
        }
        let held = self
            .incident
            .as_mut()
            .map(|incident| {
                incident.retry_desired_through = None;
                incident.cleanup = RecoveryCleanupState::Clean;
                std::mem::take(&mut incident.held_replacements)
            })
            .unwrap_or_default();
        self.pipeline.settle_held_external(&held);
        self.pipeline.discard_pending_for_external_authority();
        self.model.clear();
        self.passthrough = WirePassthrough::default();
        self.live_only_overlay.clear();
        self.file_status = RuntimeUiFileStatus::Missing;
        self.authority_epoch = self
            .authority_epoch
            .checked_add(1)
            .expect("epoch exhausted");
        self.live_state =
            RuntimeUiLiveState::rebuild(&self.seeds, &self.model, &self.live_only_overlay);
    }

    pub(super) fn finish_still_unhealthy(
        &mut self,
        error: RuntimeStateIoError,
        active: Option<RuntimeStateSourceObservation>,
    ) -> SubmitPersistenceRecoveryResult {
        let active = self.retain_or_fallback_active(active);
        self.apply_incident_staged_reload();
        let attempt = self
            .active_recovery
            .as_ref()
            .map(|attempt| attempt.id)
            .unwrap_or(RecoveryAttemptId(0));
        let recovery = self.rotate_and_checkout_handle();
        if let Some(barrier) = &mut self.active_barrier {
            barrier.phase = ControllerBarrierPhase::PersistenceUnhealthy {
                incident: self.incident.as_ref().expect("incident").id,
            };
        }
        let result = PersistenceRecoveryResult::StillUnhealthy {
            recovery,
            attempt,
            error,
            active,
            evidence: evidence(self.incident.as_ref().expect("incident")),
        };
        self.deliver_terminal(result)
    }

    pub(super) fn finish_cancelled_attempt(
        &mut self,
        active: Option<RuntimeStateSourceObservation>,
    ) {
        let active = self.retain_or_fallback_active(active);
        self.apply_incident_staged_reload();
        let (attempt_id, incident_id) = {
            let attempt = self.active_recovery.as_ref().expect("active attempt");
            (attempt.id, attempt.incident)
        };
        let recovery = self.rotate_and_checkout_handle();
        let result = PersistenceRecoveryResult::Cancelled {
            recovery,
            attempt: attempt_id,
            active,
            evidence: evidence(self.incident.as_ref().expect("incident")),
        };
        if let Some(barrier) = &mut self.active_barrier {
            barrier.phase = ControllerBarrierPhase::PersistenceUnhealthy {
                incident: incident_id,
            };
        }
        let _ = self.deliver_terminal(result);
    }

    pub(super) fn retain_or_fallback_active(
        &mut self,
        active: Option<RuntimeStateSourceObservation>,
    ) -> Option<RuntimeStateSourceObservation> {
        if let Some(observation) = active
            .as_ref()
            .filter(|observation| observation.is_consistent())
            && let Some(incident) = &mut self.incident
        {
            incident.last_safe_active = Some(observation.clone());
        }
        active
            .filter(RuntimeStateSourceObservation::is_consistent)
            .or_else(|| {
                self.incident
                    .as_ref()
                    .and_then(|incident| incident.last_safe_active.clone())
            })
    }

    pub(in crate::runtime_ui_state) fn settle_incident_for_shutdown(
        &mut self,
    ) -> Option<PersistenceIncident> {
        let incident = self.incident.take()?;
        let error = RuntimeStateIoError::new("runtime-state persistence shut down");
        self.pipeline
            .settle_held_failed(&incident.held_replacements, error.clone());
        if let RecoveryCleanupState::Pending { through }
        | RecoveryCleanupState::InFlight { through, .. } = incident.cleanup
        {
            self.pipeline.settle_failed([through], error);
        }
        self.close_barrier_and_resolve_previews(
            incident.barrier,
            AbandonedPreviewResolutionReason::CancelledUnderRetainedAuthority,
        );
        Some(incident)
    }

    pub(super) fn finish_recovery_shutdown(&mut self) -> SubmitPersistenceRecoveryResult {
        let active = self.active_recovery.take();
        let attempt = active
            .as_ref()
            .map_or(RecoveryAttemptId(0), |active| active.id);
        let incident = self
            .settle_incident_for_shutdown()
            .expect("active recovery owns an incident");
        if let Some(active) = active {
            let result = PersistenceRecoveryResult::Shutdown {
                incident: incident.id,
                evidence: evidence(&incident),
            };
            if let Err(undelivered) = active.completion.send(result) {
                drop(undelivered.0);
            }
        }
        let _ = self.pipeline.request_shutdown();
        SubmitPersistenceRecoveryResult::Terminal { attempt }
    }
}
