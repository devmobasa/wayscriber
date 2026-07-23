use std::path::PathBuf;

use super::*;
use crate::ui::toolbar::{RuntimeUiPersistenceMode, RuntimeUiPersistenceSnapshot, ToolbarEvent};

#[derive(Debug)]
struct ActiveRuntimeUiRecovery {
    cancellation: Option<RecoveryCancellation>,
    completion: RecoveryCompletionReceiver,
    cancelling: bool,
}

#[derive(Debug)]
struct InvalidRuntimeUiResetPrompt {
    recovery: PersistenceRecoveryHandle,
    confirmation: InvalidStateResetConfirmation,
}

#[derive(Debug)]
struct UnsupportedRuntimeUiResetPrompt {
    confirmation: UnsupportedResetConfirmation,
    version: Option<u64>,
}

#[derive(Debug, Default)]
pub(super) struct RuntimeUiLifecycleState {
    incident: Option<PersistenceIncidentId>,
    unsupported_confirmation: Option<UnsupportedRuntimeUiResetPrompt>,
    invalid_reset_prompt: Option<InvalidRuntimeUiResetPrompt>,
    active_recovery: Option<ActiveRuntimeUiRecovery>,
    detail: Option<String>,
    recovery_artifacts: Vec<PathBuf>,
}

impl RuntimeUiLifecycleState {
    pub(super) fn startup(incident: Option<PersistenceIncidentId>) -> Self {
        Self {
            incident,
            detail: incident
                .map(|_| "The runtime-state file is malformed and was left unchanged.".to_string()),
            ..Self::default()
        }
    }

    fn merge_artifacts(&mut self, artifacts: &[RuntimeStateRecoveryArtifact]) {
        for artifact in artifacts {
            if !self.recovery_artifacts.contains(&artifact.path) {
                self.recovery_artifacts.push(artifact.path.clone());
            }
        }
    }

    fn merge_evidence(&mut self, evidence: &PersistenceRecoveryEvidence) {
        self.merge_artifacts(&evidence.recovery_artifacts);
    }
}

impl ToolbarRuntimeState {
    #[cfg(test)]
    pub(super) fn has_retained_recovery_client(&self) -> bool {
        self.lifecycle
            .active_recovery
            .as_ref()
            .is_some_and(|active| active.cancellation.is_some())
    }

    pub(in crate::backend::wayland) fn persistence_snapshot(&self) -> RuntimeUiPersistenceSnapshot {
        let mode = if let Some(prompt) = &self.lifecycle.unsupported_confirmation {
            RuntimeUiPersistenceMode::AwaitingUnsupportedResetConfirmation {
                version: prompt.version,
            }
        } else if self.lifecycle.invalid_reset_prompt.is_some() {
            RuntimeUiPersistenceMode::AwaitingInvalidResetConfirmation
        } else if let Some(active) = &self.lifecycle.active_recovery {
            if active.cancelling {
                RuntimeUiPersistenceMode::CancellingRecovery
            } else {
                RuntimeUiPersistenceMode::Recovering
            }
        } else if let Some(barrier) = self.controller.active_barrier() {
            match barrier.phase {
                ControllerBarrierPhase::PersistenceUnhealthy { .. } => {
                    RuntimeUiPersistenceMode::Unhealthy
                }
                _ => RuntimeUiPersistenceMode::Resetting,
            }
        } else {
            match self.controller.file_status() {
                RuntimeUiFileStatus::Missing => RuntimeUiPersistenceMode::Missing,
                RuntimeUiFileStatus::Supported => RuntimeUiPersistenceMode::Supported,
                RuntimeUiFileStatus::UnsupportedReadOnly { version } => {
                    RuntimeUiPersistenceMode::UnsupportedReadOnly { version: *version }
                }
                RuntimeUiFileStatus::Invalid => RuntimeUiPersistenceMode::Unhealthy,
            }
        };
        RuntimeUiPersistenceSnapshot {
            path: self.runtime_path.clone(),
            mode,
            detail: self.lifecycle.detail.clone(),
            recovery_artifacts: self.lifecycle.recovery_artifacts.clone(),
        }
    }

    pub(in crate::backend::wayland) fn handle_persistence_lifecycle_event(
        &mut self,
        event: &ToolbarEvent,
    ) -> bool {
        let live_before = self.controller.live_state().clone();
        let handled = match event {
            ToolbarEvent::RequestRuntimeUiReset => {
                self.request_runtime_ui_reset();
                true
            }
            ToolbarEvent::ConfirmUnsupportedRuntimeUiReset => {
                self.confirm_unsupported_runtime_ui_reset();
                true
            }
            ToolbarEvent::CancelUnsupportedRuntimeUiReset => {
                self.cancel_unsupported_runtime_ui_reset();
                true
            }
            ToolbarEvent::RetryRuntimeUiPersistence => {
                self.begin_runtime_ui_recovery(PersistenceRecoveryAction::RetryPending);
                true
            }
            ToolbarEvent::DiscardPendingRuntimeUiAndAdoptDisk => {
                self.begin_runtime_ui_recovery(
                    PersistenceRecoveryAction::DiscardPendingAndAdoptObserved,
                );
                true
            }
            ToolbarEvent::RequestPreserveInvalidRuntimeUiReset => {
                self.begin_runtime_ui_recovery(
                    PersistenceRecoveryAction::RequestPreserveInvalidReset,
                );
                true
            }
            ToolbarEvent::ConfirmPreserveInvalidRuntimeUiReset => {
                self.confirm_preserve_invalid_runtime_ui_reset();
                true
            }
            ToolbarEvent::CancelPreserveInvalidRuntimeUiReset => {
                self.lifecycle.invalid_reset_prompt = None;
                self.lifecycle.detail =
                    Some("Invalid runtime-state data was left unchanged.".to_string());
                true
            }
            ToolbarEvent::CancelRuntimeUiRecovery => {
                self.cancel_runtime_ui_recovery();
                true
            }
            _ => false,
        };
        if handled {
            self.poll_recovery_completion();
            self.dispatch_writer_command();
            self.live_rebuild_pending |= self.controller.live_state() != &live_before;
        }
        handled
    }

    fn request_runtime_ui_reset(&mut self) {
        match self.controller.request_runtime_ui_reset() {
            RequestResetResult::Started { .. } => {
                self.lifecycle.detail = Some(
                    "Runtime preferences will return to the latest configured defaults."
                        .to_string(),
                );
            }
            RequestResetResult::RequiresUnsupportedConfirmation {
                observed_version,
                confirmation,
            } => {
                self.lifecycle.unsupported_confirmation = Some(UnsupportedRuntimeUiResetPrompt {
                    confirmation,
                    version: observed_version,
                });
                self.lifecycle.detail = Some(match observed_version {
                    Some(version) => format!(
                        "Version {version} is newer than this build. Confirming preserves it before reset."
                    ),
                    None => "The runtime-state version is unsupported. Confirming preserves it before reset."
                        .to_string(),
                });
            }
            result => {
                self.lifecycle.detail = Some(format!("Runtime reset could not start: {result:?}"));
            }
        }
    }

    fn confirm_unsupported_runtime_ui_reset(&mut self) {
        let Some(prompt) = self.lifecycle.unsupported_confirmation.take() else {
            self.lifecycle.detail = Some("The runtime reset confirmation expired.".to_string());
            return;
        };
        match self
            .controller
            .confirm_unsupported_reset(prompt.confirmation)
        {
            ConfirmUnsupportedResetResult::Started { .. } => {
                self.lifecycle.detail = Some(
                    "Preserving the unsupported file and resetting runtime preferences…"
                        .to_string(),
                );
            }
            result => {
                self.lifecycle.detail = Some(format!(
                    "The unsupported runtime reset could not start: {result:?}"
                ));
            }
        }
    }

    fn cancel_unsupported_runtime_ui_reset(&mut self) {
        let Some(prompt) = self.lifecycle.unsupported_confirmation.take() else {
            self.lifecycle.detail = Some("The runtime reset confirmation expired.".to_string());
            return;
        };
        self.lifecycle.detail = Some(
            match self
                .controller
                .cancel_unsupported_reset_confirmation(prompt.confirmation)
            {
                CancelUnsupportedResetConfirmationResult::Cancelled => {
                    "The unsupported runtime-state file was left unchanged.".to_string()
                }
                CancelUnsupportedResetConfirmationResult::RejectedToken => {
                    "The runtime reset confirmation had already expired.".to_string()
                }
            },
        );
    }

    fn begin_runtime_ui_recovery(&mut self, action: PersistenceRecoveryAction) {
        if self.lifecycle.active_recovery.is_some() {
            self.lifecycle.detail = Some("A recovery attempt is already running.".to_string());
            return;
        }
        let Some(incident) = self.lifecycle.incident else {
            self.lifecycle.detail =
                Some("No persistence incident is available to recover.".to_string());
            return;
        };
        let recovery = match self
            .controller
            .checkout_persistence_recovery_handle(incident)
        {
            CheckoutPersistenceRecoveryHandleResult::CheckedOut(recovery) => recovery,
            result => {
                self.lifecycle.detail = Some(format!("Recovery is not available: {result:?}"));
                return;
            }
        };
        self.begin_runtime_ui_recovery_request(PersistenceRecoveryRequest { recovery, action });
    }

    fn begin_runtime_ui_recovery_request(&mut self, request: PersistenceRecoveryRequest) {
        match self.controller.begin_persistence_recovery(request) {
            BeginPersistenceRecoveryResult::Started { client, .. } => {
                self.lifecycle.active_recovery = Some(ActiveRuntimeUiRecovery {
                    cancellation: Some(client.cancellation),
                    completion: client.completion,
                    cancelling: false,
                });
                self.lifecycle.detail = Some("Inspecting the runtime-state source…".to_string());
            }
            BeginPersistenceRecoveryResult::Rejected { request, reason } => {
                drop(request);
                self.lifecycle.detail = Some(format!("Recovery could not start: {reason:?}"));
            }
        }
    }

    fn confirm_preserve_invalid_runtime_ui_reset(&mut self) {
        let Some(prompt) = self.lifecycle.invalid_reset_prompt.take() else {
            self.lifecycle.detail = Some("The invalid-file confirmation expired.".to_string());
            return;
        };
        self.begin_runtime_ui_recovery_request(PersistenceRecoveryRequest {
            recovery: prompt.recovery,
            action: PersistenceRecoveryAction::ConfirmPreserveInvalidReset(prompt.confirmation),
        });
    }

    fn cancel_runtime_ui_recovery(&mut self) {
        let Some(active) = self.lifecycle.active_recovery.as_mut() else {
            self.lifecycle.detail = Some("No recovery attempt is running.".to_string());
            return;
        };
        let Some(cancellation) = active.cancellation.take() else {
            self.lifecycle.detail = Some("Recovery cancellation is already pending.".to_string());
            return;
        };
        match self.controller.cancel_persistence_recovery(cancellation) {
            CancelPersistenceRecoveryResult::Cancelled => {
                active.cancelling = true;
                self.lifecycle.detail = Some("Recovery was cancelled safely.".to_string());
            }
            CancelPersistenceRecoveryResult::PendingIrrevocableIo { .. } => {
                active.cancelling = true;
                self.lifecycle.detail = Some(
                    "A write had already started; waiting for its real completion before reinspection."
                        .to_string(),
                );
            }
            CancelPersistenceRecoveryResult::RerouteWrongController { cancellation } => {
                active.cancellation = Some(cancellation);
                self.lifecycle.detail =
                    Some("Recovery cancellation reached the wrong controller.".to_string());
            }
            CancelPersistenceRecoveryResult::RejectedInert { reason } => {
                self.lifecycle.detail =
                    Some(format!("Recovery cancellation was inert: {reason:?}"));
            }
        }
    }

    pub(super) fn poll_recovery_completion(&mut self) -> bool {
        let result = self
            .lifecycle
            .active_recovery
            .as_ref()
            .and_then(|active| active.completion.try_recv());
        let Some(result) = result else {
            return false;
        };
        // The controller has already terminalized this attempt. Dropping the
        // retained cancellation capability now is intentionally inert.
        self.lifecycle.active_recovery = None;
        match result {
            PersistenceRecoveryResult::Recovered {
                incident, evidence, ..
            } => {
                self.lifecycle.merge_evidence(&evidence);
                self.lifecycle.incident = None;
                self.lifecycle.detail = Some(format!(
                    "Persistence incident {} recovered; pending preferences are durable.",
                    incident.get()
                ));
            }
            PersistenceRecoveryResult::ExternalAuthorityInstalled {
                evidence,
                path_effect,
                ..
            } => {
                self.lifecycle.merge_evidence(&evidence);
                self.lifecycle.incident = None;
                self.lifecycle.detail = Some(format!(
                    "The externally changed runtime state won and was adopted ({path_effect:?})."
                ));
            }
            PersistenceRecoveryResult::InvalidSourcePreservedAndReset {
                evidence,
                path_effect,
                ..
            } => {
                self.lifecycle.merge_evidence(&evidence);
                self.lifecycle.incident = None;
                self.lifecycle.detail = Some(format!(
                    "Invalid runtime state was preserved and reset ({path_effect:?})."
                ));
            }
            PersistenceRecoveryResult::RequiresInvalidResetConfirmation {
                recovery,
                observed,
                confirmation,
                evidence,
            } => {
                self.lifecycle.merge_evidence(&evidence);
                self.lifecycle.incident = Some(recovery.incident());
                self.lifecycle.invalid_reset_prompt = Some(InvalidRuntimeUiResetPrompt {
                    recovery,
                    confirmation,
                });
                self.lifecycle.detail = Some(format!(
                    "Confirm preserving the invalid source at {} before reset.",
                    observed.revision.path_identity().source_path().display()
                ));
            }
            PersistenceRecoveryResult::ObservationChanged {
                recovery, evidence, ..
            } => {
                self.lifecycle.merge_evidence(&evidence);
                self.lifecycle.incident = Some(recovery.incident());
                self.lifecycle.detail = Some(
                    "The source changed after confirmation; it was not overwritten. Reinspect before retrying."
                        .to_string(),
                );
                drop(recovery);
            }
            PersistenceRecoveryResult::StillUnhealthy {
                recovery,
                error,
                evidence,
                ..
            } => {
                self.lifecycle.merge_evidence(&evidence);
                self.lifecycle.incident = Some(recovery.incident());
                self.lifecycle.detail = Some(error.message().to_string());
                drop(recovery);
            }
            PersistenceRecoveryResult::Cancelled {
                recovery, evidence, ..
            } => {
                self.lifecycle.merge_evidence(&evidence);
                self.lifecycle.incident = Some(recovery.incident());
                self.lifecycle.detail = Some(
                    "Recovery was cancelled; persistence remains blocked until a new action succeeds."
                        .to_string(),
                );
                drop(recovery);
            }
            PersistenceRecoveryResult::Shutdown { evidence, .. } => {
                self.lifecycle.merge_evidence(&evidence);
                self.lifecycle.incident = None;
                self.lifecycle.detail = Some("Runtime-state persistence shut down.".to_string());
            }
        }
        true
    }

    pub(super) fn note_persistence_unhealthy(
        &mut self,
        incident: PersistenceIncidentId,
        error: &RuntimeStateIoError,
        artifacts: &[RuntimeStateRecoveryArtifact],
    ) {
        self.lifecycle.incident = Some(incident);
        self.lifecycle.unsupported_confirmation = None;
        self.lifecycle.invalid_reset_prompt = None;
        self.lifecycle.merge_artifacts(artifacts);
        self.lifecycle.detail = Some(error.message().to_string());
    }

    pub(super) fn note_reset_completed(&mut self, artifacts: &[RuntimeStateRecoveryArtifact]) {
        self.lifecycle.incident = None;
        self.lifecycle.merge_artifacts(artifacts);
        self.lifecycle.detail =
            Some("Runtime preferences were reset to configured defaults.".to_string());
    }

    pub(super) fn note_recovery_artifacts(&mut self, artifacts: &[RuntimeStateRecoveryArtifact]) {
        self.lifecycle.merge_artifacts(artifacts);
    }

    pub(super) fn note_lifecycle_error(&mut self, message: impl Into<String>) {
        self.lifecycle.detail = Some(message.into());
    }

    pub(super) fn note_external_authority_installed(&mut self) {
        self.lifecycle.incident = None;
        self.lifecycle.detail =
            Some("The externally changed runtime state was adopted safely.".to_string());
    }

    pub(super) fn note_invalid_external_authority(&mut self, incident: PersistenceIncidentId) {
        self.lifecycle.incident = Some(incident);
    }
}
