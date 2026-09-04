use super::super::state::{MoveDragKind, WaylandState};
use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn toolbar_position_snapshot(&self) -> ToolbarPositionSnapshot {
        ToolbarPositionSnapshot {
            top: (
                self.toolbar_chrome.top_offset().0,
                self.toolbar_chrome.top_offset().1,
            ),
        }
    }

    pub(in crate::backend::wayland) fn apply_toolbar_runtime_finish(
        &mut self,
        finish: ToolbarRuntimeFinish,
    ) {
        let ToolbarRuntimeFinish::Rollback(rollback) = finish else {
            return;
        };
        let mut positions = self.toolbar_position_snapshot();
        apply_toolbar_runtime_rollback(
            self.render.ui_text(),
            &mut self.input_state,
            &mut positions,
            &rollback,
        );
        self.toolbar_chrome.set_top_offset(positions.top);
        self.toolbar.mark_dirty();
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
    }

    pub(in crate::backend::wayland) fn finish_toolbar_item_drag(&mut self, commit: bool) {
        let finish = match self.preferences.runtime_ui_mut().state_mut() {
            Some(runtime) => runtime.finish_item_drag(commit, &self.input_state),
            None => self
                .preferences
                .runtime_ui_mut()
                .unavailable_previews_mut()
                .finish_item_drag(commit),
        };
        self.input_state.clear_toolbar_item_drag();
        self.apply_toolbar_runtime_finish(finish);
    }

    pub(in crate::backend::wayland) fn begin_toolbar_item_drag_preview(
        &mut self,
        group: ToolbarItemOrderGroup,
    ) -> bool {
        match self.preferences.runtime_ui_mut().state_mut() {
            Some(runtime) => runtime.begin_item_drag(group, &self.input_state),
            None => self
                .preferences
                .runtime_ui_mut()
                .unavailable_previews_mut()
                .begin_item_drag(group, &self.input_state),
        }
    }

    pub(in crate::backend::wayland) fn toolbar_item_drag_update_allowed(&self) -> bool {
        match self.preferences.runtime_ui().state() {
            Some(runtime) => runtime.item_drag_update_allowed(),
            None => self
                .preferences
                .runtime_ui()
                .unavailable_previews()
                .item_drag_update_allowed(),
        }
    }

    pub(in crate::backend::wayland) fn toolbar_position_drag_update_allowed(
        &self,
        kind: MoveDragKind,
    ) -> bool {
        match self.preferences.runtime_ui().state() {
            Some(runtime) => runtime.position_drag_update_allowed(kind),
            None => self
                .preferences
                .runtime_ui()
                .unavailable_previews()
                .position_drag_update_allowed(kind),
        }
    }

    pub(in crate::backend::wayland) fn begin_toolbar_position_preview(
        &mut self,
        kind: MoveDragKind,
    ) -> bool {
        let positions = self.toolbar_position_snapshot();
        match self.preferences.runtime_ui_mut().state_mut() {
            Some(runtime) => runtime.begin_position_drag(kind, positions),
            None => self
                .preferences
                .runtime_ui_mut()
                .unavailable_previews_mut()
                .begin_position_drag(kind, positions),
        }
    }

    /// Commit or cancel a toolbar move drag.
    ///
    /// A committed drag writes `runtime-ui.toml` overrides only; the authored
    /// `ui.toolbar.*_offset*` values stay the seeds the configurator edits.
    pub(in crate::backend::wayland) fn finish_toolbar_position_preview(&mut self, commit: bool) {
        let positions = self.toolbar_position_snapshot();
        let finish = match self.preferences.runtime_ui_mut().state_mut() {
            Some(runtime) => runtime.finish_position_drag(commit, positions),
            None => self
                .preferences
                .runtime_ui_mut()
                .unavailable_previews_mut()
                .finish_position_drag(commit),
        };
        self.apply_toolbar_runtime_finish(finish);
    }

    /// Persist the top-display mode reached by its keyboard action.
    ///
    /// The cycle already applied in `InputState`, so the pre-change mode
    /// travels with the pending action and supplies the preview's rollback.
    pub(in crate::backend::wayland) fn persist_toolbar_display_mode(
        &mut self,
        previous: crate::config::TopDisplayMode,
    ) {
        let target = ToolbarRuntimeUiPersistenceTarget::TopDisplayMode;
        let rollback = match top_display_mode_values(previous, &self.input_state) {
            Ok(values) => values,
            Err(error) => {
                log::error!("Toolbar display cycle has invalid rollback values: {error:?}");
                return;
            }
        };
        // One borrow for both halves of the mutation: `input_state` is a
        // disjoint field, so nothing here has to hand the runtime back and
        // reacquire it between beginning and finishing.
        let Some(runtime) = self.preferences.runtime_ui_mut().state_mut() else {
            return;
        };
        let Some(prepared) = runtime.begin_toolbar_mutation_with_rollback(target, rollback) else {
            return;
        };
        let finish = runtime.finish_toolbar_mutation(prepared, true, &self.input_state);
        self.apply_toolbar_runtime_finish(finish);
    }

    /// Persist both pin flags reached by the keyboard visibility toggle.
    ///
    /// The toggle already applied in `InputState`, so the pre-change pins
    /// travel with the pending action and supply the preview's rollback.
    /// Without a runtime store the toggle stays run-only, like the display
    /// cycle in the same degraded mode.
    pub(in crate::backend::wayland) fn persist_toolbar_visibility(
        &mut self,
        previous_top_pinned: bool,
    ) {
        let target = ToolbarRuntimeUiPersistenceTarget::ToolbarVisibility;
        let rollback = match RuntimeUiMutationValues::one(
            InteractionSeedTarget::TopPinned,
            InteractionSeedValue::Bool(previous_top_pinned),
        ) {
            Ok(values) => values,
            Err(error) => {
                log::error!("Toolbar visibility toggle has invalid rollback values: {error:?}");
                return;
            }
        };
        let Some(runtime) = self.preferences.runtime_ui_mut().state_mut() else {
            return;
        };
        let Some(prepared) = runtime.begin_toolbar_mutation_with_rollback(target, rollback) else {
            return;
        };
        let finish = runtime.finish_toolbar_mutation(prepared, true, &self.input_state);
        self.apply_toolbar_runtime_finish(finish);
    }

    /// Persists a click-highlight change the user made outside the toolbar --
    /// a keyboard action or the command palette. Those apply inside
    /// `InputState` before the backend sees them, so the caller supplies the
    /// pre-change values as the rollback.
    pub(in crate::backend::wayland) fn persist_click_highlight(
        &mut self,
        previous_enabled: bool,
        previous_tool_ring: bool,
    ) {
        let target = ToolbarRuntimeUiPersistenceTarget::ClickHighlight;
        let rollback = match super::click_highlight_values(previous_enabled, previous_tool_ring) {
            Ok(values) => values,
            Err(error) => {
                log::error!("Click highlight toggle has invalid rollback values: {error:?}");
                return;
            }
        };
        let Some(runtime) = self.preferences.runtime_ui_mut().state_mut() else {
            return;
        };
        let Some(prepared) = runtime.begin_toolbar_mutation_with_rollback(target, rollback) else {
            return;
        };
        let finish = runtime.finish_toolbar_mutation(prepared, true, &self.input_state);
        self.apply_toolbar_runtime_finish(finish);
    }

    /// Persists one boolean chrome toggle the user made outside the toolbar.
    ///
    /// Keyboard and command-palette changes apply inside `InputState` before
    /// the backend sees them, so the caller supplies the pre-change value as
    /// the rollback rather than letting it be read back off the live state.
    pub(in crate::backend::wayland) fn persist_keyboard_chrome_toggle(
        &mut self,
        target: ToolbarRuntimeUiPersistenceTarget,
        rollback_value: bool,
    ) {
        let Some(seed_target) = super::single_bool_seed_target(target) else {
            log::error!("{target:?} is not a single boolean chrome toggle");
            return;
        };
        let rollback = match RuntimeUiMutationValues::one(
            seed_target,
            InteractionSeedValue::Bool(rollback_value),
        ) {
            Ok(values) => values,
            Err(error) => {
                log::error!("{target:?} has an invalid rollback value: {error:?}");
                return;
            }
        };
        let Some(runtime) = self.preferences.runtime_ui_mut().state_mut() else {
            return;
        };
        let Some(prepared) = runtime.begin_toolbar_mutation_with_rollback(target, rollback) else {
            return;
        };
        let finish = runtime.finish_toolbar_mutation(prepared, true, &self.input_state);
        self.apply_toolbar_runtime_finish(finish);
    }

    /// Drains every queued durable toolbar change into the runtime-ui
    /// writer, oldest first. Called on every event-loop pass and once more
    /// at teardown before the writer shuts down, so a toggle pressed in the
    /// same input batch as an exit request still reaches the file.
    ///
    /// While a reset/recovery barrier is active the queue is left intact:
    /// `begin` would refuse each entry with `ControllerBusy`, silently
    /// losing it. Entries wait for the barrier to resolve instead — the
    /// take's no-op filter then discards any the resolution made moot. At
    /// exit, `drain_toolbar_persistence_for_teardown` settles a settleable
    /// barrier first; only one that cannot settle (a persistence incident,
    /// which no write can land under) still costs the entries their write.
    pub(in crate::backend::wayland) fn drain_pending_toolbar_persistence(&mut self) {
        use crate::input::state::PendingToolbarPersistence;

        match self.preferences.runtime_ui().state() {
            // Degraded mode is run-only: consume the entries so the queue
            // cannot wake the loop for writes that have nowhere to land.
            None => {
                self.input_state.take_pending_toolbar_persistence();
                return;
            }
            Some(runtime) if runtime.mutation_barrier_active() => return,
            Some(_) => {}
        }
        for entry in self.input_state.take_pending_toolbar_persistence() {
            match entry {
                PendingToolbarPersistence::DisplayMode { previous } => {
                    self.persist_toolbar_display_mode(previous)
                }
                PendingToolbarPersistence::Visibility {
                    previous_top_pinned,
                } => self.persist_toolbar_visibility(previous_top_pinned),
                PendingToolbarPersistence::StatusBar { previous } => self
                    .persist_keyboard_chrome_toggle(
                        ToolbarRuntimeUiPersistenceTarget::StatusBar,
                        previous,
                    ),
                PendingToolbarPersistence::FloatingBadge { previous } => self
                    .persist_keyboard_chrome_toggle(
                        ToolbarRuntimeUiPersistenceTarget::FloatingBadge,
                        previous,
                    ),
                PendingToolbarPersistence::ZoomChip { previous } => self
                    .persist_keyboard_chrome_toggle(
                        ToolbarRuntimeUiPersistenceTarget::ZoomChip,
                        previous,
                    ),
                PendingToolbarPersistence::InputHud { previous } => self
                    .persist_keyboard_chrome_toggle(
                        ToolbarRuntimeUiPersistenceTarget::InputHud,
                        previous,
                    ),
                PendingToolbarPersistence::ClickHighlight {
                    previous_enabled,
                    previous_tool_ring,
                } => self.persist_click_highlight(previous_enabled, previous_tool_ring),
            }
        }
    }

    /// Teardown-time drain: settle any barrier whose resolution is already
    /// on the writer channel — exiting mid-reset must not cost a queued
    /// toggle its write — apply that resolution to live state so the
    /// drain's no-op filter judges against the post-barrier screen, then
    /// drain. The caller's writer shutdown flushes the writes to disk.
    pub(in crate::backend::wayland) fn drain_toolbar_persistence_for_teardown(&mut self) {
        if let Some(runtime) = self.preferences.runtime_ui_mut().state_mut() {
            runtime.settle_barrier_for_teardown();
        }
        self.drain_runtime_ui_completions();
        self.drain_pending_toolbar_persistence();
    }

    /// Whether the toolbar persistence drain could make progress this pass.
    ///
    /// Gates the zero-timeout wake: entries deferred behind a barrier must
    /// not busy-spin the loop — the writer completion that resolves the
    /// barrier wakes it instead. A missing runtime store still reports
    /// ready so the drain can consume the entries it will never write.
    pub(in crate::backend::wayland) fn toolbar_persistence_drain_ready(&self) -> bool {
        self.input_state.has_pending_toolbar_persistence()
            && self
                .preferences
                .runtime_ui()
                .state()
                .is_none_or(|runtime| !runtime.mutation_barrier_active())
    }

    /// Reconcile runtime overrides and active previews after an authored
    /// config reload. The product's current reload path may still restart the
    /// daemon, but keeping this boundary complete prevents a future
    /// same-process reload from committing an old drag under new seeds.
    pub(in crate::backend::wayland) fn refresh_runtime_ui_config_seeds(&mut self) {
        let configured_boards = self.config.resolved_boards();
        self.input_state
            .boards
            .sync_pin_seeds_from_config(&configured_boards);
        let mut positions = self.toolbar_position_snapshot();
        let Some(runtime) = self.preferences.runtime_ui_mut().state_mut() else {
            return;
        };
        let refresh = runtime.refresh_config_seeds(
            self.render.ui_text(),
            &self.config,
            &mut self.input_state,
            &mut positions,
        );
        if !refresh.applied {
            return;
        }
        if refresh.item_drag_aborted {
            self.input_state.clear_toolbar_item_drag();
            self.toolbar_drag.set_item_dragging(false);
        }
        if refresh.position_drag_aborted {
            self.cancel_toolbar_move_drag();
            self.cancel_gtk_toolbar_drag_lifecycle();
        }
        self.toolbar_chrome.set_top_offset(positions.top);
        self.toolbar.mark_dirty();
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
    }

    pub(in crate::backend::wayland) fn apply_board_runtime_ui_action(
        &mut self,
        action: crate::input::boards::PendingBoardRuntimeUiAction,
    ) {
        use crate::input::boards::PendingBoardRuntimeUiAction;

        match action {
            PendingBoardRuntimeUiAction::TogglePin {
                board_id,
                board_identity_generation,
                pin_seed,
            } => {
                if self.input_state.boards.board_identity_generation() != board_identity_generation
                {
                    return;
                }
                let Some(current) = self
                    .input_state
                    .boards
                    .board_states()
                    .iter()
                    .find(|board| board.spec.id == board_id)
                    .map(|board| board.spec.pinned)
                else {
                    return;
                };
                let Some(runtime) = self.preferences.runtime_ui_mut().state_mut() else {
                    self.input_state
                        .apply_board_pinned_runtime(&board_id, !current);
                    return;
                };
                let Some(prepared) =
                    runtime.begin_board_pin_toggle(&self.config, board_id, pin_seed, current)
                else {
                    return;
                };
                let applied = self
                    .input_state
                    .apply_board_pinned_runtime(&prepared.board_id, prepared.desired);
                let finish = self
                    .preferences
                    .runtime_ui_mut()
                    .state_mut()
                    .expect("runtime state remained available")
                    .finish_board_pin_toggle(prepared, applied);
                self.apply_toolbar_runtime_finish(finish);
            }
            PendingBoardRuntimeUiAction::IdentityDeleted { board_id } => {
                if let Some(runtime) = self.preferences.runtime_ui_mut().state_mut() {
                    runtime.remove_board_identity(&self.config, &board_id);
                }
            }
            PendingBoardRuntimeUiAction::IdentityAvailable {
                board_id,
                pin_seed,
                pinned,
            } => {
                let finish = self
                    .preferences
                    .runtime_ui_mut()
                    .state_mut()
                    .and_then(|runtime| {
                        runtime.restore_board_identity(
                            &self.config,
                            &mut self.input_state,
                            board_id,
                            pin_seed,
                            pinned,
                        )
                    });
                if let Some(finish) = finish {
                    self.apply_toolbar_runtime_finish(finish);
                }
            }
        }
    }

    pub(in crate::backend::wayland) fn drain_runtime_ui_completions(&mut self) {
        let drain = self
            .preferences
            .runtime_ui_mut()
            .state_mut()
            .map(ToolbarRuntimeState::drain_writer_completions)
            .unwrap_or_default();
        for rollback in drain.rollbacks {
            self.apply_toolbar_runtime_finish(ToolbarRuntimeFinish::Rollback(rollback));
        }
        if drain.rebuild_live {
            self.input_state.clear_toolbar_item_drag();
            self.toolbar_drag.set_item_dragging(false);
            self.cancel_toolbar_move_drag();
            self.cancel_gtk_toolbar_drag_lifecycle();
            let mut positions = self.toolbar_position_snapshot();
            if let Some(runtime) = self.preferences.runtime_ui().state() {
                runtime.apply_live_state(
                    self.render.ui_text(),
                    &mut self.input_state,
                    &mut positions,
                );
            }
            self.toolbar_chrome.set_top_offset(positions.top);
            self.toolbar.mark_dirty();
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
        }
        if drain.lifecycle_changed {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
        }
        let deferred_finishes = self
            .preferences
            .runtime_ui_mut()
            .state_mut()
            .map(|runtime| runtime.finish_deferred_board_pin_restores(&mut self.input_state))
            .unwrap_or_default();
        for finish in deferred_finishes {
            self.apply_toolbar_runtime_finish(finish);
        }
    }

    pub(in crate::backend::wayland) fn shutdown_runtime_ui(&mut self) {
        if let Some(runtime) = self.preferences.runtime_ui_mut().state_mut() {
            runtime.shutdown_blocking();
        }
    }
}
