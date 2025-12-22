use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn pointer_over_toolbar(&self) -> bool {
        self.data.pointer_over_toolbar
    }

    pub(in crate::backend::wayland) fn set_pointer_over_toolbar(&mut self, value: bool) {
        self.data.pointer_over_toolbar = value;
    }

    pub(in crate::backend::wayland) fn toolbar_dragging(&self) -> bool {
        self.data.toolbar_dragging
    }

    pub(in crate::backend::wayland) fn set_toolbar_dragging(&mut self, value: bool) {
        self.data.toolbar_dragging = value;
    }

    pub(in crate::backend::wayland) fn toolbar_needs_recreate(&self) -> bool {
        self.data.toolbar_needs_recreate
    }

    pub(in crate::backend::wayland) fn set_toolbar_needs_recreate(&mut self, value: bool) {
        self.data.toolbar_needs_recreate = value;
    }

    pub(in crate::backend::wayland) fn toolbar_top_offset(&self) -> f64 {
        self.data.toolbar_top_offset
    }

    pub(in crate::backend::wayland) fn toolbar_top_offset_y(&self) -> f64 {
        self.data.toolbar_top_offset_y
    }

    pub(in crate::backend::wayland) fn toolbar_side_offset(&self) -> f64 {
        self.data.toolbar_side_offset
    }

    pub(in crate::backend::wayland) fn toolbar_side_offset_x(&self) -> f64 {
        self.data.toolbar_side_offset_x
    }

    pub(in crate::backend::wayland) fn pointer_lock_active(&self) -> bool {
        self.locked_pointer.is_some()
    }

    #[allow(dead_code)] // Kept for potential future pointer lock support
    pub(in crate::backend::wayland) fn lock_pointer_for_drag(
        &mut self,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
    ) {
        if self.inline_toolbars_active() || self.pointer_lock_active() {
            log::info!(
                "skip pointer lock: inline_active={}, already_locked={}",
                self.inline_toolbars_active(),
                self.pointer_lock_active()
            );
            return;
        }
        if self.pointer_constraints_state.bound_global().is_err() {
            log::info!("pointer lock unavailable: constraints global missing");
            return;
        }
        let Some(pointer) = self.current_pointer() else {
            log::info!("pointer lock unavailable: no current pointer");
            return;
        };

        match self.pointer_constraints_state.lock_pointer(
            surface,
            &pointer,
            None,
            zwp_pointer_constraints_v1::Lifetime::Oneshot,
            qh,
        ) {
            Ok(lp) => {
                self.locked_pointer = Some(lp);
                log::info!(
                    "pointer lock requested: seat={:?}, surface={}, pointer_id={}",
                    self.current_seat_id(),
                    surface_id(surface),
                    pointer.id().protocol_id()
                );
            }
            Err(err) => {
                warn!("Failed to lock pointer for toolbar drag: {}", err);
                return;
            }
        }

        match self
            .relative_pointer_state
            .get_relative_pointer(&pointer, qh)
        {
            Ok(rp) => {
                self.relative_pointer = Some(rp);
                log::info!("relative pointer bound for drag");
            }
            Err(err) => {
                warn!("Failed to obtain relative pointer for drag: {}", err);
                // Abort lock if relative pointer is unavailable; fall back to absolute path.
                self.unlock_pointer();
            }
        }
    }

    pub(in crate::backend::wayland) fn unlock_pointer(&mut self) {
        if let Some(lp) = self.locked_pointer.take() {
            lp.destroy();
        }
        if let Some(rp) = self.relative_pointer.take() {
            rp.destroy();
        }
    }

    pub(in crate::backend::wayland) fn desired_keyboard_interactivity(
        &self,
    ) -> KeyboardInteractivity {
        if self.layer_shell.is_some() && self.toolbar.is_visible() {
            KeyboardInteractivity::OnDemand
        } else {
            KeyboardInteractivity::Exclusive
        }
    }

    fn log_toolbar_layer_shell_missing_once(&mut self) {
        if self.data.toolbar_layer_shell_missing_logged {
            return;
        }

        let desktop_env = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "unknown".into());
        let session_env = std::env::var("XDG_SESSION_DESKTOP").unwrap_or_else(|_| "unknown".into());
        log::warn!(
            "Layer-shell protocol unavailable; toolbar surfaces will not appear (desktop='{}', session='{}'). Overlay may be limited to the work area on compositors like GNOME.",
            desktop_env,
            session_env
        );
        self.data.toolbar_layer_shell_missing_logged = true;
    }

    fn notify_toolbar_layer_shell_missing_once(&mut self) {
        if self.data.toolbar_layer_shell_notice_sent {
            return;
        }

        self.data.toolbar_layer_shell_notice_sent = true;
        let summary = "Toolbars unavailable on this desktop";
        let body = "This compositor does not expose the layer-shell protocol, so the toolbar surfaces cannot be created. Try a compositor with layer-shell support or an X11 session.";
        notification::send_notification_async(
            &self.tokio_handle,
            summary.to_string(),
            body.to_string(),
            Some("dialog-warning".to_string()),
        );
        log::warn!("{}", summary);
        log::warn!("{}", body);
    }

    /// Applies keyboard interactivity based on toolbar visibility.
    pub(in crate::backend::wayland) fn refresh_keyboard_interactivity(&mut self) {
        let desired = self.desired_keyboard_interactivity();
        let current = self.current_keyboard_interactivity();

        let updated = if let Some(layer) = self.surface.layer_surface_mut() {
            if current != Some(desired) {
                layer.set_keyboard_interactivity(desired);
                true
            } else {
                false
            }
        } else {
            self.set_current_keyboard_interactivity(None);
            return;
        };

        if updated {
            self.set_current_keyboard_interactivity(Some(desired));
        }
    }

    /// Syncs toolbar visibility from the input state, ensures surfaces exist, and adjusts keyboard interactivity.
    pub(in crate::backend::wayland) fn sync_toolbar_visibility(&mut self, qh: &QueueHandle<Self>) {
        // Sync individual toolbar visibility
        let top_visible = self.input_state.toolbar_top_visible();
        let side_visible = self.input_state.toolbar_side_visible();
        let inline_active = self.inline_toolbars_active();

        if top_visible != self.toolbar.is_top_visible() {
            self.toolbar.set_top_visible(top_visible);
            self.input_state.needs_redraw = true;
        }

        if side_visible != self.toolbar.is_side_visible() {
            self.toolbar.set_side_visible(side_visible);
            self.input_state.needs_redraw = true;
            drag_log(format!(
                "toolbar visibility change: side -> {}",
                side_visible
            ));
        }

        let any_visible = self.toolbar.is_visible();
        if !any_visible {
            self.set_pointer_over_toolbar(false);
            self.data.toolbar_configure_miss_count = 0;
        }

        if any_visible {
            log::debug!(
                "Toolbar visibility sync: top_visible={}, side_visible={}, layer_shell_available={}, inline_active={}, top_created={}, side_created={}, needs_recreate={}, scale={}",
                top_visible,
                side_visible,
                self.layer_shell.is_some(),
                inline_active,
                self.toolbar.top_created(),
                self.toolbar.side_created(),
                self.toolbar_needs_recreate(),
                self.surface.scale()
            );
            drag_log(format!(
                "toolbar sync: top_offset=({}, {}), side_offset=({}, {}), inline_active={}, layer_shell={}, needs_recreate={}",
                self.data.toolbar_top_offset,
                self.data.toolbar_top_offset_y,
                self.data.toolbar_side_offset,
                self.data.toolbar_side_offset_x,
                inline_active,
                self.layer_shell.is_some(),
                self.toolbar_needs_recreate()
            ));
        }

        // Warn the user when layer-shell is unavailable and we're forced to inline fallback.
        if any_visible && self.layer_shell.is_none() {
            self.log_toolbar_layer_shell_missing_once();
            self.notify_toolbar_layer_shell_missing_once();
        }

        if any_visible && inline_active {
            // If we forced inline while layer surfaces already existed, tear them down to avoid
            // focus/input conflicts on compositors that support layer-shell.
            if self.toolbar.top_created() || self.toolbar.side_created() {
                self.toolbar.destroy_all();
                self.set_toolbar_needs_recreate(true);
            }
            self.data.toolbar_configure_miss_count = 0;
        }

        if any_visible && self.layer_shell.is_some() && !inline_active {
            // Detect compositors ignoring or failing to configure toolbar layer surfaces; if they
            // never configure after repeated attempts, fall back to inline toolbars automatically.
            let (top_configured, side_configured) = self.toolbar.configured_states();
            let expected_top = self.toolbar.is_top_visible();
            let expected_side = self.toolbar.is_side_visible();
            if (expected_top && !top_configured) || (expected_side && !side_configured) {
                self.data.toolbar_configure_miss_count =
                    self.data.toolbar_configure_miss_count.saturating_add(1);
                if debug_toolbar_drag_logging_enabled()
                    && self.data.toolbar_configure_miss_count.is_multiple_of(60)
                {
                    debug!(
                        "Toolbar configure pending: count={}, expected_top={}, configured_top={}, expected_side={}, configured_side={}",
                        self.data.toolbar_configure_miss_count,
                        expected_top,
                        top_configured,
                        expected_side,
                        side_configured
                    );
                }
            } else {
                self.data.toolbar_configure_miss_count = 0;
            }

            if self.data.toolbar_configure_miss_count > Self::TOOLBAR_CONFIGURE_FAIL_THRESHOLD {
                warn!(
                    "Toolbar layer surfaces did not configure after {} frames; falling back to inline toolbars",
                    self.data.toolbar_configure_miss_count
                );
                self.toolbar.destroy_all();
                self.data.inline_toolbars = true;
                self.set_toolbar_needs_recreate(true);
                self.data.toolbar_configure_miss_count = 0;
                // Re-run visibility sync with inline mode enabled.
                self.sync_toolbar_visibility(qh);
                return;
            }

            if self.toolbar_needs_recreate() {
                self.toolbar.destroy_all();
                self.set_toolbar_needs_recreate(false);
            }
            let snapshot = self.toolbar_snapshot();
            self.apply_toolbar_offsets(&snapshot);
            if let Some(layer_shell) = self.layer_shell.as_ref() {
                let scale = self.surface.scale();
                self.toolbar.ensure_created(
                    qh,
                    &self.compositor_state,
                    layer_shell,
                    scale,
                    &snapshot,
                );
            }
        }

        if !any_visible {
            self.clear_inline_toolbar_hits();
        }

        self.refresh_keyboard_interactivity();
    }

    pub(in crate::backend::wayland) fn render_toolbars(&mut self, snapshot: &ToolbarSnapshot) {
        if !self.toolbar.is_visible() {
            return;
        }

        // No hover tracking yet; pass None. Can be updated when we record pointer positions per surface.
        self.toolbar.render(&self.shm, snapshot, None);
    }

    fn point_in_rect(&self, px: f64, py: f64, x: f64, y: f64, w: f64, h: f64) -> bool {
        px >= x && px <= x + w && py >= y && py <= y + h
    }

    pub(in crate::backend::wayland) fn inline_toolbars_active(&self) -> bool {
        self.data.inline_toolbars
    }

    /// Base X position for the top toolbar when laid out inline.
    /// When a drag is in progress we freeze this base to avoid shifting the top bar while moving the side bar.
    fn inline_top_base_x(&self, snapshot: &ToolbarSnapshot) -> f64 {
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
        let side_overlaps_top = side_visible && side_start_y < top_bottom_y;
        let base = Self::INLINE_SIDE_X + self.data.toolbar_side_offset_x;
        // When dragging the side toolbar, don't push the top bar; keep its base stable so it
        // doesn't shift while moving the side bar.
        if side_overlaps_top && self.active_move_drag_kind() != Some(MoveDragKind::Side) {
            base + side_size.0 as f64 + Self::INLINE_TOP_PUSH
        } else {
            base
        }
    }

    fn inline_top_base_y(&self) -> f64 {
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

    fn clamp_toolbar_offsets(&mut self, snapshot: &ToolbarSnapshot) -> bool {
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

        let mut max_top_x = (width - top_w as f64 - top_base_x - Self::TOP_MARGIN_RIGHT).max(0.0);
        let mut max_top_y = (height - top_h as f64 - top_base_y - Self::TOP_MARGIN_BOTTOM).max(0.0);
        let mut max_side_y =
            (height - side_h as f64 - Self::SIDE_BASE_MARGIN_TOP - Self::SIDE_MARGIN_BOTTOM)
                .max(0.0);
        let mut max_side_x =
            (width - side_w as f64 - Self::SIDE_BASE_MARGIN_LEFT - Self::SIDE_MARGIN_RIGHT)
                .max(0.0);

        // Ensure max bounds remain non-negative.
        max_top_x = max_top_x.max(0.0);
        max_top_y = max_top_y.max(0.0);
        max_side_x = max_side_x.max(0.0);
        max_side_y = max_side_y.max(0.0);

        // Allow negative offsets so toolbars can reach screen edges by cancelling base margins
        let min_top_x = -top_base_x;
        let min_top_y = -top_base_y;
        let min_side_x = -Self::SIDE_BASE_MARGIN_LEFT;
        let min_side_y = -Self::SIDE_BASE_MARGIN_TOP;

        let before_top = (self.data.toolbar_top_offset, self.data.toolbar_top_offset_y);
        let before_side = (
            self.data.toolbar_side_offset_x,
            self.data.toolbar_side_offset,
        );
        self.data.toolbar_top_offset = self.data.toolbar_top_offset.clamp(min_top_x, max_top_x);
        self.data.toolbar_top_offset_y = self.data.toolbar_top_offset_y.clamp(min_top_y, max_top_y);
        self.data.toolbar_side_offset = self.data.toolbar_side_offset.clamp(min_side_y, max_side_y);
        self.data.toolbar_side_offset_x = self
            .data
            .toolbar_side_offset_x
            .clamp(min_side_x, max_side_x);
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
            max_top_x,
            max_top_y,
            max_side_x,
            max_side_y,
            width,
            height,
            top_base_x,
            top_base_y
        ));
        true
    }

    fn begin_toolbar_move_drag(&mut self, kind: MoveDragKind, local_coord: (f64, f64)) {
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

    fn apply_toolbar_offsets(&mut self, snapshot: &ToolbarSnapshot) {
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
            let top_margin_left = (top_base_x + self.data.toolbar_top_offset).round() as i32;
            let top_margin_top =
                (Self::TOP_BASE_MARGIN_TOP + self.data.toolbar_top_offset_y).round() as i32;
            let side_margin_top =
                (Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset).round() as i32;
            let side_margin_left =
                (Self::SIDE_BASE_MARGIN_LEFT + self.data.toolbar_side_offset_x).round() as i32;
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
            self.data.last_applied_top_margin = Some(top_margin_left);
            self.data.last_applied_side_margin = Some(side_margin_top);
            self.data.last_applied_top_margin_top = Some(top_margin_top);
            self.data.last_applied_side_margin_left = Some(side_margin_left);
            self.toolbar.set_top_margin_left(top_margin_left);
            self.toolbar.set_top_margin_top(top_margin_top);
            self.toolbar.set_side_margin_top(side_margin_top);
            self.toolbar.set_side_margin_left(side_margin_left);
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

    fn clear_inline_toolbar_hits(&mut self) {
        self.data.inline_top_hits.clear();
        self.data.inline_side_hits.clear();
        self.data.inline_top_rect = None;
        self.data.inline_side_rect = None;
        self.data.inline_top_hover = None;
        self.data.inline_side_hover = None;
    }

    pub(super) fn render_inline_toolbars(
        &mut self,
        ctx: &cairo::Context,
        snapshot: &ToolbarSnapshot,
    ) {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            self.clear_inline_toolbar_hits();
            return;
        }

        self.clear_inline_toolbar_hits();
        self.clamp_toolbar_offsets(snapshot);

        let top_size = top_size(snapshot);
        let side_size = side_size(snapshot);

        // Position inline toolbars with padding and keep top bar to the right of the side bar.
        let side_offset = (
            Self::INLINE_SIDE_X + self.data.toolbar_side_offset_x,
            Self::SIDE_BASE_MARGIN_TOP + self.data.toolbar_side_offset,
        );

        let top_base_x = self.inline_top_base_x(snapshot);
        let top_offset = (
            top_base_x + self.data.toolbar_top_offset,
            self.inline_top_base_y() + self.data.toolbar_top_offset_y,
        );

        // Top toolbar
        let top_hover_local = self
            .data
            .inline_top_hover
            .map(|(x, y)| (x - top_offset.0, y - top_offset.1));
        let _ = ctx.save();
        ctx.translate(top_offset.0, top_offset.1);
        if let Err(err) = render_top_strip(
            ctx,
            top_size.0 as f64,
            top_size.1 as f64,
            snapshot,
            &mut self.data.inline_top_hits,
            top_hover_local,
        ) {
            log::warn!("Failed to render inline top toolbar: {}", err);
        }
        let _ = ctx.restore();
        for hit in &mut self.data.inline_top_hits {
            hit.rect.0 += top_offset.0;
            hit.rect.1 += top_offset.1;
        }
        self.data.inline_top_rect = Some((
            top_offset.0,
            top_offset.1,
            top_size.0 as f64,
            top_size.1 as f64,
        ));

        // Side toolbar
        let side_hover_local = self
            .data
            .inline_side_hover
            .map(|(x, y)| (x - side_offset.0, y - side_offset.1));
        let _ = ctx.save();
        ctx.translate(side_offset.0, side_offset.1);
        if let Err(err) = render_side_palette(
            ctx,
            side_size.0 as f64,
            side_size.1 as f64,
            snapshot,
            &mut self.data.inline_side_hits,
            side_hover_local,
        ) {
            log::warn!("Failed to render inline side toolbar: {}", err);
        }
        let _ = ctx.restore();
        for hit in &mut self.data.inline_side_hits {
            hit.rect.0 += side_offset.0;
            hit.rect.1 += side_offset.1;
        }
        self.data.inline_side_rect = Some((
            side_offset.0,
            side_offset.1,
            side_size.0 as f64,
            side_size.1 as f64,
        ));
    }

    fn inline_toolbar_hit_at(
        &self,
        position: (f64, f64),
    ) -> Option<(crate::backend::wayland::toolbar_intent::ToolbarIntent, bool)> {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return None;
        }
        self.data
            .inline_top_hits
            .iter()
            .chain(self.data.inline_side_hits.iter())
            .find_map(|hit| intent_for_hit(hit, position.0, position.1))
    }

    fn inline_toolbar_drag_at(
        &self,
        position: (f64, f64),
    ) -> Option<crate::backend::wayland::toolbar_intent::ToolbarIntent> {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return None;
        }
        // If we have an active move drag, generate intent directly from it
        // This allows dragging to continue even when mouse is outside the hit region
        if let Some(intent) = self.move_drag_intent(position.0, position.1) {
            return Some(intent);
        }
        self.data
            .inline_top_hits
            .iter()
            .chain(self.data.inline_side_hits.iter())
            .find_map(|hit| drag_intent_for_hit(hit, position.0, position.1))
    }

    /// Generate a drag intent from the active toolbar move drag state.
    /// This bypasses hit testing to allow dragging to continue when the mouse
    /// moves outside the original drag handle region.
    pub(in crate::backend::wayland) fn move_drag_intent(
        &self,
        x: f64,
        y: f64,
    ) -> Option<crate::backend::wayland::toolbar_intent::ToolbarIntent> {
        use crate::backend::wayland::toolbar_intent::ToolbarIntent;
        use crate::ui::toolbar::ToolbarEvent;

        match self.data.toolbar_move_drag {
            Some(MoveDrag {
                kind: MoveDragKind::Top,
                ..
            }) => Some(ToolbarIntent(ToolbarEvent::MoveTopToolbar { x, y })),
            Some(MoveDrag {
                kind: MoveDragKind::Side,
                ..
            }) => Some(ToolbarIntent(ToolbarEvent::MoveSideToolbar { x, y })),
            None => None,
        }
    }

    /// Returns true if we're currently in a toolbar move drag operation.
    pub(in crate::backend::wayland) fn is_move_dragging(&self) -> bool {
        self.data.toolbar_move_drag.is_some()
    }

    pub(in crate::backend::wayland) fn active_move_drag_kind(&self) -> Option<MoveDragKind> {
        self.data.active_drag_kind
    }

    pub(in crate::backend::wayland) fn inline_toolbar_motion(
        &mut self,
        position: (f64, f64),
    ) -> bool {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return false;
        }

        self.set_current_mouse(position.0 as i32, position.1 as i32);
        let (mx, my) = self.current_mouse();
        self.input_state.update_pointer_position(mx, my);

        let was_top_hover = self.data.inline_top_hover;
        let was_side_hover = self.data.inline_side_hover;

        self.data.inline_top_hover = None;
        self.data.inline_side_hover = None;

        let mut over_toolbar = false;

        if let Some((x, y, w, h)) = self.data.inline_top_rect
            && self.point_in_rect(position.0, position.1, x, y, w, h)
        {
            over_toolbar = true;
            self.data.inline_top_hover = Some(position);
        }

        if let Some((x, y, w, h)) = self.data.inline_side_rect
            && self.point_in_rect(position.0, position.1, x, y, w, h)
        {
            over_toolbar = true;
            self.data.inline_side_hover = Some(position);
        }

        if self.toolbar_dragging()
            && let Some(intent) = self.inline_toolbar_drag_at(position)
        {
            let evt = intent_to_event(intent, self.toolbar.last_snapshot());
            self.handle_toolbar_event(evt);
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            over_toolbar = true;
        } else if self.toolbar_dragging() {
            if let Some(kind) = self.active_move_drag_kind() {
                self.handle_toolbar_move(kind, position);
            }
            over_toolbar = true;
        }

        if was_top_hover != self.data.inline_top_hover
            || was_side_hover != self.data.inline_side_hover
        {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
        }

        if over_toolbar {
            self.set_pointer_over_toolbar(true);
        } else if !self.toolbar_dragging() {
            self.set_pointer_over_toolbar(false);
        }

        over_toolbar
    }

    pub(in crate::backend::wayland) fn inline_toolbar_press(
        &mut self,
        position: (f64, f64),
    ) -> bool {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return false;
        }
        if let Some((intent, drag)) = self.inline_toolbar_hit_at(position) {
            self.set_toolbar_dragging(drag);
            let evt = intent_to_event(intent, self.toolbar.last_snapshot());
            self.handle_toolbar_event(evt);
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            self.set_pointer_over_toolbar(true);
            return true;
        }
        false
    }

    pub(in crate::backend::wayland) fn inline_toolbar_leave(&mut self) {
        if !self.inline_toolbars_active() {
            return;
        }
        let had_hover =
            self.data.inline_top_hover.is_some() || self.data.inline_side_hover.is_some();
        self.data.inline_top_hover = None;
        self.data.inline_side_hover = None;
        self.set_pointer_over_toolbar(false);
        // Don't clear drag state if we're in a move drag - the drag continues outside
        if !self.is_move_dragging() {
            self.set_toolbar_dragging(false);
            self.end_toolbar_move_drag();
        }
        if had_hover {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
        }
    }

    pub(in crate::backend::wayland) fn inline_toolbar_release(
        &mut self,
        position: (f64, f64),
    ) -> bool {
        if !self.inline_toolbars_active() || !self.toolbar.is_visible() {
            return false;
        }
        if self.pointer_over_toolbar() || self.toolbar_dragging() {
            if self.toolbar_dragging()
                && let Some(intent) = self.inline_toolbar_drag_at(position)
            {
                let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                self.handle_toolbar_event(evt);
                self.toolbar.mark_dirty();
                self.input_state.needs_redraw = true;
            }
            self.set_toolbar_dragging(false);
            self.set_pointer_over_toolbar(false);
            self.end_toolbar_move_drag();
            return true;
        }
        false
    }

    /// Returns a snapshot of the current input state for toolbar UI consumption.
    pub(in crate::backend::wayland) fn toolbar_snapshot(&self) -> ToolbarSnapshot {
        let hints = ToolbarBindingHints::from_keybindings(&self.config.keybindings);
        ToolbarSnapshot::from_input_with_bindings(&self.input_state, hints)
    }

    /// Applies an incoming toolbar event and schedules redraws as needed.
    pub(in crate::backend::wayland) fn handle_toolbar_event(&mut self, event: ToolbarEvent) {
        match event {
            ToolbarEvent::MoveTopToolbar { x, y } => {
                self.begin_toolbar_move_drag(MoveDragKind::Top, (x, y));
                self.handle_toolbar_move(MoveDragKind::Top, (x, y));
                return;
            }
            ToolbarEvent::MoveSideToolbar { x, y } => {
                self.begin_toolbar_move_drag(MoveDragKind::Side, (x, y));
                self.handle_toolbar_move(MoveDragKind::Side, (x, y));
                return;
            }
            _ => {}
        }

        #[cfg(tablet)]
        let prev_thickness = self.input_state.current_thickness;
        #[cfg(tablet)]
        let thickness_event = matches!(
            event,
            ToolbarEvent::SetThickness(_) | ToolbarEvent::NudgeThickness(_)
        );

        // Check if this is a toolbar config event that needs saving
        let needs_config_save = matches!(
            event,
            ToolbarEvent::PinTopToolbar(_)
                | ToolbarEvent::PinSideToolbar(_)
                | ToolbarEvent::ToggleIconMode(_)
                | ToolbarEvent::ToggleMoreColors(_)
                | ToolbarEvent::ToggleActionsSection(_)
                | ToolbarEvent::ToggleDelaySliders(_)
                | ToolbarEvent::ToggleCustomSection(_)
        );

        let persist_drawing = matches!(
            event,
            ToolbarEvent::SetColor(_)
                | ToolbarEvent::SetThickness(_)
                | ToolbarEvent::SetMarkerOpacity(_)
                | ToolbarEvent::SetEraserMode(_)
                | ToolbarEvent::SetFont(_)
                | ToolbarEvent::SetFontSize(_)
                | ToolbarEvent::ToggleFill(_)
        );

        if self.input_state.apply_toolbar_event(event) {
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;

            #[cfg(tablet)]
            if thickness_event {
                self.sync_stylus_thickness_cache(prev_thickness);
                if self.stylus_tip_down {
                    self.record_stylus_peak(self.input_state.current_thickness);
                } else {
                    self.stylus_peak_thickness = None;
                }
            }

            // Save config when pin state changes
            if needs_config_save {
                self.save_toolbar_pin_config();
            }

            if persist_drawing {
                self.save_drawing_preferences();
            }
        }
        self.refresh_keyboard_interactivity();
    }

    #[cfg(tablet)]
    fn sync_stylus_thickness_cache(&mut self, prev: f64) {
        let cur = self.input_state.current_thickness;
        if (cur - prev).abs() > f64::EPSILON {
            self.stylus_base_thickness = Some(cur);
            if self.stylus_tip_down {
                self.stylus_pressure_thickness = Some(cur);
            } else {
                self.stylus_pressure_thickness = None;
            }
        }
    }

    /// Records the maximum stylus thickness seen during the current stroke.
    #[cfg(tablet)]
    pub(in crate::backend::wayland) fn record_stylus_peak(&mut self, thickness: f64) {
        self.stylus_peak_thickness = Some(
            self.stylus_peak_thickness
                .map_or(thickness, |p| p.max(thickness)),
        );
    }

    /// Saves the current toolbar configuration to disk (pinned state, icon mode, section visibility).
    fn save_toolbar_pin_config(&mut self) {
        self.config.ui.toolbar.top_pinned = self.input_state.toolbar_top_pinned;
        self.config.ui.toolbar.side_pinned = self.input_state.toolbar_side_pinned;
        self.config.ui.toolbar.use_icons = self.input_state.toolbar_use_icons;
        self.config.ui.toolbar.show_more_colors = self.input_state.show_more_colors;
        self.config.ui.toolbar.show_actions_section = self.input_state.show_actions_section;
        self.config.ui.toolbar.show_delay_sliders = self.input_state.show_delay_sliders;
        self.config.ui.toolbar.show_marker_opacity_section =
            self.input_state.show_marker_opacity_section;
        self.config.ui.toolbar.top_offset = self.data.toolbar_top_offset;
        self.config.ui.toolbar.top_offset_y = self.data.toolbar_top_offset_y;
        self.config.ui.toolbar.side_offset = self.data.toolbar_side_offset;
        self.config.ui.toolbar.side_offset_x = self.data.toolbar_side_offset_x;
        // Step controls toggle is in history config
        self.config.history.custom_section_enabled = self.input_state.custom_section_enabled;

        if let Err(err) = self.config.save() {
            log::warn!("Failed to save toolbar config: {}", err);
        } else {
            log::debug!("Saved toolbar config");
        }
    }

    fn save_drawing_preferences(&mut self) {
        self.config.drawing.default_color = ColorSpec::from(self.input_state.current_color);
        self.config.drawing.default_thickness = self.input_state.current_thickness;
        self.config.drawing.default_eraser_mode = self.input_state.eraser_mode;
        self.config.drawing.default_fill_enabled = self.input_state.fill_enabled;
        self.config.drawing.default_font_size = self.input_state.current_font_size;
        self.config.drawing.font_family = self.input_state.font_descriptor.family.clone();
        self.config.drawing.font_weight = self.input_state.font_descriptor.weight.clone();
        self.config.drawing.font_style = self.input_state.font_descriptor.style.clone();
        self.config.drawing.marker_opacity = self.input_state.marker_opacity;

        if let Err(err) = self.config.save() {
            log::warn!("Failed to persist drawing preferences: {}", err);
        }
    }
}
