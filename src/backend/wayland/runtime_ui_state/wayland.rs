use super::super::state::{MoveDragKind, WaylandState};
use super::*;

impl WaylandState {
    fn reset_runtime_side_focus_if_pane_changed(
        &mut self,
        previous_pane: crate::ui::toolbar::SidePane,
    ) {
        if self.input_state.toolbar_side_pane != previous_pane {
            self.reset_side_toolbar_focus();
        }
    }

    pub(in crate::backend::wayland) fn toolbar_position_snapshot(&self) -> ToolbarPositionSnapshot {
        ToolbarPositionSnapshot {
            top: (self.toolbar_top_offset(), self.toolbar_top_offset_y()),
            side: (self.toolbar_side_offset_x(), self.toolbar_side_offset()),
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
        let previous_pane = self.input_state.toolbar_side_pane;
        apply_toolbar_runtime_rollback(&mut self.input_state, &mut positions, &rollback);
        self.reset_runtime_side_focus_if_pane_changed(previous_pane);
        self.restore_toolbar_offsets(positions.top, positions.side);
        self.toolbar.mark_dirty();
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
    }

    pub(in crate::backend::wayland) fn finish_toolbar_item_drag(&mut self, commit: bool) {
        let finish = self
            .runtime_ui
            .as_mut()
            .map(|runtime| runtime.finish_item_drag(commit, &self.input_state));
        self.input_state.clear_toolbar_item_drag();
        if let Some(finish) = finish {
            self.apply_toolbar_runtime_finish(finish);
        }
    }

    pub(in crate::backend::wayland) fn toolbar_item_drag_update_allowed(&self) -> bool {
        self.runtime_ui
            .as_ref()
            .is_none_or(ToolbarRuntimeState::item_drag_update_allowed)
    }

    pub(in crate::backend::wayland) fn toolbar_position_drag_update_allowed(
        &self,
        kind: MoveDragKind,
    ) -> bool {
        let target = match kind {
            MoveDragKind::Top => ConfigPositionTarget::Top,
            MoveDragKind::Side => ConfigPositionTarget::Side,
        };
        self.runtime_ui
            .as_ref()
            .is_none_or(|runtime| runtime.position_drag_update_allowed(target))
    }

    pub(in crate::backend::wayland) fn begin_toolbar_position_preview(
        &mut self,
        kind: MoveDragKind,
    ) -> bool {
        let positions = self.toolbar_position_snapshot();
        let Some(runtime) = self.runtime_ui.as_mut() else {
            return true;
        };
        let target = match kind {
            MoveDragKind::Top => ConfigPositionTarget::Top,
            MoveDragKind::Side => ConfigPositionTarget::Side,
        };
        runtime.begin_position_drag(target, positions)
    }

    pub(in crate::backend::wayland) fn finish_toolbar_position_preview(
        &mut self,
        kind: MoveDragKind,
        commit: bool,
    ) {
        let positions = self.toolbar_position_snapshot();
        let Some(runtime) = self.runtime_ui.as_mut() else {
            if commit {
                self.save_toolbar_position_config(kind);
            }
            return;
        };
        let mut next_config = self.config.clone();
        let (finish, applied_config) =
            runtime.finish_position_drag(commit, positions, |target, position| {
                let accepted = (position.x.get(), position.y.get());
                match target {
                    ConfigPositionTarget::Top => {
                        next_config.ui.toolbar.top_offset = accepted.0;
                        next_config.ui.toolbar.top_offset_y = accepted.1;
                    }
                    ConfigPositionTarget::Side => {
                        next_config.ui.toolbar.top_offset = positions.top.0;
                        next_config.ui.toolbar.side_offset_x = accepted.0;
                        next_config.ui.toolbar.side_offset = accepted.1;
                    }
                }
                next_config
                    .save()
                    .map_err(|error| ConfigMutationError::new(error.to_string()))
            });
        if applied_config {
            self.config = next_config;
        }
        self.apply_toolbar_runtime_finish(finish);
        if applied_config {
            self.refresh_runtime_ui_config_seeds();
        }
    }

    /// Reconcile runtime overrides and active previews after an authored
    /// config reload. The product's current reload path may still restart the
    /// daemon, but keeping this boundary complete prevents a future
    /// same-process reload from committing an old drag under new seeds.
    pub(in crate::backend::wayland) fn refresh_runtime_ui_config_seeds(&mut self) {
        let mut positions = self.toolbar_position_snapshot();
        let previous_pane = self.input_state.toolbar_side_pane;
        let Some(runtime) = self.runtime_ui.as_mut() else {
            return;
        };
        let refresh =
            runtime.refresh_config_seeds(&self.config, &mut self.input_state, &mut positions);
        if !refresh.applied {
            return;
        }
        if refresh.item_drag_aborted {
            self.input_state.clear_toolbar_item_drag();
            self.set_toolbar_dragging(false);
        }
        if refresh.position_drag_aborted {
            self.cancel_toolbar_move_drag();
            self.cancel_gtk_toolbar_drag_lifecycle();
        }
        self.reset_runtime_side_focus_if_pane_changed(previous_pane);
        self.restore_toolbar_offsets(positions.top, positions.side);
        self.toolbar.mark_dirty();
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
    }

    pub(in crate::backend::wayland) fn drain_runtime_ui_completions(&mut self) {
        let drain = self
            .runtime_ui
            .as_mut()
            .map(ToolbarRuntimeState::drain_writer_completions)
            .unwrap_or_default();
        for rollback in drain.rollbacks {
            self.apply_toolbar_runtime_finish(ToolbarRuntimeFinish::Rollback(rollback));
        }
        if drain.rebuild_live {
            self.input_state.clear_toolbar_item_drag();
            self.set_toolbar_dragging(false);
            self.cancel_toolbar_move_drag();
            self.cancel_gtk_toolbar_drag_lifecycle();
            let mut positions = self.toolbar_position_snapshot();
            let previous_pane = self.input_state.toolbar_side_pane;
            if let Some(runtime) = self.runtime_ui.as_ref() {
                runtime.apply_live_state(&mut self.input_state, &mut positions);
            }
            self.reset_runtime_side_focus_if_pane_changed(previous_pane);
            self.restore_toolbar_offsets(positions.top, positions.side);
            self.toolbar.mark_dirty();
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
        }
    }

    pub(in crate::backend::wayland) fn shutdown_runtime_ui(&mut self) {
        if let Some(runtime) = self.runtime_ui.as_mut() {
            runtime.shutdown_blocking();
        }
    }
}
