use super::state::MoveSample;
use super::*;

impl WaylandState {
    pub(in crate::backend::wayland::state::toolbar) fn begin_toolbar_move_drag(
        &mut self,
        kind: MoveDragKind,
        coord: (f64, f64),
        coord_is_screen: bool,
    ) -> bool {
        if !self.toolbar_drag.is_moving() {
            if !self.begin_toolbar_position_preview(kind) {
                return false;
            }
            if toolbar_drag_preview_enabled()
                && self.protocol.layer_shell().is_some()
                && !self.toolbar_chrome.inline_toolbars()
                && !self.toolbar_drag.preview_active()
            {
                drag_log(|| "enable inline drag preview (layer-shell toolbars hidden)");
                self.toolbar_drag.set_preview_active(true);
                self.toolbar
                    .set_suppressed(self.protocol.compositor(), true);
                self.input_state.dirty_tracker.mark_full();
                self.input_state.needs_redraw = true;
            }
            log::debug!(
                "Begin toolbar move drag: kind={:?}, coord=({:.3}, {:.3}), coord_is_screen={}",
                kind,
                coord.0,
                coord.1,
                coord_is_screen
            );
            drag_log(|| {
                format!(
                    "begin move drag: kind={:?}, coord=({:.3}, {:.3}), coord_is_screen={}, inline_active={}, layer_shell={}",
                    kind,
                    coord.0,
                    coord.1,
                    coord_is_screen,
                    self.toolbar_chrome.inline_toolbars(),
                    self.protocol.layer_shell().is_some()
                )
            });
            // Freeze the base position so a relayout cannot shift the surface
            // under the pointer mid-drag.
            let top_base_x = self.inline_top_base_x();
            let top_base_y = self.inline_top_base_y();
            drag_log(|| {
                format!(
                    "begin move drag snapshot: kind={:?}, top_base=({:.3}, {:.3}), offsets=({}, {}), size=({}, {}), scale={}",
                    kind,
                    top_base_x,
                    top_base_y,
                    self.toolbar_chrome.top_offset().0,
                    self.toolbar_chrome.top_offset().1,
                    self.surface.width(),
                    self.surface.height(),
                    self.surface.scale()
                )
            });
            self.toolbar_drag
                .begin_move(kind, coord, coord_is_screen, (top_base_x, top_base_y));
        }
        self.toolbar_drag.set_item_dragging(true);
        true
    }

    /// Handle motion reported by the toolbar surface, preserving local samples
    /// while its inline preview is active.
    pub(in crate::backend::wayland) fn handle_toolbar_move(
        &mut self,
        kind: MoveDragKind,
        local_coord: (f64, f64),
    ) {
        self.handle_toolbar_move_sample(kind, MoveSample::Local(local_coord));
    }

    /// Handle motion already in screen space, such as the main overlay surface.
    pub(in crate::backend::wayland) fn handle_toolbar_move_screen(
        &mut self,
        kind: MoveDragKind,
        screen_coord: (f64, f64),
    ) {
        self.handle_toolbar_move_sample(kind, MoveSample::Screen(screen_coord));
    }

    fn handle_toolbar_move_sample(&mut self, kind: MoveDragKind, sample: MoveSample) {
        if !self.toolbar_position_drag_update_allowed(kind) {
            self.toolbar_drag.note_move(kind, sample);
            return;
        }
        if self.pointer_lock_active() {
            drag_log(|| {
                format!("skip toolbar move: pointer locked, kind={kind:?}, sample={sample:?}")
            });
            return;
        }

        let local_origin = self.local_to_screen_coords(kind, (0.0, 0.0));
        let Some(delta) = self.toolbar_drag.move_to(kind, sample, local_origin) else {
            return;
        };
        drag_log(|| {
            format!(
                "toolbar move: kind={kind:?}, sample={sample:?}, delta={delta:?}, offsets_before={:?}",
                self.toolbar_chrome.top_offset(),
            )
        });
        if delta.0 == 0.0 && delta.1 == 0.0 {
            return;
        }

        let snapshot = self
            .toolbar
            .last_snapshot()
            .cloned()
            .unwrap_or_else(|| self.toolbar_snapshot());
        match kind {
            MoveDragKind::Top => self.toolbar_chrome.add_top_offset(delta),
        }

        // Clamp and throttle both coordinate routes identically. A preview also
        // moves the suppressed layer surface so release does not replay the drag.
        self.apply_toolbar_offsets_throttled(&snapshot);
        let inline_render_active = self.inline_toolbars_render_active();
        if inline_render_active {
            self.toolbar.mark_dirty();
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
        }
        if self.protocol.layer_shell().is_none() || inline_render_active {
            self.toolbar_chrome.clear_inline_hits();
        }
    }
}
