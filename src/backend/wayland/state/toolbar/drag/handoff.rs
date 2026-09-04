use super::*;
use crate::backend::wayland::state::helpers::toolbar_drag_handoff_delay;

impl WaylandState {
    pub(in crate::backend::wayland) fn toolbar_drag_handoff_timeout(
        &self,
        now: Instant,
    ) -> Option<Duration> {
        self.toolbar_drag.handoff_timeout(now)
    }

    pub(in crate::backend::wayland) fn finish_toolbar_drag_handoff_if_due(
        &mut self,
        now: Instant,
    ) -> bool {
        let Some(end) = self.toolbar_drag.finish_handoff_if_due(now) else {
            return false;
        };
        self.apply_toolbar_drag_handoff_end(end);
        true
    }

    fn schedule_toolbar_drag_handoff(&mut self) {
        let delay = toolbar_drag_handoff_delay();
        if delay.is_zero() {
            self.finish_toolbar_drag_handoff();
            return;
        }
        drag_log(|| {
            format!(
                "schedule toolbar drag handoff after {}ms",
                delay.as_millis()
            )
        });
        self.toolbar_drag.begin_handoff(Instant::now() + delay);
        self.toolbar_chrome.clear_inline_hover();
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
    }

    pub(in crate::backend::wayland::state::toolbar::drag) fn begin_toolbar_drag_handoff(&mut self) {
        drag_log(|| "begin toolbar drag handoff (keep inline preview while layer surface settles)");
        let snapshot = self.toolbar_snapshot();
        let _ = self.apply_toolbar_offsets(&snapshot);
        self.toolbar_drag.request_flush();
        self.schedule_toolbar_drag_handoff();
    }

    pub(in crate::backend::wayland) fn begin_gtk_toolbar_drag_preview(
        &mut self,
        kind: crate::toolbar_gtk::GtkToolbarKind,
    ) {
        let frozen_top_base_x = self.inline_top_base_x();
        drag_log(|| {
            format!(
                "begin GTK {:?} drag preview (park transparent input surface, freeze top base at {frozen_top_base_x:.3})",
                kind,
            )
        });
        self.toolbar_drag.begin_gtk_preview(kind, frozen_top_base_x);
        self.toolbar.mark_dirty();
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
    }

    pub(in crate::backend::wayland) fn begin_gtk_toolbar_drag_handoff(&mut self) {
        if self.toolbar_drag.gtk_preview_kind().is_none() {
            return;
        }
        drag_log(|| "begin GTK drag handoff (move transparent surface before reveal)");
        self.toolbar_drag.request_flush();
        self.schedule_toolbar_drag_handoff();
    }

    pub(in crate::backend::wayland) fn cancel_gtk_toolbar_drag_lifecycle(&mut self) {
        let had_preview = self.toolbar_drag.gtk_preview_kind().is_some();
        if had_preview {
            self.finish_toolbar_position_preview(false);
        }
        let had_state = self.toolbar_drag.cancel_gtk();
        if had_state {
            drag_log(|| "cancel GTK drag lifecycle (restore built-in toolbar rendering)");
        }
        self.toolbar_drag.request_flush();
        self.toolbar_chrome.clear_inline_hits();
        self.toolbar_chrome.clear_inline_hover();
        self.toolbar.mark_dirty();
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
    }

    pub(in crate::backend::wayland::state::toolbar::drag) fn finish_toolbar_drag_handoff(
        &mut self,
    ) {
        let Some(end) = self.toolbar_drag.finish_handoff() else {
            return;
        };
        self.apply_toolbar_drag_handoff_end(end);
    }

    fn apply_toolbar_drag_handoff_end(&mut self, end: HandoffEnd) {
        if end == HandoffEnd::Gtk {
            drag_log(|| "finish GTK drag handoff (reveal surface at final position)");
            self.toolbar_drag.request_flush();
            self.toolbar_chrome.clear_inline_hits();
            self.toolbar_chrome.clear_inline_hover();
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
            return;
        }

        drag_log(|| "finish toolbar drag handoff (restore layer-shell toolbars)");
        let snapshot = self.toolbar_snapshot();
        let _ = self.apply_toolbar_offsets(&snapshot);
        self.toolbar
            .set_suppressed(self.protocol.compositor(), false);
        self.toolbar_drag.request_flush();
        self.toolbar_chrome.clear_inline_hits();
        self.toolbar_chrome.clear_inline_hover();
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
    }
}
