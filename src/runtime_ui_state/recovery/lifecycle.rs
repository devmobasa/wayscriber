use super::*;

impl RuntimeUiStateController {
    const RECOVERY_COMMAND_TOMBSTONE_LIMIT: usize = 1024;

    pub(super) fn record_integrated_recovery_command(&mut self, command: RecoveryCommandId) {
        if self.integrated_recovery_commands.insert(command) {
            self.integrated_recovery_command_order.push_back(command);
        }
        while self.integrated_recovery_commands.len() > Self::RECOVERY_COMMAND_TOMBSTONE_LIMIT {
            let expired = self
                .integrated_recovery_command_order
                .pop_front()
                .expect("integrated command order tracks every tombstone");
            self.integrated_recovery_commands.remove(&expired);
        }
    }

    pub(super) fn record_cancelled_read_only_command(&mut self, command: RecoveryCommandId) {
        if self.cancelled_read_only_commands.insert(command) {
            self.cancelled_read_only_command_order.push_back(command);
        }
        while self.cancelled_read_only_commands.len() > Self::RECOVERY_COMMAND_TOMBSTONE_LIMIT {
            let expired = self
                .cancelled_read_only_command_order
                .pop_front()
                .expect("cancelled command order tracks every tombstone");
            self.cancelled_read_only_commands.remove(&expired);
        }
    }

    pub(super) fn record_rejected_recovery_completion(&mut self, completion: RecoveryIoCompletion) {
        if self
            .rejected_recovery_completions
            .iter()
            .any(|recorded| recorded == &completion)
        {
            return;
        }
        self.rejected_recovery_completions.push_back(completion);
        while self.rejected_recovery_completions.len() > Self::RECOVERY_COMMAND_TOMBSTONE_LIMIT {
            self.rejected_recovery_completions.pop_front();
        }
    }

    pub(super) fn take_cancelled_read_only_command(&mut self, command: RecoveryCommandId) -> bool {
        if !self.cancelled_read_only_commands.remove(&command) {
            return false;
        }
        self.cancelled_read_only_command_order
            .retain(|candidate| *candidate != command);
        true
    }

    pub(in crate::runtime_ui_state) fn prepare_recovery_shutdown(&mut self) -> bool {
        let Some(active) = self.active_recovery.as_ref() else {
            self.settle_incident_for_shutdown();
            return true;
        };
        match active.current_command.expected {
            RecoveryCommandExpectation::Inspection => {
                let command = active.current_command.id;
                self.record_cancelled_read_only_command(command);
                self.finish_recovery_shutdown();
                true
            }
            RecoveryCommandExpectation::SourceMutation { .. } => {
                self.active_recovery
                    .as_mut()
                    .expect("active recovery was just observed")
                    .cancel_requested = true;
                false
            }
        }
    }

    pub(crate) fn checkout_persistence_recovery_handle(
        &mut self,
        incident_id: PersistenceIncidentId,
    ) -> CheckoutPersistenceRecoveryHandleResult {
        self.drain_lifecycle_controls();
        let Some(incident) = self.incident.as_mut() else {
            return CheckoutPersistenceRecoveryHandleResult::RejectedNotUnhealthy;
        };
        if incident.id != incident_id {
            return CheckoutPersistenceRecoveryHandleResult::RejectedWrongControllerOrIncident;
        }
        if !matches!(
            incident.handle.availability,
            RecoveryHandleAvailability::Available
        ) {
            return CheckoutPersistenceRecoveryHandleResult::AlreadyCheckedOut;
        }
        let Some(lease) =
            allocate_counter(&mut self.next_recovery_lease_nonce).map(RecoveryLeaseNonce)
        else {
            return CheckoutPersistenceRecoveryHandleResult::AlreadyCheckedOut;
        };
        incident.handle.availability = RecoveryHandleAvailability::CheckedOut(lease);
        CheckoutPersistenceRecoveryHandleResult::CheckedOut(PersistenceRecoveryHandle {
            controller_id: self.id,
            incident: incident.id,
            barrier: incident.barrier,
            handle_id: incident.handle.id,
            lease,
            lifecycle: self.lifecycle_tx.clone(),
            armed: true,
        })
    }

    pub(crate) fn begin_persistence_recovery(
        &mut self,
        mut request: PersistenceRecoveryRequest,
    ) -> BeginPersistenceRecoveryResult {
        self.drain_lifecycle_controls();
        let rejection = self.validate_recovery_request(&request);
        if let Some(reason) = rejection {
            return BeginPersistenceRecoveryResult::Rejected { request, reason };
        }

        let attempt_id = RecoveryAttemptId(
            allocate_counter(&mut self.next_recovery_attempt_id)
                .expect("recovery attempt id exhausted"),
        );
        let command_id = RecoveryCommandId(
            allocate_counter(&mut self.next_recovery_command_id)
                .expect("recovery command id exhausted"),
        );
        request.recovery.disarm();
        let incident_id = request.recovery.incident;
        let barrier = request.recovery.barrier;
        let kind = match request.action {
            PersistenceRecoveryAction::RetryPending => RecoveryAttemptKind::RetryPending,
            PersistenceRecoveryAction::DiscardPendingAndAdoptObserved => {
                RecoveryAttemptKind::DiscardPendingAndAdoptObserved
            }
            PersistenceRecoveryAction::RequestPreserveInvalidReset => {
                RecoveryAttemptKind::RequestPreserveInvalidReset
            }
            PersistenceRecoveryAction::ConfirmPreserveInvalidReset(confirmation) => {
                RecoveryAttemptKind::ConfirmPreserveInvalidReset { confirmation }
            }
        };
        let (completion_tx, completion_rx) = std::sync::mpsc::channel();
        let active = ActiveRecoveryAttempt {
            id: attempt_id,
            incident: incident_id,
            barrier,
            kind,
            current_command: ActiveRecoveryCommand {
                id: command_id,
                expected: RecoveryCommandExpectation::Inspection,
            },
            protocol_failure_pending: false,
            cancel_requested: false,
            completion: completion_tx,
        };
        self.incident
            .as_mut()
            .expect("validated incident")
            .handle
            .availability = RecoveryHandleAvailability::InAttempt(attempt_id);
        self.active_recovery = Some(active);
        self.recovery_outbox.push_back(RecoveryIoCommand {
            controller_id: self.id,
            incident: incident_id,
            barrier,
            attempt: attempt_id,
            command_id,
            operation: RecoveryIoOperation::Inspect,
        });
        if let Some(active_barrier) = &mut self.active_barrier {
            active_barrier.phase = ControllerBarrierPhase::Recovering {
                incident: incident_id,
                attempt: attempt_id,
                step: RecoveryAttemptStep::Inspecting,
            };
        }
        let cancellation = RecoveryCancellation {
            controller_id: self.id,
            incident: incident_id,
            barrier,
            attempt: attempt_id,
            lifecycle: self.lifecycle_tx.clone(),
            armed: true,
        };
        BeginPersistenceRecoveryResult::Started {
            client: RecoveryAttemptClient {
                cancellation,
                completion: RecoveryCompletionReceiver {
                    receiver: completion_rx,
                },
            },
            dispatched: command_id,
        }
    }

    pub(crate) fn take_recovery_io_command(&mut self) -> Option<RecoveryIoCommand> {
        self.recovery_outbox.pop_front()
    }

    pub(crate) fn submit_persistence_recovery_io(
        &mut self,
        completion: RecoveryIoCompletion,
    ) -> SubmitPersistenceRecoveryResult {
        self.drain_lifecycle_controls();
        if completion.controller_id != self.id {
            return SubmitPersistenceRecoveryResult::RerouteWrongController { completion };
        }
        if self.take_cancelled_read_only_command(completion.command_id) {
            self.record_integrated_recovery_command(completion.command_id);
            return SubmitPersistenceRecoveryResult::IgnoredCancelledReadOnly {
                command_id: completion.command_id,
            };
        }
        if self
            .integrated_recovery_commands
            .contains(&completion.command_id)
        {
            return SubmitPersistenceRecoveryResult::IgnoredDuplicateAlreadyIntegrated {
                command_id: completion.command_id,
            };
        }
        if self
            .rejected_recovery_completions
            .iter()
            .any(|recorded| recorded == &completion)
        {
            return SubmitPersistenceRecoveryResult::IgnoredDuplicateAlreadyIntegrated {
                command_id: completion.command_id,
            };
        }
        let Some(active) = self.active_recovery.as_ref() else {
            return SubmitPersistenceRecoveryResult::RejectedNoActiveRecovery { completion };
        };
        if active.incident != completion.incident || active.barrier != completion.barrier {
            return self.block_protocol_failure(
                RecoveryCompletionProtocolError::WrongIncidentOrBarrier,
                completion,
            );
        }
        if active.id != completion.attempt {
            return self.block_protocol_failure(
                RecoveryCompletionProtocolError::UnknownAttempt,
                completion,
            );
        }
        if active.current_command.id != completion.command_id {
            return self.block_protocol_failure(
                RecoveryCompletionProtocolError::UnknownCommand,
                completion,
            );
        }
        let expectation = active.current_command.expected.clone();
        if let Some(reason) = completion_protocol_error(&completion.result, &expectation) {
            return self.block_protocol_failure(reason, completion);
        }
        let incident = active.incident;
        let attempt = active.id;
        let protocol_failure_pending = active.protocol_failure_pending;
        self.record_integrated_recovery_command(completion.command_id);
        if let Some(barrier) = &mut self.active_barrier {
            barrier.phase = ControllerBarrierPhase::Recovering {
                incident,
                attempt,
                step: RecoveryAttemptStep::AwaitingControllerDecision,
            };
        }
        match completion.result {
            RecoveryIoResult::Inspected(result) => self.integrate_recovery_inspection(result),
            RecoveryIoResult::SourceMutation(result) if protocol_failure_pending => {
                self.integrate_protocol_failure_source_completion(result)
            }
            RecoveryIoResult::SourceMutation(result) => {
                self.integrate_recovery_source_mutation(result)
            }
        }
    }

    pub(crate) fn cancel_persistence_recovery(
        &mut self,
        mut cancellation: RecoveryCancellation,
    ) -> CancelPersistenceRecoveryResult {
        self.drain_lifecycle_controls();
        if cancellation.controller_id != self.id {
            return CancelPersistenceRecoveryResult::RerouteWrongController { cancellation };
        }
        let Some(active) = self.active_recovery.as_mut() else {
            cancellation.disarm();
            return CancelPersistenceRecoveryResult::RejectedInert {
                reason: RecoveryCancellationRejection::UnknownOrCompletedAttempt,
            };
        };
        if active.incident != cancellation.incident
            || active.barrier != cancellation.barrier
            || active.id != cancellation.attempt
        {
            cancellation.disarm();
            return CancelPersistenceRecoveryResult::RejectedInert {
                reason: RecoveryCancellationRejection::WrongIncidentOrBarrier,
            };
        }
        cancellation.disarm();
        match active.current_command.expected {
            RecoveryCommandExpectation::Inspection => {
                let writer_observation = reinspection_writer_observation(&active.kind);
                let command = active.current_command.id;
                self.record_cancelled_read_only_command(command);
                self.finish_cancelled_attempt(writer_observation);
                CancelPersistenceRecoveryResult::Cancelled
            }
            RecoveryCommandExpectation::SourceMutation { .. } => {
                active.cancel_requested = true;
                if let Some(barrier) = &mut self.active_barrier {
                    barrier.phase = ControllerBarrierPhase::Recovering {
                        incident: active.incident,
                        attempt: active.id,
                        step: RecoveryAttemptStep::CancellationPending(active.current_command.id),
                    };
                }
                CancelPersistenceRecoveryResult::PendingIrrevocableIo {
                    attempt: active.id,
                    command_id: active.current_command.id,
                }
            }
        }
    }

    pub(in crate::runtime_ui_state) fn enter_persistence_incident(
        &mut self,
        error: RuntimeStateIoError,
        active: Option<RuntimeStateSourceObservation>,
        recovery_artifacts: Vec<RuntimeStateRecoveryArtifact>,
        path_effect: RuntimeStateFailurePathEffect,
        failed_replacement: Option<HeldReplacementStage>,
    ) -> PersistenceIncidentId {
        let barrier = match self.active_barrier.as_ref() {
            Some(barrier) => barrier.id,
            None => {
                let id = ControllerBarrierId(self.next_barrier_id);
                self.next_barrier_id = self
                    .next_barrier_id
                    .checked_add(1)
                    .expect("barrier id exhausted");
                self.active_barrier = Some(ActiveControllerBarrier {
                    id,
                    operation: ControllerBarrierOperation::PersistenceFailureRecovery,
                    phase: ControllerBarrierPhase::Inspecting,
                });
                id
            }
        };
        let id = PersistenceIncidentId(
            allocate_counter(&mut self.next_incident_id).expect("incident id exhausted"),
        );
        let handle_id = RecoveryHandleId(
            allocate_counter(&mut self.next_recovery_handle_id).expect("handle id exhausted"),
        );
        let held_replacements = if let Some(transaction) = self.supported_reset.take() {
            let _ = self.pipeline.cancel_pending_reset(
                transaction.through,
                DurabilityOutcome::Failed(error.clone()),
            );
            let mut held = self.pipeline.hold_all_pending_replacements();
            held.extend(transaction.held_by_reset);
            held
        } else {
            self.pipeline.hold_all_pending_replacements()
        };
        let failed_desired_through = failed_replacement.as_ref().map(|stage| stage.through);
        if let Some(failed_replacement) = &failed_replacement {
            self.pipeline
                .settle_held_failed(std::slice::from_ref(failed_replacement), error.clone());
        }
        let retry_desired_through = held_replacements
            .iter()
            .map(|stage| stage.through)
            .chain(failed_desired_through)
            .max();
        let retained_authority = PersistenceAuthoritySnapshot {
            expected_source: self.pipeline.stable_source().clone(),
            file_status: self.file_status.clone(),
            authority_epoch: self.authority_epoch,
            model: self.model.clone(),
            passthrough: self.passthrough.clone(),
            seeds: self.seeds.clone(),
            live_state: self.live_state.clone(),
        };
        let crossed_authority_change = matches!(
            self.active_barrier
                .as_ref()
                .map(|barrier| &barrier.operation),
            Some(ControllerBarrierOperation::ExternalAuthorityReconciliation)
        );
        let retained_authority_is_proven = self.staged_reload.is_none()
            && matches!(
                &path_effect,
                RuntimeStateFailurePathEffect::Known(RuntimeStateObservedPathEffect::Untouched)
            )
            && active.as_ref().is_some_and(|observation| {
                observation.is_consistent()
                    && observation.revision == retained_authority.expected_source
                    && observation_matches_file_status(
                        &retained_authority.file_status,
                        &observation.envelope,
                    )
            });
        let mut artifacts = Vec::new();
        merge_artifacts(&mut artifacts, recovery_artifacts);
        let last_safe_active = active.filter(RuntimeStateSourceObservation::is_consistent);
        let staged_reload = self.staged_reload.take();
        let cleanup = if staged_reload.is_some() {
            RecoveryCleanupState::NeedsRecompute
        } else {
            RecoveryCleanupState::Clean
        };
        self.incident = Some(PersistenceIncident {
            id,
            barrier,
            retained_authority,
            held_replacements,
            retry_desired_through,
            staged_reload,
            applied_reload_changed_targets: BTreeSet::new(),
            last_safe_active,
            cleanup,
            recovery_artifacts: artifacts,
            path_effect_history: vec![path_effect],
            handle: RecoveryHandleState {
                id: handle_id,
                availability: RecoveryHandleAvailability::Available,
            },
        });
        if let Some(active_barrier) = &mut self.active_barrier {
            active_barrier.phase = ControllerBarrierPhase::PersistenceUnhealthy { incident: id };
        }
        if retained_authority_is_proven {
            self.resolve_previews_while_barrier_retained(
                barrier,
                if crossed_authority_change {
                    AbandonedPreviewResolutionReason::DiscardedForAuthorityChange
                } else {
                    AbandonedPreviewResolutionReason::CancelledUnderRetainedAuthority
                },
                None,
            );
        }
        id
    }

    pub(in crate::runtime_ui_state) fn apply_lifecycle_control(
        &mut self,
        control: LifecycleControl,
    ) {
        match control {
            LifecycleControl::ReturnRecoveryHandle {
                controller,
                incident,
                barrier,
                handle,
                lease,
            } => {
                if controller != self.id {
                    return;
                }
                if let Some(current) = self.incident.as_mut()
                    && current.id == incident
                    && current.barrier == barrier
                    && current.handle.id == handle
                    && current.handle.availability == RecoveryHandleAvailability::CheckedOut(lease)
                {
                    current.handle.availability = RecoveryHandleAvailability::Available;
                }
            }
            LifecycleControl::CancelAttempt {
                controller,
                incident,
                barrier,
                attempt,
            } => {
                if controller != self.id {
                    return;
                }
                let Some(active) = self.active_recovery.as_ref() else {
                    return;
                };
                if active.incident != incident || active.barrier != barrier || active.id != attempt
                {
                    return;
                }
                if self.shutting_down {
                    return;
                }
                match active.current_command.expected {
                    RecoveryCommandExpectation::Inspection => {
                        let writer_observation = reinspection_writer_observation(&active.kind);
                        let command = active.current_command.id;
                        self.record_cancelled_read_only_command(command);
                        self.finish_cancelled_attempt(writer_observation);
                    }
                    RecoveryCommandExpectation::SourceMutation { .. } => {
                        if let Some(active) = self.active_recovery.as_mut() {
                            active.cancel_requested = true;
                            if let Some(barrier) = &mut self.active_barrier {
                                barrier.phase = ControllerBarrierPhase::Recovering {
                                    incident: active.incident,
                                    attempt: active.id,
                                    step: RecoveryAttemptStep::CancellationPending(
                                        active.current_command.id,
                                    ),
                                };
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn validate_recovery_request(
        &self,
        request: &PersistenceRecoveryRequest,
    ) -> Option<RecoveryBeginRejection> {
        if self.shutting_down {
            return Some(RecoveryBeginRejection::ShuttingDown);
        }
        if request.recovery.controller_id != self.id {
            return Some(RecoveryBeginRejection::WrongController);
        }
        let Some(incident) = &self.incident else {
            return Some(RecoveryBeginRejection::NotUnhealthy);
        };
        if self.active_recovery.is_some() {
            return Some(RecoveryBeginRejection::AttemptAlreadyRunning);
        }
        if incident.id != request.recovery.incident
            || incident.barrier != request.recovery.barrier
            || incident.handle.id != request.recovery.handle_id
            || incident.handle.availability
                != RecoveryHandleAvailability::CheckedOut(request.recovery.lease)
        {
            return Some(RecoveryBeginRejection::StaleHandle);
        }
        if let PersistenceRecoveryAction::ConfirmPreserveInvalidReset(confirmation) =
            &request.action
            && (confirmation.controller != self.id
                || confirmation.incident != incident.id
                || confirmation.barrier != incident.barrier
                || confirmation.handle != incident.handle.id)
        {
            return Some(RecoveryBeginRejection::InvalidActionOrConfirmation);
        }
        None
    }
}
