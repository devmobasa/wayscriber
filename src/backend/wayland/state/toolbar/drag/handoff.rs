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

    fn finish_toolbar_drag_handoff(&mut self) {
        self.data.toolbar_drag_handoff_at = None;
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
