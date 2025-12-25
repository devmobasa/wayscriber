use super::*;

fn desired_keyboard_interactivity_for(
    layer_shell_available: bool,
    toolbar_visible: bool,
) -> KeyboardInteractivity {
    if layer_shell_available && toolbar_visible {
        KeyboardInteractivity::OnDemand
    } else {
        KeyboardInteractivity::Exclusive
    }
}

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

    /// Clear cached margins so recreated/hidden toolbars reapply offsets once.
    fn reset_toolbar_margin_cache(&mut self) {
        self.data.last_applied_top_margin = None;
        self.data.last_applied_top_margin_top = None;
        self.data.last_applied_side_margin = None;
        self.data.last_applied_side_margin_left = None;
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
        if self.overlay_suppressed() {
            return KeyboardInteractivity::None;
        }
        desired_keyboard_interactivity_for(self.layer_shell.is_some(), self.toolbar.is_visible())
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
            self.reset_toolbar_margin_cache();
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
                self.reset_toolbar_margin_cache();
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
                self.reset_toolbar_margin_cache();
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
                self.reset_toolbar_margin_cache();
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

    pub(in crate::backend::wayland) fn render_layer_toolbars_if_needed(&mut self) {
        if !self.toolbar.is_visible() || self.inline_toolbars_active() {
            return;
        }

        let snapshot = self.toolbar_snapshot();
        let changed = self.toolbar.update_snapshot(&snapshot);
        if changed {
            self.toolbar.mark_dirty();
        }
        if changed || self.toolbar.needs_render() {
            self.render_toolbars(&snapshot);
        }
    }

    pub(in crate::backend::wayland) fn inline_toolbars_active(&self) -> bool {
        self.data.inline_toolbars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desired_keyboard_interactivity_requires_layer_shell_and_visibility() {
        assert_eq!(
            desired_keyboard_interactivity_for(true, true),
            KeyboardInteractivity::OnDemand
        );
        assert_eq!(
            desired_keyboard_interactivity_for(true, false),
            KeyboardInteractivity::Exclusive
        );
        assert_eq!(
            desired_keyboard_interactivity_for(false, true),
            KeyboardInteractivity::Exclusive
        );
    }
}
