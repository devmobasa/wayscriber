use super::*;

impl RuntimeUiStateController {
    pub(crate) fn take_source_mutation(&mut self) -> Option<SourceMutationRequest> {
        self.pipeline.take_outbound()
    }

    pub(crate) fn submit_source_mutation(
        &mut self,
        result: SourceMutationResult,
    ) -> SubmitSourceMutationResult {
        self.drain_lifecycle_controls();
        let integrated = match self.pipeline.integrate(result) {
            Ok(integrated) => integrated,
            Err(error) => return SubmitSourceMutationResult::Rejected(error),
        };

        match &integrated.result {
            SourceMutationResult::Applied {
                recovery_artifacts, ..
            } => {
                let recovery_artifacts = recovery_artifacts.clone();
                if matches!(&integrated.request.kind, SourceMutationKind::Replace(_)) {
                    self.file_status = RuntimeUiFileStatus::Supported;
                }
                if matches!(
                    &integrated.request.kind,
                    SourceMutationKind::ResetSupported { .. }
                        | SourceMutationKind::ResetUnsupportedIfUnchanged { .. }
                ) {
                    return self.finish_supported_reset_success(integrated);
                }
                if self.supported_reset.is_some() {
                    self.capture_reset_authority_after_prerequisite();
                }
                if let Err(error) = self.pipeline.resume_after_integration() {
                    return SubmitSourceMutationResult::Rejected(error);
                }
                self.refresh_reset_barrier_phase();
                if matches!(
                    self.active_barrier
                        .as_ref()
                        .map(|barrier| &barrier.operation),
                    Some(ControllerBarrierOperation::ExternalAuthorityReconciliation)
                ) && !self.pipeline.has_source_mutation_in_flight()
                    && self.pipeline.pending_replacements() == 0
                {
                    if self.shutting_down {
                        self.settle_external_reconciliation_for_shutdown();
                    } else if let Err(error) = self.finish_external_reconciliation_write() {
                        return SubmitSourceMutationResult::Rejected(error);
                    }
                }
                SubmitSourceMutationResult::Integrated { recovery_artifacts }
            }
            SourceMutationResult::SourceChangedBeforeMutation { active, .. } => {
                self.enter_external_reconciliation(
                    active.clone(),
                    Vec::new(),
                    RuntimeStateObservedPathEffect::Untouched,
                );
                let barrier = self.active_barrier.as_ref().expect("barrier installed").id;
                if self.shutting_down {
                    self.settle_external_reconciliation_for_shutdown();
                    return SubmitSourceMutationResult::ExternalReconciliationSettledForShutdown {
                        barrier,
                        active: active.clone(),
                        recovery_artifacts: Vec::new(),
                        path_effect: RuntimeStateObservedPathEffect::Untouched,
                    };
                }
                SubmitSourceMutationResult::ExternalReconciliationRequired {
                    barrier,
                    active: active.clone(),
                    recovery_artifacts: Vec::new(),
                    path_effect: RuntimeStateObservedPathEffect::Untouched,
                }
            }
            SourceMutationResult::ObservationChangedAfterClaim {
                active,
                recovery_artifacts,
                path_effect,
                ..
            } => {
                let observed_effect =
                    RuntimeStateObservedPathEffect::PostClaim(path_effect.clone());
                self.enter_external_reconciliation(
                    active.clone(),
                    recovery_artifacts.clone(),
                    observed_effect.clone(),
                );
                let barrier = self.active_barrier.as_ref().expect("barrier installed").id;
                if self.shutting_down {
                    self.settle_external_reconciliation_for_shutdown();
                    return SubmitSourceMutationResult::ExternalReconciliationSettledForShutdown {
                        barrier,
                        active: active.clone(),
                        recovery_artifacts: recovery_artifacts.clone(),
                        path_effect: observed_effect,
                    };
                }
                SubmitSourceMutationResult::ExternalReconciliationRequired {
                    barrier,
                    active: active.clone(),
                    recovery_artifacts: recovery_artifacts.clone(),
                    path_effect: observed_effect,
                }
            }
            SourceMutationResult::Failed {
                error,
                active,
                recovery_artifacts,
                path_effect,
                ..
            } => {
                let failed_replacement = match &integrated.request.kind {
                    SourceMutationKind::Replace(snapshot) => Some(HeldReplacementStage {
                        snapshot: snapshot.clone(),
                        through: integrated.request.accepted_through,
                        covered: integrated.covered.clone(),
                        authority_epoch: integrated.request.expected_epoch,
                    }),
                    SourceMutationKind::ResetSupported { .. }
                    | SourceMutationKind::ResetUnsupportedIfUnchanged { .. } => {
                        self.pipeline
                            .settle_failed(integrated.covered.iter().copied(), error.clone());
                        None
                    }
                };
                let incident = self.enter_persistence_incident(
                    error.clone(),
                    active.clone(),
                    recovery_artifacts.clone(),
                    path_effect.clone(),
                    failed_replacement,
                );
                let barrier = self.active_barrier.as_ref().expect("barrier installed").id;
                if self.shutting_down {
                    self.settle_incident_for_shutdown();
                    if let Err(error) = self.pipeline.resume_after_integration() {
                        return SubmitSourceMutationResult::Rejected(error);
                    }
                    return SubmitSourceMutationResult::PersistenceFailureSettledForShutdown {
                        barrier,
                        error: error.clone(),
                        active: active.clone(),
                        recovery_artifacts: recovery_artifacts.clone(),
                        path_effect: path_effect.clone(),
                    };
                }
                SubmitSourceMutationResult::PersistenceUnhealthy {
                    barrier,
                    incident,
                    error: error.clone(),
                    active: active.clone(),
                    recovery_artifacts: recovery_artifacts.clone(),
                    path_effect: path_effect.clone(),
                }
            }
        }
    }

    pub(crate) fn install_external_authority(
        &mut self,
        barrier: ControllerBarrierId,
        observation: RuntimeStateSourceObservation,
        file_status: RuntimeUiFileStatus,
        model: RuntimeUiModel,
        passthrough: WirePassthrough,
    ) -> Result<ExternalAuthorityInstallResult, ExternalAuthorityInstallError> {
        if self.shutting_down {
            return Err(ExternalAuthorityInstallError::ShuttingDown);
        }
        if self.external_reconciliation.is_none() {
            return Err(ExternalAuthorityInstallError::NoReconciliationPending);
        }
        if !matches!(
            self.active_barrier.as_ref(),
            Some(active)
                if active.id == barrier
                    && active.operation
                        == ControllerBarrierOperation::ExternalAuthorityReconciliation
        ) {
            return Err(ExternalAuthorityInstallError::WrongBarrier);
        }
        if !observation.is_consistent() {
            return Err(ExternalAuthorityInstallError::InconsistentObservation);
        }
        if matches!(
            observation.envelope,
            RuntimeStateObservedEnvelope::PresentWithoutReadableVersion
        ) {
            let evidence = self
                .external_reconciliation
                .take()
                .expect("external reconciliation validated above");
            self.pipeline.install_acknowledged_authority(
                observation.revision.clone(),
                RuntimeUiWireState::default(),
            );
            self.file_status = RuntimeUiFileStatus::Invalid;
            self.model.clear();
            self.passthrough = WirePassthrough::default();
            self.live_only_overlay.clear();
            self.rebuild_live_state();
            let incident = self.enter_persistence_incident(
                RuntimeStateIoError::new(
                    "external runtime-state authority is malformed or unreadable",
                ),
                Some(observation),
                evidence.recovery_artifacts,
                RuntimeStateFailurePathEffect::Known(evidence.path_effect),
                None,
            );
            return Err(ExternalAuthorityInstallError::InvalidAuthority { incident });
        }
        if !file_status_matches_observation(&file_status, &observation.envelope) {
            return Err(ExternalAuthorityInstallError::FileStatusMismatch);
        }
        if !matches!(file_status, RuntimeUiFileStatus::Supported)
            && (!model.is_empty() || !passthrough.is_empty())
        {
            return Err(ExternalAuthorityInstallError::UnexpectedDecodedAuthority);
        }
        let retains_live_only_authority =
            matches!(
                self.file_status,
                RuntimeUiFileStatus::UnsupportedReadOnly { .. }
            ) && matches!(file_status, RuntimeUiFileStatus::UnsupportedReadOnly { .. });
        let next_epoch = if retains_live_only_authority {
            self.authority_epoch
        } else {
            self.authority_epoch
                .checked_add(1)
                .ok_or(ExternalAuthorityInstallError::AuthorityEpochExhausted)?
        };
        let observed_wire = RuntimeUiWireState {
            model: model.clone(),
            passthrough: passthrough.clone(),
        };
        let (next_seeds, changed_targets, tombstoned_targets) = self
            .staged_reload
            .as_ref()
            .cloned()
            .map(StagedSeedReload::into_parts)
            .unwrap_or_else(|| (self.seeds.clone(), BTreeSet::new(), BTreeSet::new()));
        let mut canonical_model = model;
        canonical_model.remove_targets(&tombstoned_targets);
        canonical_model.reconcile(&next_seeds);
        let mut canonical_passthrough = passthrough.clone();
        canonical_passthrough.reconcile_entries(&canonical_model);
        let canonical_wire = RuntimeUiWireState {
            model: canonical_model.clone(),
            passthrough: canonical_passthrough.clone(),
        };
        let needs_cleanup = matches!(file_status, RuntimeUiFileStatus::Supported)
            && canonical_wire != observed_wire;
        if needs_cleanup {
            self.pipeline
                .preflight_accept_replace()
                .map_err(ExternalAuthorityInstallError::Persistence)?;
        }

        self.staged_reload = None;
        self.seeds = next_seeds;
        self.file_status = file_status;
        self.model = canonical_model;
        self.passthrough = canonical_passthrough;
        self.pipeline
            .install_acknowledged_authority(observation.revision.clone(), observed_wire);
        if retains_live_only_authority {
            self.live_only_overlay.reconcile(&changed_targets);
        } else {
            self.live_only_overlay.clear();
        }
        self.authority_epoch = next_epoch;
        if let Some(transaction) = self.supported_reset.take() {
            self.pipeline
                .settle_held_external(&transaction.held_by_reset);
        }
        self.rebuild_live_state();
        let cleanup = needs_cleanup.then(|| {
            self.pipeline
                .accept_replace(canonical_wire, self.authority_epoch)
                .expect("preflighted external-authority cleanup must dispatch")
        });
        if cleanup.is_none() {
            if let Some(barrier) = self.active_barrier.as_ref().map(|barrier| barrier.id) {
                self.close_barrier_and_resolve_previews_after_seed_changes(
                    barrier,
                    if retains_live_only_authority {
                        AbandonedPreviewResolutionReason::CancelledUnderRetainedAuthority
                    } else {
                        AbandonedPreviewResolutionReason::DiscardedForAuthorityChange
                    },
                    Some(&changed_targets),
                );
            }
        } else if let Some(barrier) = &mut self.active_barrier {
            barrier.phase = ControllerBarrierPhase::Writing(
                self.pipeline
                    .source_mutation_in_flight()
                    .expect("cleanup dispatched")
                    .id,
            );
        }
        let evidence = self
            .external_reconciliation
            .take()
            .expect("reconciliation evidence validated before authority installation");
        Ok(ExternalAuthorityInstallResult {
            cleanup_through: cleanup,
            evidence,
        })
    }
}
