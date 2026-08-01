use super::*;

impl RuntimeUiStateController {
    pub(crate) fn request_supported_reset(&mut self) -> RequestResetResult {
        self.request_runtime_ui_reset()
    }

    pub(crate) fn request_runtime_ui_reset(&mut self) -> RequestResetResult {
        self.drain_lifecycle_controls();
        if self.shutting_down {
            return RequestResetResult::RejectedShuttingDown;
        }
        if let Some(barrier) = &self.active_barrier {
            return RequestResetResult::RejectedControllerBusy(barrier.id);
        }
        if let RuntimeUiFileStatus::UnsupportedReadOnly { version } = self.file_status {
            let id = UnsupportedResetConfirmationId(
                match self.next_unsupported_reset_confirmation_id.checked_add(1) {
                    Some(next) => {
                        let current = self.next_unsupported_reset_confirmation_id;
                        self.next_unsupported_reset_confirmation_id = next;
                        current
                    }
                    None => return RequestResetResult::ConfirmationIdExhausted,
                },
            );
            let confirmation = UnsupportedResetConfirmation {
                controller: self.id,
                id,
                observed_version: version,
                revision: self.pipeline.stable_source().clone(),
            };
            self.pending_unsupported_reset_confirmation = Some(confirmation.clone());
            return RequestResetResult::RequiresUnsupportedConfirmation {
                observed_version: version,
                confirmation,
            };
        }
        if matches!(self.file_status, RuntimeUiFileStatus::Invalid) {
            return RequestResetResult::RejectedUnsupportedVersion;
        }
        self.pending_unsupported_reset_confirmation = None;
        let Some(publish_epoch) = self.authority_epoch.checked_add(1) else {
            return RequestResetResult::EpochExhausted;
        };
        let Some(barrier_id) = self.allocate_barrier_id() else {
            return RequestResetResult::BarrierIdExhausted;
        };
        let through = match self.pipeline.allocate_reset_revision() {
            Ok(through) => through,
            Err(error) => return RequestResetResult::Rejected(error),
        };
        let held_by_reset = self.pipeline.hold_trailing_replacements();
        let waiting = self.pipeline.has_source_mutation_in_flight();
        self.active_barrier = Some(ActiveControllerBarrier {
            id: barrier_id,
            operation: ControllerBarrierOperation::ResetSupported,
            phase: if let Some(request) = self.pipeline.source_mutation_in_flight() {
                ControllerBarrierPhase::WaitingForPrerequisite(request.id)
            } else {
                ControllerBarrierPhase::Inspecting
            },
        });
        self.supported_reset = Some(SupportedResetTransaction {
            barrier: barrier_id,
            original_epoch: self.authority_epoch,
            publish_epoch,
            through,
            held_by_reset,
            authority: if waiting {
                SupportedResetAuthorityState::WaitingForPrerequisite
            } else {
                SupportedResetAuthorityState::Captured(Box::new(self.capture_reset_authority()))
            },
        });
        if let Err(error) = self.pipeline.stage_supported_reset(through, publish_epoch) {
            self.active_barrier = None;
            if let Some(transaction) = self.supported_reset.take() {
                let reset_error = RuntimeStateIoError::new("reset dispatch failed");
                if !self.pipeline.cancel_pending_reset(
                    transaction.through,
                    DurabilityOutcome::Failed(reset_error.clone()),
                ) {
                    self.pipeline
                        .settle_failed([transaction.through], reset_error);
                }
                let held_by_reset = transaction.held_by_reset;
                if let Err(restore_error) = self
                    .pipeline
                    .restore_held_replacements(held_by_reset.clone())
                {
                    self.pipeline.settle_held_failed(
                        &held_by_reset,
                        RuntimeStateIoError::new(format!(
                            "failed to restore reset-held state: {restore_error:?}"
                        )),
                    );
                }
            }
            return RequestResetResult::Rejected(error);
        }
        self.refresh_reset_barrier_phase();
        RequestResetResult::Started {
            barrier: barrier_id,
            through,
            publish_epoch,
        }
    }

    pub(crate) fn cancel_unsupported_reset_confirmation(
        &mut self,
        confirmation: UnsupportedResetConfirmation,
    ) -> CancelUnsupportedResetConfirmationResult {
        self.drain_lifecycle_controls();
        if !self.unsupported_reset_confirmation_is_current(&confirmation) {
            return CancelUnsupportedResetConfirmationResult::RejectedToken;
        }
        self.pending_unsupported_reset_confirmation = None;
        CancelUnsupportedResetConfirmationResult::Cancelled
    }

    pub(crate) fn confirm_unsupported_reset(
        &mut self,
        confirmation: UnsupportedResetConfirmation,
    ) -> ConfirmUnsupportedResetResult {
        self.drain_lifecycle_controls();
        if !self.unsupported_reset_confirmation_is_current(&confirmation) {
            return ConfirmUnsupportedResetResult::RejectedToken;
        }
        self.pending_unsupported_reset_confirmation = None;
        if self.shutting_down {
            return ConfirmUnsupportedResetResult::RejectedShuttingDown;
        }
        if let Some(barrier) = &self.active_barrier {
            return ConfirmUnsupportedResetResult::RejectedControllerBusy(barrier.id);
        }
        let Some(publish_epoch) = self.authority_epoch.checked_add(1) else {
            return ConfirmUnsupportedResetResult::EpochExhausted;
        };
        let Some(barrier_id) = self.allocate_barrier_id() else {
            return ConfirmUnsupportedResetResult::BarrierIdExhausted;
        };
        let through = match self.pipeline.allocate_reset_revision() {
            Ok(through) => through,
            Err(error) => return ConfirmUnsupportedResetResult::Rejected(error),
        };
        let held_by_reset = self.pipeline.hold_trailing_replacements();
        let waiting = self.pipeline.has_source_mutation_in_flight();
        self.active_barrier = Some(ActiveControllerBarrier {
            id: barrier_id,
            operation: ControllerBarrierOperation::ConfirmUnsupportedReset,
            phase: if let Some(request) = self.pipeline.source_mutation_in_flight() {
                ControllerBarrierPhase::WaitingForPrerequisite(request.id)
            } else {
                ControllerBarrierPhase::Inspecting
            },
        });
        self.supported_reset = Some(SupportedResetTransaction {
            barrier: barrier_id,
            original_epoch: self.authority_epoch,
            publish_epoch,
            through,
            held_by_reset,
            authority: if waiting {
                SupportedResetAuthorityState::WaitingForPrerequisite
            } else {
                SupportedResetAuthorityState::Captured(Box::new(self.capture_reset_authority()))
            },
        });
        if let Err(error) =
            self.pipeline
                .stage_unsupported_reset(through, publish_epoch, confirmation.revision)
        {
            self.active_barrier = None;
            if let Some(transaction) = self.supported_reset.take() {
                let reset_error = RuntimeStateIoError::new("unsupported reset dispatch failed");
                if !self.pipeline.cancel_pending_reset(
                    transaction.through,
                    DurabilityOutcome::Failed(reset_error.clone()),
                ) {
                    self.pipeline
                        .settle_failed([transaction.through], reset_error);
                }
                let held_by_reset = transaction.held_by_reset;
                if let Err(restore_error) = self
                    .pipeline
                    .restore_held_replacements(held_by_reset.clone())
                {
                    self.pipeline.settle_held_failed(
                        &held_by_reset,
                        RuntimeStateIoError::new(format!(
                            "failed to restore unsupported-reset-held state: {restore_error:?}"
                        )),
                    );
                }
            }
            return ConfirmUnsupportedResetResult::Rejected(error);
        }
        self.refresh_reset_barrier_phase();
        ConfirmUnsupportedResetResult::Started {
            barrier: barrier_id,
            through,
            publish_epoch,
        }
    }
}
