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

    /// Handle toolbar move with toolbar-surface-local coordinates.
    /// On layer-shell, toolbar-local coords stay consistent as the toolbar moves,
    /// so we use them directly for delta calculation.
    pub(in crate::backend::wayland) fn handle_toolbar_move(
        &mut self,
        kind: MoveDragKind,
        local_coord: (f64, f64),
    ) {
        if !self.toolbar_position_drag_update_allowed(kind) {
            // Consume the coordinate baseline without moving the toolbar. If
            // the exact same authority resumes this untouched preview, the
            // next accepted event applies only post-barrier movement.
            self.toolbar_drag.note_move(kind, local_coord, false);
            return;
        }
        if self.pointer_lock_active() {
            drag_log(|| {
                format!(
                    "skip handle_toolbar_move_local: pointer locked, kind={:?}, coord=({:.3}, {:.3})",
                    kind, local_coord.0, local_coord.1
                )
            });
            return;
        }
        drag_log(|| {
            format!(
                "handle_toolbar_move_local: kind={:?}, local_coord=({:.3}, {:.3}), offsets=({}, {})",
                kind,
                local_coord.0,
                local_coord.1,
                self.toolbar_chrome.top_offset().0,
                self.toolbar_chrome.top_offset().1
            )
        });
        // For layer-shell surfaces, use local coordinates directly since they're
        // consistent within the toolbar surface. Only convert to screen coords
        // when transitioning to/from main surface.
        self.handle_toolbar_move_local(kind, local_coord);
    }

    /// Handle toolbar move with toolbar-surface-local coordinates.
    fn handle_toolbar_move_local(&mut self, kind: MoveDragKind, local_coord: (f64, f64)) {
        let snapshot = self
            .toolbar
            .last_snapshot()
            .cloned()
            .unwrap_or_else(|| self.toolbar_snapshot());

        // When inline drag preview is active we keep the layer-shell toolbars
        // suppressed and only move the inline-rendered preview.
        if self.toolbar_drag.preview_active() {
            let delta = self
                .toolbar_drag
                .move_to(kind, local_coord, false)
                .unwrap_or((0.0, 0.0));
            if delta.0 == 0.0 && delta.1 == 0.0 {
                return;
            }

            match kind {
                MoveDragKind::Top => {
                    self.toolbar_chrome.add_top_offset(delta);
                }
            }

            // Clamp offsets; pointer-locked preview drags also move the suppressed
            // layer surface so release does not visibly replay the drag.
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
            return;
        }

        // Check if we need to transition coordinate systems
        let (last_coord, coord_is_screen) = self
            .toolbar_drag
            .move_sample()
            .map_or((local_coord, false), |sample| {
                (sample.coord, sample.is_screen)
            });

        // If last coord was screen-based, convert current local to screen for comparison
        let last_screen = if coord_is_screen {
            last_coord
        } else {
            self.local_to_screen_coords(kind, last_coord)
        };
        let effective_coord = self.local_to_screen_coords(kind, local_coord);

        if !coord_is_screen {
            self.toolbar_drag.note_move(kind, last_screen, true);
        }
        let delta = self
            .toolbar_drag
            .move_to(kind, effective_coord, true)
            .unwrap_or((0.0, 0.0));
        drag_log(|| {
            format!(
                "move_local delta: kind={:?}, local=({:.3}, {:.3}), effective=({:.3}, {:.3}), last_screen=({:.3}, {:.3}), delta=({:.3}, {:.3}), offsets_before=({}, {})",
                kind,
                local_coord.0,
                local_coord.1,
                effective_coord.0,
                effective_coord.1,
                last_screen.0,
                last_screen.1,
                delta.0,
                delta.1,
                self.toolbar_chrome.top_offset().0,
                self.toolbar_chrome.top_offset().1
            )
        });
        log::debug!(
            "handle_toolbar_move_local: kind={:?}, local_coord=({:.3}, {:.3}), effective_coord=({:.3}, {:.3}), last_coord=({:.3}, {:.3}), delta=({:.3}, {:.3}), offsets=({}, {})",
            kind,
            local_coord.0,
            local_coord.1,
            effective_coord.0,
            effective_coord.1,
            last_screen.0,
            last_screen.1,
            delta.0,
            delta.1,
            self.toolbar_chrome.top_offset().0,
            self.toolbar_chrome.top_offset().1
        );
        if delta.0 == 0.0 && delta.1 == 0.0 {
            return;
        }

        match kind {
            MoveDragKind::Top => {
                self.toolbar_chrome.add_top_offset(delta);
            }
        }
        drag_log(|| {
            format!(
                "move_local applied: kind={:?}, offsets_after=({}, {})",
                kind,
                self.toolbar_chrome.top_offset().0,
                self.toolbar_chrome.top_offset().1
            )
        });
        log::debug!(
            "After update offsets: top=({}, {})",
            self.toolbar_chrome.top_offset().0,
            self.toolbar_chrome.top_offset().1
        );

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

    /// Handle toolbar move with screen-relative coordinates (no conversion).
    /// Use this when coords are already in screen space (e.g., from main overlay surface).
    pub(in crate::backend::wayland) fn handle_toolbar_move_screen(
        &mut self,
        kind: MoveDragKind,
        screen_coord: (f64, f64),
    ) {
        if !self.toolbar_position_drag_update_allowed(kind) {
            self.toolbar_drag.note_move(kind, screen_coord, true);
            return;
        }
        if self.pointer_lock_active() {
            drag_log(|| {
                format!(
                    "skip handle_toolbar_move_screen: pointer locked, kind={:?}, coord=({:.3}, {:.3})",
                    kind, screen_coord.0, screen_coord.1
                )
            });
            return;
        }
        drag_log(|| {
            format!(
                "handle_toolbar_move_screen: kind={:?}, screen_coord=({:.3}, {:.3}), offsets=({}, {})",
                kind,
                screen_coord.0,
                screen_coord.1,
                self.toolbar_chrome.top_offset().0,
                self.toolbar_chrome.top_offset().1
            )
        });
        let snapshot = self
            .toolbar
            .last_snapshot()
            .cloned()
            .unwrap_or_else(|| self.toolbar_snapshot());

        // Get last coord, converting from local to screen if needed
        let last_screen_coord = match self.toolbar_drag.move_sample() {
            Some(sample) if sample.is_screen => sample.coord,
            Some(sample) => self.local_to_screen_coords(kind, sample.coord),
            None => screen_coord,
        };

        if self
            .toolbar_drag
            .move_sample()
            .is_some_and(|sample| !sample.is_screen)
        {
            self.toolbar_drag.note_move(kind, last_screen_coord, true);
        }
        let delta = self
            .toolbar_drag
            .move_to(kind, screen_coord, true)
            .unwrap_or((0.0, 0.0));
        drag_log(|| {
            format!(
                "move_screen delta: kind={:?}, screen=({:.3}, {:.3}), last_screen=({:.3}, {:.3}), delta=({:.3}, {:.3}), offsets_before=({}, {})",
                kind,
                screen_coord.0,
                screen_coord.1,
                last_screen_coord.0,
                last_screen_coord.1,
                delta.0,
                delta.1,
                self.toolbar_chrome.top_offset().0,
                self.toolbar_chrome.top_offset().1
            )
        });
        log::debug!(
            "handle_toolbar_move_screen: kind={:?}, screen_coord=({:.3}, {:.3}), last_screen_coord=({:.3}, {:.3}), delta=({:.3}, {:.3}), offsets=({}, {})",
            kind,
            screen_coord.0,
            screen_coord.1,
            last_screen_coord.0,
            last_screen_coord.1,
            delta.0,
            delta.1,
            self.toolbar_chrome.top_offset().0,
            self.toolbar_chrome.top_offset().1
        );
        if delta.0 == 0.0 && delta.1 == 0.0 {
            return;
        }
        match kind {
            MoveDragKind::Top => {
                self.toolbar_chrome.add_top_offset(delta);
            }
        }
        drag_log(|| {
            format!(
                "move_screen applied: kind={:?}, offsets_after=({}, {})",
                kind,
                self.toolbar_chrome.top_offset().0,
                self.toolbar_chrome.top_offset().1
            )
        });

        self.apply_toolbar_offsets_throttled(&snapshot);
        let inline_render_active = self.inline_toolbars_render_active();
        if inline_render_active {
            self.toolbar.mark_dirty();
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
        }
        if self.protocol.layer_shell().is_none() || inline_render_active {
            // Inline mode uses cached rects, so force a relayout.
            self.toolbar_chrome.clear_inline_hits();
        }
    }
}
