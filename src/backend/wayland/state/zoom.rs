use super::*;

impl WaylandState {
    pub(in crate::backend::wayland) fn sync_zoom_board_mode(&mut self) {
        let board_is_transparent = self.input_state.board_is_transparent();
        if !board_is_transparent {
            if self.data.overlay_suppression == OverlaySuppression::Zoom {
                self.exit_overlay_suppression(OverlaySuppression::Zoom);
            }
            if self.zoom.abort_capture() {
                self.input_state.dirty_tracker.mark_full();
                self.input_state.needs_redraw = true;
            }
            if self.zoom.is_engaged() && !self.zoom.active {
                self.zoom.activate_without_capture();
                self.input_state
                    .set_zoom_status(true, self.zoom.locked, self.zoom.scale);
            }
            if self.zoom.clear_image() {
                self.input_state.dirty_tracker.mark_full();
                self.input_state.needs_redraw = true;
            }
            return;
        }

        if self.zoom.is_engaged()
            && self.zoom.image().is_none()
            && !self.zoom.is_in_progress()
            && let Err(err) = self.start_zoom_capture(false)
        {
            warn!("Zoom capture failed to start: {}", err);
            self.zoom.deactivate(&mut self.input_state);
            self.exit_overlay_suppression(OverlaySuppression::Zoom);
        }
    }

    pub(in crate::backend::wayland) fn zoomed_world_coords(
        &self,
        screen_x: f64,
        screen_y: f64,
    ) -> (i32, i32) {
        if self.zoom.active {
            let (wx, wy) = self.zoom.screen_to_world(screen_x, screen_y);
            (wx.round() as i32, wy.round() as i32)
        } else {
            (screen_x.round() as i32, screen_y.round() as i32)
        }
    }

    pub(in crate::backend::wayland) fn handle_zoom_action(&mut self, action: ZoomAction) {
        let (sx, sy) = self.zoom_keyboard_anchor();
        match action {
            ZoomAction::In => {
                self.apply_zoom_factor(Self::ZOOM_STEP_KEY, sx, sy, true);
            }
            ZoomAction::Out => {
                self.apply_zoom_factor(1.0 / Self::ZOOM_STEP_KEY, sx, sy, false);
            }
            ZoomAction::Reset => {
                if self.zoom.is_engaged() {
                    self.exit_zoom();
                }
            }
            ZoomAction::ToggleLock => {
                if self.zoom.active {
                    self.zoom.locked = !self.zoom.locked;
                    if self.zoom.locked && self.zoom.panning {
                        self.zoom.stop_pan();
                    }
                    self.input_state.set_zoom_status(
                        self.zoom.active,
                        self.zoom.locked,
                        self.zoom.scale,
                    );
                }
            }
            ZoomAction::RefreshCapture => {
                if !self.input_state.board_is_transparent() {
                    info!("Zoom capture refresh ignored in board mode");
                } else if self.zoom.active
                    && let Err(err) = self.start_zoom_capture(true)
                {
                    warn!("Zoom capture refresh failed: {}", err);
                }
            }
        }
    }

    fn zoom_keyboard_anchor(&self) -> (f64, f64) {
        if self.has_pointer_focus() {
            let (sx, sy) = self.current_mouse();
            (sx as f64, sy as f64)
        } else {
            let cx = (self.surface.width() as f64) * 0.5;
            let cy = (self.surface.height() as f64) * 0.5;
            (cx, cy)
        }
    }

    pub(in crate::backend::wayland) fn handle_zoom_scroll(
        &mut self,
        zoom_in: bool,
        screen_x: f64,
        screen_y: f64,
    ) {
        let factor = if zoom_in {
            Self::ZOOM_STEP_SCROLL
        } else {
            1.0 / Self::ZOOM_STEP_SCROLL
        };
        self.apply_zoom_factor(factor, screen_x, screen_y, zoom_in);
    }

    pub(in crate::backend::wayland) fn exit_zoom(&mut self) {
        if self.zoom.is_engaged() {
            self.zoom.deactivate(&mut self.input_state);
            self.exit_overlay_suppression(OverlaySuppression::Zoom);
        }
    }

    fn apply_zoom_factor(
        &mut self,
        factor: f64,
        screen_x: f64,
        screen_y: f64,
        allow_activate: bool,
    ) {
        let screen_w = self.surface.width();
        let screen_h = self.surface.height();
        let board_zoom = !self.input_state.board_is_transparent();
        if board_zoom {
            let mut cleared = false;
            if self.zoom.abort_capture() {
                cleared = true;
                self.exit_overlay_suppression(OverlaySuppression::Zoom);
            }
            if self.zoom.clear_image() {
                cleared = true;
            }
            if cleared {
                self.input_state.dirty_tracker.mark_full();
                self.input_state.needs_redraw = true;
            }
        }

        if !self.zoom.is_engaged() {
            if !allow_activate {
                return;
            }
            self.zoom.locked = false;
            self.zoom.reset_view();
            self.input_state.close_context_menu();
            self.input_state.close_properties_panel();
            if board_zoom {
                self.zoom.activate_without_capture();
                self.input_state
                    .set_zoom_status(true, self.zoom.locked, self.zoom.scale);
            } else {
                self.zoom.request_activation();
            }
        } else if board_zoom && !self.zoom.active {
            self.zoom.activate_without_capture();
            self.input_state
                .set_zoom_status(true, self.zoom.locked, self.zoom.scale);
        }

        let changed = self
            .zoom
            .zoom_at_screen_point(factor, screen_x, screen_y, screen_w, screen_h);
        if self.zoom.active && changed {
            self.input_state
                .set_zoom_status(true, self.zoom.locked, self.zoom.scale);
        }

        if self.zoom.is_engaged()
            && !board_zoom
            && let Err(err) = self.start_zoom_capture(false)
        {
            warn!("Zoom capture failed to start: {}", err);
            self.zoom.deactivate(&mut self.input_state);
            self.exit_overlay_suppression(OverlaySuppression::Zoom);
        }
    }

    fn start_zoom_capture(&mut self, force: bool) -> Result<()> {
        if self.zoom.is_in_progress() {
            return Ok(());
        }
        if !force && self.zoom.image().is_some() {
            return Ok(());
        }
        if !self.input_state.board_is_transparent() {
            debug!("Zoom capture skipped in board mode");
            return Ok(());
        }
        if self.frozen.is_in_progress() {
            warn!("Zoom capture requested while frozen capture is in progress; ignoring");
            return Ok(());
        }
        let use_fallback = !self.zoom.manager_available();
        if use_fallback {
            warn!("Zoom: screencopy unavailable, using portal fallback");
        } else {
            log::info!("Zoom: using screencopy fast path");
        }
        self.enter_overlay_suppression(OverlaySuppression::Zoom);
        match self.zoom.start_capture(use_fallback, &self.tokio_handle) {
            Ok(()) => Ok(()),
            Err(err) => {
                self.exit_overlay_suppression(OverlaySuppression::Zoom);
                Err(err)
            }
        }
    }
}
