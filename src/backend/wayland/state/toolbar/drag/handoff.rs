use super::*;
use crate::backend::wayland::state::helpers::toolbar_drag_handoff_delay;

impl WaylandState {
    pub(in crate::backend::wayland) fn toolbar_drag_handoff_timeout(
        &self,
        now: Instant,
    ) -> Option<Duration> {
        self.data
            .toolbar_drag_handoff_at
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    pub(in crate::backend::wayland) fn finish_toolbar_drag_handoff_if_due(
        &mut self,
        now: Instant,
    ) -> bool {
        let Some(deadline) = self.data.toolbar_drag_handoff_at else {
            return false;
        };
        if now < deadline {
            return false;
        }
        self.finish_toolbar_drag_handoff();
        true
    }

    fn schedule_toolbar_drag_handoff(&mut self) {
        let delay = toolbar_drag_handoff_delay();
        if delay.is_zero() {
            self.finish_toolbar_drag_handoff();
            return;
        }
        drag_log(format!(
            "schedule toolbar drag handoff after {}ms",
            delay.as_millis()
        ));
        self.data.toolbar_drag_handoff_at = Some(Instant::now() + delay);
        self.clear_inline_toolbar_hover();
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
    }

    pub(in crate::backend::wayland::state::toolbar::drag) fn begin_toolbar_drag_handoff(&mut self) {
        drag_log("begin toolbar drag handoff (keep inline preview while layer surface settles)");
        let snapshot = self.toolbar_snapshot();
        let _ = self.apply_toolbar_offsets(&snapshot);
        self.request_toolbar_drag_flush();
        self.schedule_toolbar_drag_handoff();
    }

    pub(in crate::backend::wayland) fn begin_gtk_toolbar_drag_preview(
        &mut self,
        kind: crate::toolbar_gtk::GtkToolbarKind,
    ) {
        let snapshot = self.toolbar_snapshot();
        let frozen_top_base_x = self.inline_top_base_x(&snapshot);
        drag_log(format!(
            "begin GTK {:?} drag preview (park transparent input surface, freeze top base at {frozen_top_base_x:.3})",
            kind,
        ));
        self.data.toolbar_drag_handoff_at = None;
        self.data.drag_top_base_x = Some(frozen_top_base_x);
        self.data.gtk_drag_preview = Some(kind);
        self.toolbar.mark_dirty();
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
    }

    pub(in crate::backend::wayland) fn begin_gtk_toolbar_drag_handoff(&mut self) {
        if self.data.gtk_drag_preview.is_none() {
            return;
        }
        drag_log("begin GTK drag handoff (move transparent surface before reveal)");
        self.request_toolbar_drag_flush();
        self.schedule_toolbar_drag_handoff();
    }

    fn finish_toolbar_drag_handoff(&mut self) {
        self.data.toolbar_drag_handoff_at = None;
        if self.data.gtk_drag_preview.take().is_some() {
            drag_log("finish GTK drag handoff (reveal surface at final position)");
            self.request_toolbar_drag_flush();
            self.clear_inline_toolbar_hits();
            self.clear_inline_toolbar_hover();
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
            return;
        }
        if !self.toolbar_drag_preview_active() {
            return;
        }
        drag_log("finish toolbar drag handoff (restore layer-shell toolbars)");
        self.set_toolbar_drag_preview_active(false);
        let snapshot = self.toolbar_snapshot();
        let _ = self.apply_toolbar_offsets(&snapshot);
        self.toolbar.set_suppressed(&self.compositor_state, false);
        self.request_toolbar_drag_flush();
        self.clear_inline_toolbar_hits();
        self.clear_inline_toolbar_hover();
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
    }
}
