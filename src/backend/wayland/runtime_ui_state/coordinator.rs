use std::path::Path;
use std::sync::mpsc::TryRecvError;

use anyhow::{Context, Result};

use super::*;

impl ToolbarRuntimeState {
    pub(in crate::backend::wayland) fn start(
        config: &Config,
        path: &Path,
        runtime_wake: crate::backend::wayland::RuntimeWakeHandle,
    ) -> Result<Self> {
        let parent = path
            .parent()
            .context("runtime UI state path has no parent directory")?;
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create runtime UI state directory {}",
                parent.display()
            )
        })?;

        let seeds = toolbar_seeds_from_config(config)?;
        let store = RuntimeUiStateStore::new(path);
        let inspection = store.inspect().map_err(|error| {
            anyhow::anyhow!("failed to inspect runtime UI state: {}", error.message())
        })?;
        let bootstrap = inspection.into_controller_bootstrap(seeds);
        if let Some(incident) = bootstrap.startup_incident {
            log::warn!(
                "Runtime UI state started behind recovery barrier {:?}; toolbar preference writes are blocked",
                incident
            );
        }
        let writer = RuntimeUiStateWriter::spawn_with_completion_notifier(store, move || {
            if let Err(error) = runtime_wake.wake() {
                log::error!("Failed to wake runtime for runtime UI state completion: {error}");
            }
        })
        .context("failed to start runtime UI state writer")?;
        let mut runtime = Self {
            controller: bootstrap.controller,
            writer: Some(writer),
            pending_writer_command: None,
            live_rebuild_pending: false,
            item_drag: None,
            position_drag: None,
        };
        runtime.dispatch_writer_command();
        Ok(runtime)
    }

    pub(in crate::backend::wayland) fn apply_startup_state(&self, input: &mut InputState) {
        apply_live_toolbar_state(input, self.controller.live_state(), |_| true);
        input.toolbar_top_visible = input.toolbar_top_pinned;
        input.toolbar_side_visible = input.toolbar_side_pinned;
        input.toolbar_visible = input.toolbar_top_visible || input.toolbar_side_visible;
    }

    pub(super) fn apply_live_state(
        &self,
        input: &mut InputState,
        positions: &mut ToolbarPositionSnapshot,
    ) {
        apply_live_toolbar_state(input, self.controller.live_state(), |_| true);
        apply_live_toolbar_positions(positions, self.controller.live_state(), |_| true);
    }

    pub(in crate::backend::wayland) fn begin_toolbar_mutation(
        &self,
        target: ToolbarRuntimeUiPersistenceTarget,
        input: &InputState,
    ) -> Option<PreparedToolbarMutation> {
        let values = toolbar_values(target, input).ok()?;
        let rollback = PreviewRollbackSnapshot {
            values: values.values().clone(),
        };
        let scope = RuntimeUiMutationScope::batch(values.targets());
        match self.controller.begin_runtime_preview(scope, rollback) {
            Ok(session) => Some(PreparedToolbarMutation { target, session }),
            Err(error) => {
                log::warn!("Toolbar runtime mutation blocked: {error:?}");
                None
            }
        }
    }

    pub(in crate::backend::wayland) fn finish_toolbar_mutation(
        &mut self,
        prepared: PreparedToolbarMutation,
        applied: bool,
        input: &InputState,
    ) -> ToolbarRuntimeFinish {
        let intent = if applied {
            match toolbar_values(prepared.target, input) {
                Ok(values) => RuntimePreviewFinishIntent::Commit(values),
                Err(error) => {
                    log::error!("Toolbar runtime mutation produced invalid values: {error:?}");
                    RuntimePreviewFinishIntent::Cancel
                }
            }
        } else {
            RuntimePreviewFinishIntent::Cancel
        };
        let result = self.controller.finish_preview(
            PreviewFinishRequest::RuntimeUi {
                session: prepared.session,
                intent,
            },
            |_, _| unreachable!("runtime toolbar mutation cannot write config"),
        );
        self.finish_result(result)
    }

    pub(in crate::backend::wayland) fn begin_item_drag(
        &mut self,
        group: ToolbarItemOrderGroup,
        input: &InputState,
    ) -> bool {
        if self.item_drag.is_some() {
            return false;
        }
        let target = ToolbarRuntimeUiPersistenceTarget::ItemOrder(group);
        let values = match toolbar_values(target, input) {
            Ok(values) => values,
            Err(error) => {
                log::error!("Toolbar item drag has invalid rollback values: {error:?}");
                return false;
            }
        };
        let rollback = PreviewRollbackSnapshot {
            values: values.values().clone(),
        };
        let scope = RuntimeUiMutationScope::one(InteractionSeedTarget::ItemOrder(group));
        match self.controller.begin_runtime_preview(scope, rollback) {
            Ok(session) => {
                self.item_drag = Some(ActiveItemDrag { group, session });
                true
            }
            Err(error) => {
                log::warn!("Toolbar item drag blocked: {error:?}");
                false
            }
        }
    }

    pub(in crate::backend::wayland) fn item_drag_update_allowed(&self) -> bool {
        self.controller.active_barrier().is_none() && self.item_drag.is_some()
    }

    pub(in crate::backend::wayland) fn finish_item_drag(
        &mut self,
        commit: bool,
        input: &InputState,
    ) -> ToolbarRuntimeFinish {
        let Some(active) = self.item_drag.take() else {
            return ToolbarRuntimeFinish::KeepPreview;
        };
        let intent = if commit {
            let target = ToolbarRuntimeUiPersistenceTarget::ItemOrder(active.group);
            match toolbar_values(target, input) {
                Ok(values) => RuntimePreviewFinishIntent::Commit(values),
                Err(error) => {
                    log::error!("Toolbar item drag produced invalid values: {error:?}");
                    RuntimePreviewFinishIntent::Cancel
                }
            }
        } else {
            RuntimePreviewFinishIntent::Cancel
        };
        let result = self.controller.finish_preview(
            PreviewFinishRequest::RuntimeUi {
                session: active.session,
                intent,
            },
            |_, _| unreachable!("item-order preview cannot write config"),
        );
        self.finish_result(result)
    }

    pub(in crate::backend::wayland) fn begin_position_drag(
        &mut self,
        target: ConfigPositionTarget,
        positions: ToolbarPositionSnapshot,
    ) -> bool {
        if let Some(active) = &self.position_drag {
            return active.target == target;
        }
        let rollback = position_rollback(target, positions);
        match self
            .controller
            .begin_config_position_preview(target, rollback)
        {
            Ok(session) => {
                self.position_drag = Some(ActivePositionDrag { target, session });
                true
            }
            Err(error) => {
                log::warn!("Toolbar position drag blocked: {error:?}");
                false
            }
        }
    }

    pub(in crate::backend::wayland) fn position_drag_update_allowed(
        &self,
        target: ConfigPositionTarget,
    ) -> bool {
        self.controller.active_barrier().is_none()
            && self
                .position_drag
                .as_ref()
                .is_some_and(|active| active.target == target)
    }

    pub(in crate::backend::wayland) fn finish_position_drag(
        &mut self,
        commit: bool,
        positions: ToolbarPositionSnapshot,
        apply_config: impl FnOnce(
            ConfigPositionTarget,
            ToolbarPositionSeed,
        ) -> std::result::Result<(), ConfigMutationError>,
    ) -> (ToolbarRuntimeFinish, bool) {
        let Some(active) = self.position_drag.take() else {
            return (ToolbarRuntimeFinish::KeepPreview, false);
        };
        let position = match active.target {
            ConfigPositionTarget::Top => positions.top,
            ConfigPositionTarget::Side => positions.side,
        };
        let guarded_positions_are_finite = active.target.seed_targets().into_iter().all(|target| {
            let raw = match target {
                InteractionSeedTarget::TopPosition => positions.top,
                InteractionSeedTarget::SidePosition => positions.side,
                _ => unreachable!("config position target returned a runtime-owned seed"),
            };
            ToolbarPositionSeed::new(raw.0, raw.1).is_some()
        });
        let Some(position) = ToolbarPositionSeed::new(position.0, position.1)
            .filter(|_| guarded_positions_are_finite)
        else {
            let result = self.controller.finish_preview(
                PreviewFinishRequest::ConfigPosition {
                    session: active.session,
                    intent: ConfigPositionFinishIntent::Cancel,
                },
                apply_config,
            );
            return (self.finish_result(result), false);
        };
        let intent = if commit {
            ConfigPositionFinishIntent::Commit(position)
        } else {
            ConfigPositionFinishIntent::Cancel
        };
        let result = self.controller.finish_preview(
            PreviewFinishRequest::ConfigPosition {
                session: active.session,
                intent,
            },
            apply_config,
        );
        let applied_config = matches!(result, PreviewFinishResult::AppliedConfig { .. });
        (self.finish_result(result), applied_config)
    }

    pub(in crate::backend::wayland) fn refresh_config_seeds(
        &mut self,
        config: &Config,
        input: &mut InputState,
        positions: &mut ToolbarPositionSnapshot,
    ) -> ToolbarSeedRefresh {
        let seeds = match toolbar_seeds_from_config(config) {
            Ok(seeds) => seeds,
            Err(error) => {
                log::warn!("Runtime UI seed refresh rejected: {error:#}");
                return ToolbarSeedRefresh::default();
            }
        };
        let changed = match self.controller.update_seeds(seeds) {
            UpdateSeedsResult::Applied {
                changed_targets, ..
            } => changed_targets,
            UpdateSeedsResult::StagedBehindBarrier { .. } => {
                self.dispatch_writer_command();
                return ToolbarSeedRefresh::default();
            }
            other => {
                log::warn!("Runtime UI seed refresh failed: {other:?}");
                return ToolbarSeedRefresh::default();
            }
        };
        let item_drag_aborted = self.item_drag.as_ref().is_some_and(|active| {
            changed.contains(&InteractionSeedTarget::ItemOrder(active.group))
        });
        if item_drag_aborted {
            self.item_drag = None;
            input.clear_toolbar_item_drag();
        }
        let position_drag_aborted = self.position_drag.as_ref().is_some_and(|active| {
            active
                .session
                .permit
                .guards
                .iter()
                .any(|guard| changed.contains(&guard.target))
        });
        let position_rollback = position_drag_aborted.then(|| {
            let mut rollback = self
                .position_drag
                .take()
                .expect("aborted position drag was just observed")
                .session
                .rollback;
            // A side drag is guarded by both position seeds because its final
            // save can reconcile the top X offset. If only one guard changes,
            // the changed target must come from the new live authority while
            // every other previewed target returns to its pre-drag value.
            rollback
                .values
                .retain(|target, _| !changed.contains(target));
            rollback
        });
        apply_live_toolbar_state(input, self.controller.live_state(), |target| {
            changed.contains(target)
        });
        apply_live_toolbar_positions(positions, self.controller.live_state(), |target| {
            changed.contains(target)
        });
        if let Some(rollback) = position_rollback {
            apply_toolbar_runtime_rollback(input, positions, &rollback);
        }
        self.dispatch_writer_command();
        ToolbarSeedRefresh {
            item_drag_aborted,
            position_drag_aborted,
            applied: true,
        }
    }

    pub(super) fn drain_writer_completions(&mut self) -> ToolbarRuntimeDrain {
        let mut drain = ToolbarRuntimeDrain {
            rebuild_live: std::mem::take(&mut self.live_rebuild_pending),
            ..ToolbarRuntimeDrain::default()
        };
        self.collect_preview_resolutions(&mut drain);
        loop {
            let completion = match self.writer.as_ref().map(RuntimeUiStateWriter::try_recv) {
                Some(Ok(completion)) => completion,
                Some(Err(TryRecvError::Empty)) | None => break,
                Some(Err(TryRecvError::Disconnected)) => {
                    log::error!("Runtime UI state writer disconnected");
                    break;
                }
            };
            self.integrate_writer_completion(completion);
            drain.rebuild_live |= std::mem::take(&mut self.live_rebuild_pending);
            self.collect_preview_resolutions(&mut drain);
            self.dispatch_writer_command();
        }
        if self.drop_stale_active_previews() {
            drain.rebuild_live = true;
        }
        drain
    }

    fn collect_preview_resolutions(&mut self, drain: &mut ToolbarRuntimeDrain) {
        for resolution in self.controller.take_preview_resolutions() {
            match resolution.reason {
                AbandonedPreviewResolutionReason::CancelledUnderRetainedAuthority => {
                    drain.rollbacks.push(resolution.rollback);
                }
                AbandonedPreviewResolutionReason::DiscardedForAuthorityChange => {
                    drain.rebuild_live = true;
                }
            }
        }
    }

    pub(in crate::backend::wayland) fn shutdown_blocking(&mut self) {
        if self.writer.is_none() {
            return;
        }
        if let Err(error) = self.controller.request_shutdown() {
            log::warn!("Runtime UI controller shutdown request failed: {error:?}");
        }
        self.dispatch_writer_command();
        while !self.controller.shutdown_complete() {
            let completion = match self.writer.as_ref().expect("writer checked").recv() {
                Ok(completion) => completion,
                Err(error) => {
                    log::error!("Runtime UI writer stopped during shutdown: {error}");
                    break;
                }
            };
            self.integrate_writer_completion(completion);
            self.dispatch_writer_command();
        }
        if let Some(writer) = self.writer.take() {
            writer.shutdown();
        }
    }

    fn finish_result(&mut self, result: PreviewFinishResult) -> ToolbarRuntimeFinish {
        let effect = match result {
            PreviewFinishResult::AcceptedRuntime { .. }
            | PreviewFinishResult::AppliedLiveOnly
            | PreviewFinishResult::AppliedConfig { .. }
            | PreviewFinishResult::NoChange => ToolbarRuntimeFinish::KeepPreview,
            PreviewFinishResult::Cancelled { rollback }
            | PreviewFinishResult::RejectedStaleAuthority { rollback }
            | PreviewFinishResult::FailedConfig { rollback, .. } => {
                ToolbarRuntimeFinish::Rollback(rollback)
            }
            PreviewFinishResult::AbandonedDuringBarrier { .. } => {
                ToolbarRuntimeFinish::DeferredBehindBarrier
            }
        };
        self.dispatch_writer_command();
        effect
    }

    pub(super) fn dispatch_writer_command(&mut self) {
        if self.pending_writer_command.is_none() {
            self.pending_writer_command = self
                .controller
                .take_source_mutation()
                .map(RuntimeStateWriterCommand::SourceMutation)
                .or_else(|| {
                    self.controller
                        .take_recovery_io_command()
                        .map(RuntimeStateWriterCommand::Recovery)
                });
        }
        let Some(command) = self.pending_writer_command.take() else {
            return;
        };
        let Some(writer) = self.writer.as_ref() else {
            self.integrate_rejected_writer_command(command);
            return;
        };
        match writer.submit(command) {
            Ok(()) => {}
            Err(RuntimeStateWriterSubmitError::Full(command)) => {
                self.pending_writer_command = Some(*command);
            }
            Err(RuntimeStateWriterSubmitError::Disconnected(command)) => {
                self.integrate_rejected_writer_command(*command);
            }
        }
    }

    fn integrate_rejected_writer_command(&mut self, command: RuntimeStateWriterCommand) {
        let live_before = self.controller.live_state().clone();
        let error = RuntimeStateIoError::new("runtime UI writer rejected an undispatched command");
        match command {
            RuntimeStateWriterCommand::SourceMutation(request) => {
                let result = SourceMutationResult::Failed {
                    id: request.id,
                    error,
                    active: None,
                    recovery_artifacts: Vec::new(),
                    path_effect: RuntimeStateFailurePathEffect::Known(
                        RuntimeStateObservedPathEffect::Untouched,
                    ),
                };
                self.handle_source_mutation_result(result);
            }
            RuntimeStateWriterCommand::Recovery(command) => {
                let result = match &command.operation {
                    RecoveryIoOperation::Inspect => RecoveryIoResult::Inspected(Err(
                        RuntimeStateInspectionError::new(error.message()),
                    )),
                    RecoveryIoOperation::PreserveInvalidIfUnchanged { mutation_id, .. } => {
                        RecoveryIoResult::SourceMutation(rejected_source_mutation(
                            *mutation_id,
                            error,
                        ))
                    }
                    RecoveryIoOperation::PersistCanonicalIfUnchanged { request, .. } => {
                        RecoveryIoResult::SourceMutation(rejected_source_mutation(
                            request.id, error,
                        ))
                    }
                };
                let completion = RecoveryIoCompletion {
                    controller_id: command.controller_id,
                    incident: command.incident,
                    barrier: command.barrier,
                    attempt: command.attempt,
                    command_id: command.command_id,
                    result,
                };
                let _ = self.controller.submit_persistence_recovery_io(completion);
            }
        }
        self.live_rebuild_pending |= self.controller.live_state() != &live_before;
    }

    pub(super) fn integrate_writer_completion(&mut self, completion: RuntimeStateWriterCompletion) {
        let live_before = self.controller.live_state().clone();
        match completion {
            RuntimeStateWriterCompletion::SourceMutation(result) => {
                self.handle_source_mutation_result(result);
            }
            RuntimeStateWriterCompletion::Recovery(completion) => {
                let result = self.controller.submit_persistence_recovery_io(completion);
                log::debug!("Integrated runtime UI recovery completion: {result:?}");
            }
        }
        self.live_rebuild_pending |= self.controller.live_state() != &live_before;
    }

    fn drop_stale_active_previews(&mut self) -> bool {
        let seeds = self.controller.seeds();
        let item_stale = self.item_drag.as_ref().is_some_and(|active| {
            let (controller_id, authority_epoch, guards) =
                runtime_preview_authority(&active.session);
            controller_id != self.controller.id()
                || authority_epoch != self.controller.authority_epoch()
                || guards.iter().any(|guard| !seeds.guard_is_current(guard))
        });
        if item_stale {
            self.item_drag = None;
        }
        let position_stale = self.position_drag.as_ref().is_some_and(|active| {
            active.session.permit.controller_id != self.controller.id()
                || active.session.permit.authority_epoch != self.controller.authority_epoch()
                || active
                    .session
                    .permit
                    .guards
                    .iter()
                    .any(|guard| !seeds.guard_is_current(guard))
        });
        if position_stale {
            self.position_drag = None;
        }
        item_stale || position_stale
    }

    pub(super) fn handle_source_mutation_result(&mut self, result: SourceMutationResult) {
        match self.controller.submit_source_mutation(result) {
            SubmitSourceMutationResult::ExternalReconciliationRequired {
                barrier, active, ..
            } => self.install_external_authority(barrier, active),
            SubmitSourceMutationResult::PersistenceUnhealthy {
                incident, error, ..
            } => log::warn!(
                "Runtime UI persistence is blocked by incident {:?}: {}",
                incident,
                error.message()
            ),
            SubmitSourceMutationResult::Rejected(error) => {
                log::error!("Runtime UI writer completion was rejected: {error:?}");
            }
            _ => {}
        }
    }

    fn install_external_authority(
        &mut self,
        barrier: ControllerBarrierId,
        observation: RuntimeStateSourceObservation,
    ) {
        let (status, wire) = match observation.revision.bytes() {
            None => (RuntimeUiFileStatus::Missing, RuntimeUiWireState::default()),
            Some(bytes) => {
                let decoded = decode_runtime_ui_file(bytes);
                (decoded.status, decoded.supported_wire.unwrap_or_default())
            }
        };
        let RuntimeUiWireState { model, passthrough } = wire;
        if let Err(error) = self.controller.install_external_authority(
            barrier,
            observation,
            status,
            model,
            passthrough,
        ) {
            log::warn!("Could not install externally changed runtime UI authority: {error:?}");
        }
    }
}

impl Drop for ToolbarRuntimeState {
    fn drop(&mut self) {
        self.shutdown_blocking();
    }
}
