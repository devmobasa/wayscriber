use super::*;

impl WaylandState {
    /// Apply a relative delta to toolbar offsets (used with locked pointer + relative motion).
    pub(in crate::backend::wayland) fn apply_toolbar_relative_delta(
        &mut self,
        kind: MoveDragKind,
        delta: (f64, f64),
    ) {
        if !self.toolbar_position_drag_update_allowed(kind) {
            return;
        }
        drag_log(|| {
            format!(
                "relative delta begin: kind={:?}, delta=({:.3}, {:.3}), offsets_before=({}, {})",
                kind,
                delta.0,
                delta.1,
                self.toolbar_chrome.top_offset().0,
                self.toolbar_chrome.top_offset().1
            )
        });
        let snapshot = self
            .toolbar
            .last_snapshot()
            .cloned()
            .unwrap_or_else(|| self.toolbar_snapshot());

        match kind {
            MoveDragKind::Top => {
                self.toolbar_chrome.add_top_offset(delta);
            }
        }

        self.apply_toolbar_offsets_throttled(&snapshot);

        drag_log(|| {
            format!(
                "relative delta applied: kind={:?}, delta=({:.3}, {:.3}), offsets=({}, {})",
                kind,
                delta.0,
                delta.1,
                self.toolbar_chrome.top_offset().0,
                self.toolbar_chrome.top_offset().1
            )
        });

        if self.inline_toolbars_render_active() {
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
        }
    }

    pub(in crate::backend::wayland) fn end_toolbar_move_drag(&mut self) {
        self.finish_toolbar_move_drag(true);
    }

    pub(in crate::backend::wayland) fn cancel_toolbar_move_drag(&mut self) {
        self.finish_toolbar_move_drag(false);
    }

    fn finish_toolbar_move_drag(&mut self, commit: bool) {
        let active_kind = self.toolbar_drag.kind();
        let Some(end) = self.toolbar_drag.end_move() else {
            return;
        };
        if commit && let Some(old_base_x) = end.commit_base {
            self.reconcile_top_base_after_drag(old_base_x);
        }
        drag_log(|| {
            format!(
                "end move drag: offsets=({}, {}), active_kind={:?}, pointer_locked={}",
                self.toolbar_chrome.top_offset().0,
                self.toolbar_chrome.top_offset().1,
                active_kind,
                self.pointer_lock_active()
            )
        });
        self.toolbar_chrome.set_pointer_over_toolbar(false);
        if end.pending_apply {
            let snapshot = self.toolbar_snapshot();
            let _ = self.apply_toolbar_offsets(&snapshot);
        }
        if end.had_preview {
            if commit {
                self.begin_toolbar_drag_handoff();
            } else {
                self.finish_toolbar_drag_handoff();
            }
        }
        self.finish_toolbar_position_preview(commit);
        self.unlock_pointer();
    }
}
