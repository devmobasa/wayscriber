use std::collections::BTreeSet;

use super::*;

#[derive(Debug)]
pub(crate) struct RuntimeUiLiveOnlyGuard {
    pub(crate) controller_id: ControllerId,
    pub(crate) authority_epoch: u64,
    pub(crate) session_id: u64,
    pub(crate) guards: Vec<SeedGuard>,
}

#[derive(Debug)]
pub(crate) struct RuntimePersistentPreviewSession {
    pub(crate) permit: RuntimeUiMutationPermit,
    pub(crate) scope: RuntimeUiMutationScope,
    pub(crate) rollback: PreviewRollbackSnapshot,
}

#[derive(Debug)]
pub(crate) struct RuntimeLiveOnlyPreviewSession {
    pub(crate) guard: RuntimeUiLiveOnlyGuard,
    pub(crate) scope: RuntimeUiMutationScope,
    pub(crate) rollback: PreviewRollbackSnapshot,
}

#[derive(Debug)]
pub(crate) enum RuntimeUiPreviewSession {
    Persistent(RuntimePersistentPreviewSession),
    LiveOnly(RuntimeLiveOnlyPreviewSession),
}

#[derive(Debug)]
pub(crate) struct ConfigPositionPreviewSession {
    pub(crate) permit: ConfigInteractionPermit,
    pub(crate) rollback: PreviewRollbackSnapshot,
}

#[derive(Debug)]
pub(crate) enum RuntimePreviewFinishIntent {
    Commit(RuntimeUiMutationValues),
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConfigPositionFinishIntent {
    Commit(ToolbarPositionSeed),
    Cancel,
}

#[derive(Debug)]
pub(crate) enum PreviewFinishRequest {
    RuntimeUi {
        session: RuntimeUiPreviewSession,
        intent: RuntimePreviewFinishIntent,
    },
    ConfigPosition {
        session: ConfigPositionPreviewSession,
        intent: ConfigPositionFinishIntent,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigMutationError {
    message: String,
}

impl ConfigMutationError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PreviewFinishResult {
    AcceptedRuntime {
        through: AcceptedStateRevision,
    },
    AppliedLiveOnly,
    AppliedConfig {
        target: ConfigPositionTarget,
    },
    NoChange,
    Cancelled {
        rollback: PreviewRollbackSnapshot,
    },
    AbandonedDuringBarrier {
        barrier: ControllerBarrierId,
    },
    RejectedStaleAuthority {
        rollback: PreviewRollbackSnapshot,
    },
    FailedConfig {
        error: ConfigMutationError,
        rollback: PreviewRollbackSnapshot,
    },
}

#[derive(Debug)]
pub(crate) struct BarrierAbandonedPreview {
    pub(crate) barrier: ControllerBarrierId,
    pub(crate) session: AbandonedPreviewSession,
    pub(crate) finish: AbandonedPreviewFinish,
}

#[derive(Debug)]
pub(crate) enum AbandonedPreviewSession {
    RuntimeUi(RuntimeUiPreviewSession),
    ConfigPosition(ConfigPositionPreviewSession),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbandonedPreviewFinish {
    CommitRequested,
    CancelRequested,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BeginPreviewError {
    ControllerBusy(ControllerBarrierId),
    InvalidScope(MutationShapeError),
    MissingSeed(SeedRegistryError),
    MutationIdExhausted,
    ShuttingDown,
    InvalidAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbandonedPreviewResolutionReason {
    CancelledUnderRetainedAuthority,
    DiscardedForAuthorityChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbandonedPreviewResolution {
    pub(crate) barrier: ControllerBarrierId,
    pub(crate) finish: AbandonedPreviewFinish,
    pub(crate) rollback: PreviewRollbackSnapshot,
    pub(crate) reason: AbandonedPreviewResolutionReason,
}

impl RuntimeUiStateController {
    pub(crate) fn begin_runtime_preview(
        &self,
        scope: RuntimeUiMutationScope,
        rollback: PreviewRollbackSnapshot,
    ) -> Result<RuntimeUiPreviewSession, BeginPreviewError> {
        if self.shutting_down {
            return Err(BeginPreviewError::ShuttingDown);
        }
        if let Some(barrier) = &self.active_barrier {
            return Err(BeginPreviewError::ControllerBusy(barrier.id));
        }
        match self.file_status {
            RuntimeUiFileStatus::UnsupportedReadOnly { .. } => {
                let targets = scope
                    .canonical_targets()
                    .map_err(BeginPreviewError::InvalidScope)?;
                let guards = self
                    .seeds
                    .guards(&targets)
                    .map_err(BeginPreviewError::MissingSeed)?;
                let session_id = self.next_preview_session_id.get();
                self.next_preview_session_id.set(
                    session_id
                        .checked_add(1)
                        .ok_or(BeginPreviewError::MutationIdExhausted)?,
                );
                Ok(RuntimeUiPreviewSession::LiveOnly(
                    RuntimeLiveOnlyPreviewSession {
                        guard: RuntimeUiLiveOnlyGuard {
                            controller_id: self.id,
                            authority_epoch: self.authority_epoch,
                            session_id,
                            guards,
                        },
                        scope,
                        rollback,
                    },
                ))
            }
            RuntimeUiFileStatus::Invalid => Err(BeginPreviewError::InvalidAuthority),
            RuntimeUiFileStatus::Missing | RuntimeUiFileStatus::Supported => self
                .begin_mutation(scope.clone())
                .map(|permit| {
                    RuntimeUiPreviewSession::Persistent(RuntimePersistentPreviewSession {
                        permit,
                        scope,
                        rollback,
                    })
                })
                .map_err(|error| match error {
                    BeginMutationError::ControllerBusy(barrier) => {
                        BeginPreviewError::ControllerBusy(barrier)
                    }
                    BeginMutationError::InvalidScope(error) => {
                        BeginPreviewError::InvalidScope(error)
                    }
                    BeginMutationError::Seed(error) => BeginPreviewError::MissingSeed(error),
                    BeginMutationError::MutationIdExhausted => {
                        BeginPreviewError::MutationIdExhausted
                    }
                    BeginMutationError::ShuttingDown => BeginPreviewError::ShuttingDown,
                    BeginMutationError::UnsupportedVersion => unreachable!(),
                }),
        }
    }

    pub(crate) fn begin_config_position_preview(
        &self,
        target: ConfigPositionTarget,
        rollback: PreviewRollbackSnapshot,
    ) -> Result<ConfigPositionPreviewSession, BeginPreviewError> {
        self.begin_config_interaction(target)
            .map(|permit| ConfigPositionPreviewSession { permit, rollback })
            .map_err(|error| match error {
                BeginConfigInteractionError::ControllerBusy(barrier) => {
                    BeginPreviewError::ControllerBusy(barrier)
                }
                BeginConfigInteractionError::ShuttingDown => BeginPreviewError::ShuttingDown,
                BeginConfigInteractionError::Seed(error) => BeginPreviewError::MissingSeed(error),
                BeginConfigInteractionError::MutationIdExhausted => {
                    BeginPreviewError::MutationIdExhausted
                }
            })
    }

    pub(crate) fn finish_preview(
        &mut self,
        request: PreviewFinishRequest,
        apply_config: impl FnOnce(
            ConfigPositionTarget,
            ToolbarPositionSeed,
        ) -> Result<(), ConfigMutationError>,
    ) -> PreviewFinishResult {
        self.drain_lifecycle_controls();
        if let Some(barrier) = &self.active_barrier {
            let barrier = barrier.id;
            self.record_abandoned_preview(barrier, request);
            return PreviewFinishResult::AbandonedDuringBarrier { barrier };
        }
        match request {
            PreviewFinishRequest::RuntimeUi { session, intent } => {
                self.finish_runtime_preview(session, intent)
            }
            PreviewFinishRequest::ConfigPosition { session, intent } => match intent {
                ConfigPositionFinishIntent::Cancel => {
                    let ConfigPositionPreviewSession { permit, rollback } = session;
                    match self.validate_config_interaction(permit) {
                        ValidateConfigInteractionResult::Accepted(_) => {
                            PreviewFinishResult::Cancelled { rollback }
                        }
                        ValidateConfigInteractionResult::RejectedControllerBusy(barrier) => {
                            unreachable!(
                                "barrier {barrier:?} cannot begin inside serialized preview finish"
                            )
                        }
                        ValidateConfigInteractionResult::RejectedWrongController
                        | ValidateConfigInteractionResult::RejectedShuttingDown
                        | ValidateConfigInteractionResult::RejectedStaleAuthority
                        | ValidateConfigInteractionResult::RejectedSeedChanged => {
                            PreviewFinishResult::RejectedStaleAuthority { rollback }
                        }
                    }
                }
                ConfigPositionFinishIntent::Commit(position) => {
                    let ConfigPositionPreviewSession { permit, rollback } = session;
                    match self.validate_config_interaction(permit) {
                        ValidateConfigInteractionResult::Accepted(target) => {
                            match apply_config(target, position) {
                                Ok(()) => PreviewFinishResult::AppliedConfig { target },
                                Err(error) => PreviewFinishResult::FailedConfig { error, rollback },
                            }
                        }
                        ValidateConfigInteractionResult::RejectedControllerBusy(barrier) => {
                            unreachable!(
                                "barrier {barrier:?} cannot begin inside serialized preview finish"
                            )
                        }
                        ValidateConfigInteractionResult::RejectedWrongController
                        | ValidateConfigInteractionResult::RejectedShuttingDown
                        | ValidateConfigInteractionResult::RejectedStaleAuthority
                        | ValidateConfigInteractionResult::RejectedSeedChanged => {
                            PreviewFinishResult::RejectedStaleAuthority { rollback }
                        }
                    }
                }
            },
        }
    }

    pub(super) fn resolve_abandoned_previews(
        &mut self,
        barrier: ControllerBarrierId,
        reason: AbandonedPreviewResolutionReason,
        changed_targets: Option<&BTreeSet<InteractionSeedTarget>>,
    ) -> Vec<AbandonedPreviewResolution> {
        let mut retained = Vec::new();
        let mut resolved = Vec::new();
        for abandoned in std::mem::take(&mut self.abandoned_previews) {
            if abandoned.barrier != barrier {
                retained.push(abandoned);
                continue;
            }
            let affected_by_reload = changed_targets.is_some_and(|changed| {
                abandoned_preview_targets(&abandoned.session).any(|target| changed.contains(target))
            });
            let rollback = match abandoned.session {
                AbandonedPreviewSession::RuntimeUi(RuntimeUiPreviewSession::Persistent(
                    session,
                )) => session.rollback,
                AbandonedPreviewSession::RuntimeUi(RuntimeUiPreviewSession::LiveOnly(session)) => {
                    session.rollback
                }
                AbandonedPreviewSession::ConfigPosition(session) => session.rollback,
            };
            resolved.push(AbandonedPreviewResolution {
                barrier,
                finish: abandoned.finish,
                rollback,
                reason: if affected_by_reload {
                    AbandonedPreviewResolutionReason::DiscardedForAuthorityChange
                } else {
                    reason
                },
            });
        }
        self.abandoned_previews = retained;
        resolved
    }

    fn finish_runtime_preview(
        &mut self,
        session: RuntimeUiPreviewSession,
        intent: RuntimePreviewFinishIntent,
    ) -> PreviewFinishResult {
        match (session, intent) {
            (session, RuntimePreviewFinishIntent::Cancel) => {
                let is_current = self.runtime_preview_session_is_current(&session);
                let rollback = runtime_preview_rollback(session);
                if is_current {
                    PreviewFinishResult::Cancelled { rollback }
                } else {
                    PreviewFinishResult::RejectedStaleAuthority { rollback }
                }
            }
            (
                RuntimeUiPreviewSession::Persistent(session),
                RuntimePreviewFinishIntent::Commit(values),
            ) => {
                let RuntimePersistentPreviewSession {
                    permit,
                    scope,
                    rollback,
                } = session;
                match self.commit(permit, values) {
                    CommitResult::Accepted { through } => {
                        PreviewFinishResult::AcceptedRuntime { through }
                    }
                    CommitResult::NoChange => PreviewFinishResult::NoChange,
                    CommitResult::RejectedControllerBusy { permit, barrier } => {
                        self.abandoned_previews.push(BarrierAbandonedPreview {
                            barrier,
                            session: AbandonedPreviewSession::RuntimeUi(
                                RuntimeUiPreviewSession::Persistent(
                                    RuntimePersistentPreviewSession {
                                        permit,
                                        scope,
                                        rollback,
                                    },
                                ),
                            ),
                            finish: AbandonedPreviewFinish::CommitRequested,
                        });
                        PreviewFinishResult::AbandonedDuringBarrier { barrier }
                    }
                    CommitResult::RejectedStaleAuthorityEpoch
                    | CommitResult::RejectedSeedChanged { .. }
                    | CommitResult::RejectedWrongController
                    | CommitResult::RejectedUnsupportedVersion
                    | CommitResult::RejectedShuttingDown
                    | CommitResult::RejectedInvalidValues(_)
                    | CommitResult::RejectedPersistence(_) => {
                        PreviewFinishResult::RejectedStaleAuthority { rollback }
                    }
                }
            }
            (
                RuntimeUiPreviewSession::LiveOnly(session),
                RuntimePreviewFinishIntent::Commit(values),
            ) => {
                if session.guard.controller_id != self.id
                    || self.shutting_down
                    || session.guard.authority_epoch != self.authority_epoch
                    || values.targets()
                        != session
                            .guard
                            .guards
                            .iter()
                            .map(|guard| guard.target.clone())
                            .collect()
                    || session
                        .guard
                        .guards
                        .iter()
                        .any(|guard| !self.seeds.guard_is_current(guard))
                {
                    return PreviewFinishResult::RejectedStaleAuthority {
                        rollback: session.rollback,
                    };
                }
                if self.live_only_overlay.apply(&session.guard.guards, &values) {
                    self.live_state = RuntimeUiLiveState::rebuild(
                        &self.seeds,
                        &self.model,
                        &self.live_only_overlay,
                    );
                    PreviewFinishResult::AppliedLiveOnly
                } else {
                    PreviewFinishResult::NoChange
                }
            }
        }
    }

    fn record_abandoned_preview(
        &mut self,
        barrier: ControllerBarrierId,
        request: PreviewFinishRequest,
    ) {
        let (session, finish) = match request {
            PreviewFinishRequest::RuntimeUi { session, intent } => (
                AbandonedPreviewSession::RuntimeUi(session),
                match intent {
                    RuntimePreviewFinishIntent::Commit(_) => {
                        AbandonedPreviewFinish::CommitRequested
                    }
                    RuntimePreviewFinishIntent::Cancel => AbandonedPreviewFinish::CancelRequested,
                },
            ),
            PreviewFinishRequest::ConfigPosition { session, intent } => (
                AbandonedPreviewSession::ConfigPosition(session),
                match intent {
                    ConfigPositionFinishIntent::Commit(_) => {
                        AbandonedPreviewFinish::CommitRequested
                    }
                    ConfigPositionFinishIntent::Cancel => AbandonedPreviewFinish::CancelRequested,
                },
            ),
        };
        self.abandoned_previews.push(BarrierAbandonedPreview {
            barrier,
            session,
            finish,
        });
    }

    fn runtime_preview_session_is_current(&self, session: &RuntimeUiPreviewSession) -> bool {
        if self.shutting_down || self.active_barrier.is_some() {
            return false;
        }
        match session {
            RuntimeUiPreviewSession::Persistent(session) => {
                session.permit.controller_id == self.id
                    && session.permit.authority_epoch == self.authority_epoch
                    && matches!(
                        self.file_status,
                        RuntimeUiFileStatus::Missing | RuntimeUiFileStatus::Supported
                    )
                    && session
                        .permit
                        .guards
                        .iter()
                        .all(|guard| self.seeds.guard_is_current(guard))
            }
            RuntimeUiPreviewSession::LiveOnly(session) => {
                session.guard.controller_id == self.id
                    && session.guard.authority_epoch == self.authority_epoch
                    && matches!(
                        self.file_status,
                        RuntimeUiFileStatus::UnsupportedReadOnly { .. }
                    )
                    && session
                        .guard
                        .guards
                        .iter()
                        .all(|guard| self.seeds.guard_is_current(guard))
            }
        }
    }
}

fn abandoned_preview_targets(
    session: &AbandonedPreviewSession,
) -> impl Iterator<Item = &InteractionSeedTarget> {
    let guards = match session {
        AbandonedPreviewSession::RuntimeUi(RuntimeUiPreviewSession::Persistent(session)) => {
            session.permit.guards.as_slice()
        }
        AbandonedPreviewSession::RuntimeUi(RuntimeUiPreviewSession::LiveOnly(session)) => {
            session.guard.guards.as_slice()
        }
        AbandonedPreviewSession::ConfigPosition(session) => {
            std::slice::from_ref(&session.permit.guard)
        }
    };
    guards.iter().map(|guard| &guard.target)
}

fn runtime_preview_rollback(session: RuntimeUiPreviewSession) -> PreviewRollbackSnapshot {
    match session {
        RuntimeUiPreviewSession::Persistent(session) => session.rollback,
        RuntimeUiPreviewSession::LiveOnly(session) => session.rollback,
    }
}
