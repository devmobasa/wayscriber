use super::*;

impl WaylandState {
    /// Apply a relative delta to toolbar offsets (used with locked pointer + relative motion).
    pub(in crate::backend::wayland) fn apply_toolbar_relative_delta(
        &mut self,
        kind: MoveDragKind,
        delta: (f64, f64),
    ) {
        let snapshot = self
            .toolbar
            .last_snapshot()
            .cloned()
            .unwrap_or_else(|| self.toolbar_snapshot());

        match kind {
            MoveDragKind::Top => {
                self.data.toolbar_top_offset += delta.0;
                self.data.toolbar_top_offset_y += delta.1;
            }
            MoveDragKind::Side => {
                self.data.toolbar_side_offset_x += delta.0;
                self.data.toolbar_side_offset += delta.1;
            }
        }

        let _ = self.apply_toolbar_offsets(&snapshot);

        drag_log(format!(
            "relative delta applied: kind={:?}, delta=({:.3}, {:.3}), offsets=({}, {})/({}, {})",
            kind,
            delta.0,
            delta.1,
            self.data.toolbar_top_offset,
            self.data.toolbar_top_offset_y,
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset
        ));

        if self.inline_toolbars_render_active() {
            self.input_state.needs_redraw = true;
        }
    }

    pub(in crate::backend::wayland) fn end_toolbar_move_drag(&mut self) {
        if self.data.toolbar_move_drag.is_some() {
            self.data.toolbar_move_drag = None;
            self.set_toolbar_dragging(false);
            self.set_pointer_over_toolbar(false);
            self.data.active_drag_kind = None;
            self.data.drag_top_base_x = None;
            self.data.drag_top_base_y = None;
            if self.toolbar_drag_preview_active() {
                drag_log("disable inline drag preview (restore layer-shell toolbars)");
                self.set_toolbar_drag_preview_active(false);
                self.toolbar.set_suppressed(&self.compositor_state, false);
                self.clear_inline_toolbar_hits();
                self.clear_inline_toolbar_hover();
                self.input_state.needs_redraw = true;
            }
            self.save_toolbar_pin_config();
            self.unlock_pointer();
        }
    }
}
