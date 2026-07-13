use super::*;
use crate::backend::wayland::state::helpers::toolbar_drag_handoff_delay;

fn reset_gtk_drag_lifecycle(
    preview: &mut Option<crate::toolbar_gtk::GtkToolbarKind>,
    handoff_at: &mut Option<Instant>,
    frozen_top_base_x: &mut Option<f64>,
    top_blocked: &mut bool,
    side_blocked: &mut bool,
) -> bool {
    let had_state = preview.is_some()
        || handoff_at.is_some()
        || frozen_top_base_x.is_some()
        || *top_blocked
        || *side_blocked;
    *preview = None;
    *handoff_at = None;
    *frozen_top_base_x = None;
    *top_blocked = false;
    *side_blocked = false;
    had_state
}

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

    pub(in crate::backend::wayland) fn cancel_gtk_toolbar_drag_lifecycle(&mut self) {
        if self.data.gtk_drag_preview.is_some() {
            self.reconcile_top_base_after_drag();
        }
        let had_state = reset_gtk_drag_lifecycle(
            &mut self.data.gtk_drag_preview,
            &mut self.data.toolbar_drag_handoff_at,
            &mut self.data.drag_top_base_x,
            &mut self.data.gtk_top_drag_blocked,
            &mut self.data.gtk_side_drag_blocked,
        );
        if had_state {
            drag_log("cancel GTK drag lifecycle (restore built-in toolbar rendering)");
        }
        self.request_toolbar_drag_flush();
        self.clear_inline_toolbar_hits();
        self.clear_inline_toolbar_hover();
        self.toolbar.mark_dirty();
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gtk_fallback_clears_the_entire_drag_lifecycle() {
        let mut preview = Some(crate::toolbar_gtk::GtkToolbarKind::Top);
        let mut handoff_at = Some(Instant::now());
        let mut frozen_top_base_x = Some(42.0);
        let mut top_blocked = true;
        let mut side_blocked = true;

        assert!(reset_gtk_drag_lifecycle(
            &mut preview,
            &mut handoff_at,
            &mut frozen_top_base_x,
            &mut top_blocked,
            &mut side_blocked,
        ));
        assert_eq!(preview, None);
        assert_eq!(handoff_at, None);
        assert_eq!(frozen_top_base_x, None);
        assert!(!top_blocked);
        assert!(!side_blocked);
    }
}
