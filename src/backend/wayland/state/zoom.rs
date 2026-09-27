use super::*;
use crate::input::{ZoomAnchor, ZoomRequest};

impl WaylandState {
    pub(in crate::backend::wayland) fn sync_input_zoom_state(&mut self) {
        self.input_state.set_zoom_status(
            self.zoom.active,
            self.zoom.locked,
            self.zoom.scale,
            self.zoom.view_offset,
        );
    }

    pub(in crate::backend::wayland) fn sync_zoom_board_mode(&mut self) {
        let board_is_transparent = self.input_state.board_is_transparent();
        if !board_is_transparent {
            if self.suppression.reason() == OverlaySuppression::Zoom {
                self.exit_overlay_suppression(OverlaySuppression::Zoom);
            }
            if self.zoom.abort_capture() {
                self.input_state.dirty_tracker.mark_full();
                self.input_state.needs_redraw = true;
            }
            if self.zoom.is_engaged() && !self.zoom.active {
                self.zoom.activate_without_capture();
                self.sync_input_zoom_state();
            }
            if self.zoom.clear_image() {
                self.input_state.dirty_tracker.mark_full();
                self.input_state.needs_redraw = true;
            }
            self.cancel_screen_modals_if_source_changed();
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
            self.cancel_screen_modals_if_source_changed();
        }
    }

    pub(in crate::backend::wayland) fn zoomed_world_coords(
        &self,
        screen_x: f64,
        screen_y: f64,
    ) -> (i32, i32) {
        self.canvas_world_coords(screen_x, screen_y)
    }

    /// Open UI takes Escape and arrows before zoom so dismissal never also
    /// exits zoom, and menu navigation never pans the canvas.
    pub(in crate::backend::wayland) fn zoom_keys_yield_to_open_menu(&self) -> bool {
        self.input_state.modal_owns_text_input() || self.input_state.toolbar_top_menu().is_open()
    }

    pub(in crate::backend::wayland) fn handle_zoom_action(&mut self, request: ZoomRequest) {
        let (sx, sy) = self.zoom_anchor_point(request.anchor);
        match request.action {
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
                    self.sync_input_zoom_state();
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

    fn zoom_anchor_point(&self, anchor: ZoomAnchor) -> (f64, f64) {
        resolve_zoom_anchor(
            anchor,
            self.focus
                .pointer_focused()
                .then_some(self.pointer.position()),
            self.surface.width(),
            self.surface.height(),
        )
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

    pub(in crate::backend::wayland) fn zoom_panning_active(&self) -> bool {
        self.zoom.panning
    }

    pub(in crate::backend::wayland) fn exit_zoom(&mut self) {
        if self.zoom.is_engaged() {
            self.zoom.deactivate(&mut self.input_state);
            self.exit_overlay_suppression(OverlaySuppression::Zoom);
            self.cancel_screen_modals_if_source_changed();
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
                self.cancel_screen_modals_if_source_changed();
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
                self.sync_input_zoom_state();
            } else {
                self.zoom.request_activation();
            }
        } else if board_zoom && !self.zoom.active {
            self.zoom.activate_without_capture();
            self.sync_input_zoom_state();
        }

        let changed = self
            .zoom
            .zoom_at_screen_point(factor, screen_x, screen_y, screen_w, screen_h);
        if self.zoom.active && changed {
            self.sync_input_zoom_state();
            self.cancel_screen_modals_if_source_changed();
        }

        if self.zoom.is_engaged()
            && !board_zoom
            && let Err(err) = self.start_zoom_capture(false)
        {
            warn!("Zoom capture failed to start: {}", err);
            self.zoom.deactivate(&mut self.input_state);
            self.exit_overlay_suppression(OverlaySuppression::Zoom);
            self.cancel_screen_modals_if_source_changed();
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
        if !self.enter_overlay_suppression_with_keyboard_policy(
            OverlaySuppression::Zoom,
            zoom_suppression_keyboard_policy(use_fallback),
        ) {
            anyhow::bail!("Zoom capture requested while another overlay operation is preparing");
        }
        match self.zoom.start_capture(use_fallback, &self.tokio_handle) {
            Ok(()) => Ok(()),
            Err(err) => {
                self.exit_overlay_suppression(OverlaySuppression::Zoom);
                Err(err)
            }
        }
    }
}

fn zoom_suppression_keyboard_policy(use_fallback: bool) -> OverlaySuppressionKeyboardPolicy {
    if use_fallback {
        // Portal dialogs need the compositor to release Wayscriber's focus.
        OverlaySuppressionKeyboardPolicy::Release
    } else {
        // Native screencopy does not need focus. Keep held modifiers and key
        // repeats alive while the first transparent frame is captured.
        OverlaySuppressionKeyboardPolicy::Retain
    }
}

/// The screen point a zoom step centres on. A pointer anchor falls back to
/// the screen centre when the pointer is not on the overlay.
fn resolve_zoom_anchor(
    anchor: ZoomAnchor,
    pointer: Option<(i32, i32)>,
    width: u32,
    height: u32,
) -> (f64, f64) {
    let center = (width as f64 * 0.5, height as f64 * 0.5);

    match anchor {
        ZoomAnchor::Pointer => pointer.map_or(center, |(x, y)| (x as f64, y as f64)),
        ZoomAnchor::ScreenCenter => center,
        ZoomAnchor::At(x, y) => (x as f64, y as f64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shortcuts zoom at the pointer; the chip, toolbar, and palette sit away
    /// from what the user looks at and zoom the centre; a context menu zooms
    /// where it was opened.
    #[test]
    fn zoom_anchors_resolve_to_pointer_centre_or_menu_origin() {
        assert_eq!(
            resolve_zoom_anchor(ZoomAnchor::Pointer, Some((300, 200)), 1920, 1080),
            (300.0, 200.0)
        );
        assert_eq!(
            resolve_zoom_anchor(ZoomAnchor::Pointer, None, 1920, 1080),
            (960.0, 540.0)
        );
        assert_eq!(
            resolve_zoom_anchor(ZoomAnchor::ScreenCenter, Some((1850, 1050)), 1920, 1080),
            (960.0, 540.0)
        );
        assert_eq!(
            resolve_zoom_anchor(ZoomAnchor::At(12, 34), Some((1850, 1050)), 1920, 1080),
            (12.0, 34.0)
        );
    }

    #[test]
    fn native_zoom_retains_keyboard_but_portal_zoom_releases_it() {
        assert_eq!(
            zoom_suppression_keyboard_policy(false),
            OverlaySuppressionKeyboardPolicy::Retain
        );
        assert_eq!(
            zoom_suppression_keyboard_policy(true),
            OverlaySuppressionKeyboardPolicy::Release
        );
    }
}
