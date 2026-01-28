use super::*;

impl WaylandState {
    /// Apply a relative delta to toolbar offsets (used with locked pointer + relative motion).
    pub(in crate::backend::wayland) fn apply_toolbar_relative_delta(
        &mut self,
        kind: MoveDragKind,
        delta: (f64, f64),
    ) {
        drag_log(format!(
            "relative delta begin: kind={:?}, delta=({:.3}, {:.3}), offsets_before=({}, {})/({}, {})",
            kind,
            delta.0,
            delta.1,
            self.data.toolbar_top_offset,
            self.data.toolbar_top_offset_y,
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset
        ));
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

        self.apply_toolbar_offsets_throttled(&snapshot);

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
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
        }
    }

    pub(in crate::backend::wayland) fn end_toolbar_move_drag(&mut self) {
        if self.data.toolbar_move_drag.is_some() {
            // Preserve the top toolbar's screen position if its base X would change on release.
            if let Some(old_base_x) = self.data.drag_top_base_x {
                let snapshot = self.toolbar_snapshot();
                let side_visible = self.toolbar.is_side_visible();
                let side_size = side_size(&snapshot);
                let top_size = top_size(&snapshot);
                let side_start_y = Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset;
                let top_bottom_y =
                    Self::INLINE_TOP_Y + self.data.toolbar_top_offset_y + top_size.1 as f64;
                let base = Self::INLINE_SIDE_X;
                let new_base_x = geometry::compute_inline_top_base_x(
                    base,
                    side_visible,
                    side_size.0 as f64,
                    side_start_y,
                    top_bottom_y,
                    Self::INLINE_TOP_PUSH,
                    true,
                );
                let delta = old_base_x - new_base_x;
                if delta.abs() > 0.01 {
                    self.data.toolbar_top_offset += delta;
                    drag_log(format!(
                        "end move drag: preserve top position, old_base_x={:.3}, new_base_x={:.3}, delta={:.3}, top_offset=({}, {})",
                        old_base_x,
                        new_base_x,
                        delta,
                        self.data.toolbar_top_offset,
                        self.data.toolbar_top_offset_y
                    ));
                }
            }
            drag_log(format!(
                "end move drag: offsets=({}, {})/({}, {}), active_kind={:?}, pointer_locked={}",
                self.data.toolbar_top_offset,
                self.data.toolbar_top_offset_y,
                self.data.toolbar_side_offset_x,
                self.data.toolbar_side_offset,
                self.data.active_drag_kind,
                self.pointer_lock_active()
            ));
            self.data.toolbar_move_drag = None;
            self.set_toolbar_dragging(false);
            self.set_pointer_over_toolbar(false);
            self.data.active_drag_kind = None;
            self.data.drag_top_base_x = None;
            self.data.drag_top_base_y = None;
            self.data.last_toolbar_drag_apply = None;
            if self.data.toolbar_drag_pending_apply {
                let snapshot = self.toolbar_snapshot();
                let _ = self.apply_toolbar_offsets(&snapshot);
                self.data.toolbar_drag_pending_apply = false;
            }
            if self.toolbar_drag_preview_active() {
                drag_log("disable inline drag preview (restore layer-shell toolbars)");
                // Turn off preview first so apply_toolbar_offsets can update layer-surface margins.
                self.set_toolbar_drag_preview_active(false);
                // Apply final offsets to the (still suppressed) layer-shell toolbars.
                let snapshot = self.toolbar_snapshot();
                let _ = self.apply_toolbar_offsets(&snapshot);
                self.toolbar.set_suppressed(&self.compositor_state, false);
                self.clear_inline_toolbar_hits();
                self.clear_inline_toolbar_hover();
                self.input_state.dirty_tracker.mark_full();
                self.input_state.needs_redraw = true;
            }
            self.save_toolbar_pin_config();
            self.unlock_pointer();
        }
    }
}
