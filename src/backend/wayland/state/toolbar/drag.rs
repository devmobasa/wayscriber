use super::*;

impl WaylandState {
    /// Base X position for the top toolbar when laid out inline.
    /// When a drag is in progress we freeze this base to avoid shifting the top bar while moving the side bar.
    pub(super) fn inline_top_base_x(&self, snapshot: &ToolbarSnapshot) -> f64 {
        if self.is_move_dragging()
            && let Some(x) = self.data.drag_top_base_x
        {
            return x;
        }
        let side_visible = self.toolbar.is_side_visible();
        let side_size = side_size(snapshot);
        let top_size = top_size(snapshot);
        let side_start_y = Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset;
        let top_bottom_y = Self::INLINE_TOP_Y + self.data.toolbar_top_offset_y + top_size.1 as f64;
        let base = Self::INLINE_SIDE_X + self.data.toolbar_side_offset_x;
        // When dragging the side toolbar, don't push the top bar; keep its base stable so it
        // doesn't shift while moving the side bar.
        let allow_push = self.active_move_drag_kind() != Some(MoveDragKind::Side);
        geometry::compute_inline_top_base_x(
            base,
            side_visible,
            side_size.0 as f64,
            side_start_y,
            top_bottom_y,
            Self::INLINE_TOP_PUSH,
            allow_push,
        )
    }

    pub(super) fn inline_top_base_y(&self) -> f64 {
        if self.is_move_dragging()
            && let Some(y) = self.data.drag_top_base_y
        {
            return y;
        }
        Self::INLINE_TOP_Y
    }

    /// Convert a toolbar-local coordinate into a screen-relative coordinate so that
    /// dragging continues to work even after the surface has moved.
    fn local_to_screen_coords(&self, kind: MoveDragKind, local_coord: (f64, f64)) -> (f64, f64) {
        match kind {
            MoveDragKind::Top => (
                self.inline_top_base_x(&self.toolbar_snapshot())
                    + self.data.toolbar_top_offset
                    + local_coord.0,
                self.inline_top_base_y() + self.data.toolbar_top_offset_y + local_coord.1,
            ),
            MoveDragKind::Side => (
                Self::SIDE_BASE_MARGIN_LEFT + self.data.toolbar_side_offset_x + local_coord.0,
                Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset + local_coord.1,
            ),
        }
    }

    pub(super) fn clamp_toolbar_offsets(&mut self, snapshot: &ToolbarSnapshot) -> bool {
        let width = self.surface.width() as f64;
        let height = self.surface.height() as f64;
        if width == 0.0 || height == 0.0 {
            drag_log(format!(
                "skip clamp: surface not configured (width={}, height={})",
                width, height
            ));
            return false;
        }
        let (top_w, top_h) = top_size(snapshot);
        let (side_w, side_h) = side_size(snapshot);
        let top_base_x = self.inline_top_base_x(snapshot);
        let top_base_y = self.inline_top_base_y();

        let before_top = (self.data.toolbar_top_offset, self.data.toolbar_top_offset_y);
        let before_side = (
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset,
        );
        let input = geometry::ToolbarClampInput {
            width,
            height,
            top_size: (top_w, top_h),
            side_size: (side_w, side_h),
            top_base_x,
            top_base_y,
            top_margin_right: Self::TOP_MARGIN_RIGHT,
            top_margin_bottom: Self::TOP_MARGIN_BOTTOM,
            side_base_margin_left: Self::SIDE_BASE_MARGIN_LEFT,
            side_base_margin_top: Self::SIDE_BASE_MARGIN_TOP,
            side_margin_right: Self::SIDE_MARGIN_RIGHT,
            side_margin_bottom: Self::SIDE_MARGIN_BOTTOM,
        };
        let offsets = geometry::ToolbarOffsets {
            top_x: self.data.toolbar_top_offset,
            top_y: self.data.toolbar_top_offset_y,
            side_x: self.data.toolbar_side_offset_x,
            side_y: self.data.toolbar_side_offset,
        };
        let (clamped, bounds) = geometry::clamp_toolbar_offsets(offsets, input);
        self.data.toolbar_top_offset = clamped.top_x;
        self.data.toolbar_top_offset_y = clamped.top_y;
        self.data.toolbar_side_offset_x = clamped.side_x;
        self.data.toolbar_side_offset = clamped.side_y;
        drag_log(format!(
            "clamp offsets: before=({:.3}, {:.3})/({:.3}, {:.3}), after=({:.3}, {:.3})/({:.3}, {:.3}), max=({:.3}, {:.3})/({:.3}, {:.3}), size=({}, {}), top_base_x={:.3}, top_base_y={:.3}",
            before_top.0,
            before_top.1,
            before_side.0,
            before_side.1,
            self.data.toolbar_top_offset,
            self.data.toolbar_top_offset_y,
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset,
            bounds.max_top_x,
            bounds.max_top_y,
            bounds.max_side_x,
            bounds.max_side_y,
            width,
            height,
            top_base_x,
            top_base_y
        ));
        true
    }

    pub(super) fn begin_toolbar_move_drag(&mut self, kind: MoveDragKind, local_coord: (f64, f64)) {
        if self.data.toolbar_move_drag.is_none() {
            log::debug!(
                "Begin toolbar move drag: kind={:?}, local_coord=({:.3}, {:.3})",
                kind,
                local_coord.0,
                local_coord.1
            );
            // Store as local coords since the initial press is on the toolbar surface
            self.data.toolbar_move_drag = Some(MoveDrag {
                kind,
                last_coord: local_coord,
                coord_is_screen: false,
            });
            // Freeze base positions so the other toolbar doesn't push while dragging.
            let snapshot = self.toolbar_snapshot();
            self.data.drag_top_base_x = Some(self.inline_top_base_x(&snapshot));
            self.data.drag_top_base_y = Some(self.inline_top_base_y());
        }
        self.data.active_drag_kind = Some(kind);
        self.set_toolbar_dragging(true);
    }

    pub(super) fn apply_toolbar_offsets(&mut self, snapshot: &ToolbarSnapshot) {
        if self.surface.width() == 0 || self.surface.height() == 0 {
            drag_log(format!(
                "skip apply_toolbar_offsets: surface not configured (width={}, height={})",
                self.surface.width(),
                self.surface.height()
            ));
            return;
        }
        let _ = self.clamp_toolbar_offsets(snapshot);
        if self.layer_shell.is_some() {
            let top_base_x = self.inline_top_base_x(snapshot);
            let (top_margin_left, top_margin_top, side_margin_top, side_margin_left) =
                geometry::compute_layer_margins(
                    top_base_x,
                    Self::TOP_BASE_MARGIN_TOP,
                    Self::SIDE_BASE_MARGIN_LEFT,
                    Self::SIDE_BASE_MARGIN_TOP,
                    geometry::ToolbarOffsets {
                        top_x: self.data.toolbar_top_offset,
                        top_y: self.data.toolbar_top_offset_y,
                        side_x: self.data.toolbar_side_offset_x,
                        side_y: self.data.toolbar_side_offset,
                    },
                );
            drag_log(format!(
                "apply_toolbar_offsets: top_margin_left={}, top_margin_top={}, side_margin_top={}, side_margin_left={}, offsets=({}, {})/({}, {}), scale={}, top_base_x={}",
                top_margin_left,
                top_margin_top,
                side_margin_top,
                side_margin_left,
                self.data.toolbar_top_offset,
                self.data.toolbar_top_offset_y,
                self.data.toolbar_side_offset_x,
                self.data.toolbar_side_offset,
                self.surface.scale(),
                top_base_x
            ));
            if debug_toolbar_drag_logging_enabled() {
                debug!(
                    "apply_toolbar_offsets: top_margin_left={} (last={:?}), top_margin_top={} (last={:?}), side_margin_top={} (last={:?}), side_margin_left={} (last={:?}), offsets=({}, {})/({}, {}), top_base_x={}",
                    top_margin_left,
                    self.data.last_applied_top_margin,
                    top_margin_top,
                    self.data.last_applied_top_margin_top,
                    side_margin_top,
                    self.data.last_applied_side_margin,
                    side_margin_left,
                    self.data.last_applied_side_margin_left,
                    self.data.toolbar_top_offset,
                    self.data.toolbar_top_offset_y,
                    self.data.toolbar_side_offset_x,
                    self.data.toolbar_side_offset,
                    top_base_x
                );
            }
            let margins_changed =
                self.data.last_applied_top_margin != Some(top_margin_left)
                    || self.data.last_applied_top_margin_top != Some(top_margin_top)
                    || self.data.last_applied_side_margin != Some(side_margin_top)
                    || self.data.last_applied_side_margin_left != Some(side_margin_left);
            if !margins_changed {
                return;
            }
            self.data.last_applied_top_margin = Some(top_margin_left);
            self.data.last_applied_side_margin = Some(side_margin_top);
            self.data.last_applied_top_margin_top = Some(top_margin_top);
            self.data.last_applied_side_margin_left = Some(side_margin_left);
            self.toolbar
                .set_top_margins(top_margin_top, top_margin_left);
            self.toolbar
                .set_side_margins(side_margin_top, side_margin_left);
            self.toolbar.mark_dirty();
        }
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
        self.apply_toolbar_offsets(&snapshot);
        // Force commits so compositors apply new margins immediately.
        if let Some(layer) = self.toolbar.top_layer_surface() {
            layer.wl_surface().commit();
        }
        if let Some(layer) = self.toolbar.side_layer_surface() {
            layer.wl_surface().commit();
        }
        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;
        self.clamp_toolbar_offsets(&snapshot);

        if self.layer_shell.is_none() || self.inline_toolbars_active() {
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
        self.apply_toolbar_offsets(&snapshot);
        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;

        // Ensure we don't drift off-screen.
        self.clamp_toolbar_offsets(&snapshot);

        if self.layer_shell.is_none() || self.inline_toolbars_active() {
            // Inline mode uses cached rects, so force a relayout.
            self.clear_inline_toolbar_hits();
        }
    }

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

        self.clamp_toolbar_offsets(&snapshot);
        self.apply_toolbar_offsets(&snapshot);
        // Commit both toolbar surfaces immediately to force the compositor to apply margins.
        if let Some(layer) = self.toolbar.top_layer_surface() {
            layer.wl_surface().commit();
        }
        if let Some(layer) = self.toolbar.side_layer_surface() {
            layer.wl_surface().commit();
        }

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

        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;
    }

    pub(in crate::backend::wayland) fn end_toolbar_move_drag(&mut self) {
        if self.data.toolbar_move_drag.is_some() {
            self.data.toolbar_move_drag = None;
            self.set_toolbar_dragging(false);
            self.set_pointer_over_toolbar(false);
            self.data.active_drag_kind = None;
            self.data.drag_top_base_x = None;
            self.data.drag_top_base_y = None;
            self.save_toolbar_pin_config();
            self.unlock_pointer();
        }
    }
}
