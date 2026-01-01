use super::*;

impl WaylandState {
    pub(in crate::backend::wayland::state::toolbar) fn begin_toolbar_move_drag(
        &mut self,
        kind: MoveDragKind,
        coord: (f64, f64),
        coord_is_screen: bool,
    ) {
        if self.data.toolbar_move_drag.is_none() {
            log::debug!(
                "Begin toolbar move drag: kind={:?}, coord=({:.3}, {:.3}), coord_is_screen={}",
                kind,
                coord.0,
                coord.1,
                coord_is_screen
            );
            drag_log(format!(
                "begin move drag: kind={:?}, coord=({:.3}, {:.3}), coord_is_screen={}, inline_active={}, layer_shell={}",
                kind,
                coord.0,
                coord.1,
                coord_is_screen,
                self.inline_toolbars_active(),
                self.layer_shell.is_some()
            ));
            // Store initial coord with explicit coordinate space (screen vs toolbar-local).
            self.data.toolbar_move_drag = Some(MoveDrag {
                kind,
                last_coord: coord,
                coord_is_screen,
            });
            // Freeze base positions so the other toolbar doesn't push while dragging.
            let snapshot = self.toolbar_snapshot();
            self.data.drag_top_base_x = Some(self.inline_top_base_x(&snapshot));
            self.data.drag_top_base_y = Some(self.inline_top_base_y());
        }
        self.data.active_drag_kind = Some(kind);
        self.set_toolbar_dragging(true);
    }

    /// Handle toolbar move with toolbar-surface-local coordinates.
    /// On layer-shell, toolbar-local coords stay consistent as the toolbar moves,
    /// so we use them directly for delta calculation.
    pub(in crate::backend::wayland) fn handle_toolbar_move(
        &mut self,
        kind: MoveDragKind,
        local_coord: (f64, f64),
    ) {
        if self.pointer_lock_active() {
            drag_log(format!(
                "skip handle_toolbar_move_local: pointer locked, kind={:?}, coord=({:.3}, {:.3})",
                kind, local_coord.0, local_coord.1
            ));
            return;
        }
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

        // Check if we need to transition coordinate systems
        let (last_coord, coord_is_screen) = match &self.data.toolbar_move_drag {
            Some(d) if d.kind == kind => (d.last_coord, d.coord_is_screen),
            _ => (local_coord, false), // Start fresh with local coords
        };

        // If last coord was screen-based, convert current local to screen for comparison
        let last_screen = if coord_is_screen {
            last_coord
        } else {
            self.local_to_screen_coords(kind, last_coord)
        };
        let effective_coord = self.local_to_screen_coords(kind, local_coord);

        self.data.active_drag_kind = Some(kind);

        let delta = (
            effective_coord.0 - last_screen.0,
            effective_coord.1 - last_screen.1,
        );
        log::debug!(
            "handle_toolbar_move_local: kind={:?}, local_coord=({:.3}, {:.3}), effective_coord=({:.3}, {:.3}), last_coord=({:.3}, {:.3}), delta=({:.3}, {:.3}), offsets=({}, {})/({}, {})",
            kind,
            local_coord.0,
            local_coord.1,
            effective_coord.0,
            effective_coord.1,
            last_screen.0,
            last_screen.1,
            delta.0,
            delta.1,
            self.data.toolbar_top_offset,
            self.data.toolbar_top_offset_y,
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset
        );
        if delta.0 == 0.0 && delta.1 == 0.0 {
            self.data.toolbar_move_drag = Some(MoveDrag {
                kind,
                last_coord: effective_coord,
                coord_is_screen: true,
            });
            return;
        }

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
        log::debug!(
            "After update offsets: top=({}, {}), side=({}, {})",
            self.data.toolbar_top_offset,
            self.data.toolbar_top_offset_y,
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset
        );

        self.data.toolbar_move_drag = Some(MoveDrag {
            kind,
            last_coord: effective_coord,
            coord_is_screen: true,
        });
        let _ = self.apply_toolbar_offsets(&snapshot);
        let inline_active = self.inline_toolbars_active();
        if inline_active {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
        }
        if self.layer_shell.is_none() || inline_active {
            self.clear_inline_toolbar_hits();
        }
    }

    /// Handle toolbar move with screen-relative coordinates (no conversion).
    /// Use this when coords are already in screen space (e.g., from main overlay surface).
    pub(in crate::backend::wayland) fn handle_toolbar_move_screen(
        &mut self,
        kind: MoveDragKind,
        screen_coord: (f64, f64),
    ) {
        if self.pointer_lock_active() {
            drag_log(format!(
                "skip handle_toolbar_move_screen: pointer locked, kind={:?}, coord=({:.3}, {:.3})",
                kind, screen_coord.0, screen_coord.1
            ));
            return;
        }
        let snapshot = self
            .toolbar
            .last_snapshot()
            .cloned()
            .unwrap_or_else(|| self.toolbar_snapshot());

        // Get last coord, converting from local to screen if needed
        let last_screen_coord = match self.data.toolbar_move_drag {
            Some(d) if d.kind == kind => {
                if d.coord_is_screen {
                    d.last_coord
                } else {
                    self.local_to_screen_coords(kind, d.last_coord)
                }
            }
            _ => screen_coord, // Start fresh
        };

        self.data.active_drag_kind = Some(kind);

        let delta = (
            screen_coord.0 - last_screen_coord.0,
            screen_coord.1 - last_screen_coord.1,
        );
        log::debug!(
            "handle_toolbar_move_screen: kind={:?}, screen_coord=({:.3}, {:.3}), last_screen_coord=({:.3}, {:.3}), delta=({:.3}, {:.3}), offsets=({}, {})/({}, {})",
            kind,
            screen_coord.0,
            screen_coord.1,
            last_screen_coord.0,
            last_screen_coord.1,
            delta.0,
            delta.1,
            self.data.toolbar_top_offset,
            self.data.toolbar_top_offset_y,
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset
        );
        if delta.0 == 0.0 && delta.1 == 0.0 {
            self.data.toolbar_move_drag = Some(MoveDrag {
                kind,
                last_coord: screen_coord,
                coord_is_screen: true,
            });
            return;
        }
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

        self.data.toolbar_move_drag = Some(MoveDrag {
            kind,
            last_coord: screen_coord,
            coord_is_screen: true,
        });
        let _ = self.apply_toolbar_offsets(&snapshot);
        let inline_active = self.inline_toolbars_active();
        if inline_active {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
        }
        if self.layer_shell.is_none() || inline_active {
            // Inline mode uses cached rects, so force a relayout.
            self.clear_inline_toolbar_hits();
        }
    }
}
