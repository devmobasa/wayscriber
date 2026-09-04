use log::debug;
use smithay_client_toolkit::seat::touch::TouchHandler;
use wayland_client::{
    Connection, QueueHandle,
    protocol::{wl_surface, wl_touch},
};

use crate::backend::wayland::state::{
    PerfInputSource, RegionReviewPress, TouchTarget, WaylandState,
    debug_toolbar_drag_logging_enabled,
};
use crate::backend::wayland::toolbar_intent::intent_to_event;
use crate::input::MouseButton;
use crate::input::state::{HelpOverlayPressSource, RegionInputSource};

impl TouchHandler for WaylandState {
    fn down(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        _touch: &wl_touch::WlTouch,
        serial: u32,
        _time: u32,
        surface: wl_surface::WlSurface,
        id: i32,
        position: (f64, f64),
    ) {
        if !self
            .pointer
            .begin_touch(id, position, surface.clone(), TouchTarget::None)
        {
            debug!("Ignoring secondary touch down id={id}");
            return;
        }

        self.focus.note_activation_serial(serial);
        let target = self.handle_touch_down(conn, qh, &surface, position);
        self.pointer.set_touch_target(target);
    }

    fn up(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _touch: &wl_touch::WlTouch,
        _serial: u32,
        _time: u32,
        id: i32,
    ) {
        let Some(end) = self.pointer.end_touch(id) else {
            debug!("Ignoring inactive touch up id={id}");
            return;
        };

        self.handle_touch_up(&end.surface, end.position, end.target);
    }

    fn motion(
        &mut self,
        conn: &Connection,
        _qh: &QueueHandle<Self>,
        _touch: &wl_touch::WlTouch,
        _time: u32,
        id: i32,
        position: (f64, f64),
    ) {
        let Some((surface, target)) = self.pointer.touch_position(id, position) else {
            return;
        };
        self.handle_touch_motion(conn, &surface, position, target);
    }

    fn shape(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _touch: &wl_touch::WlTouch,
        _id: i32,
        _major: f64,
        _minor: f64,
    ) {
    }

    fn orientation(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _touch: &wl_touch::WlTouch,
        _id: i32,
        _orientation: f64,
    ) {
    }

    fn cancel(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _touch: &wl_touch::WlTouch) {
        self.cancel_active_touch_sequence();
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn cancel_active_touch_sequence(&mut self) {
        let Some(end) = self.pointer.cancel_touch() else {
            self.input_state
                .clear_help_overlay_press_for(HelpOverlayPressSource::Touch);
            return;
        };

        let target = end.target;
        // A cancelled sequence never produces a release, so a region drag it
        // started has to end here — otherwise the selector, and any freeze it
        // owns, would outlive the touch that opened it. A region another device
        // is dragging is untouched.
        self.cancel_region_selection_from(RegionInputSource::Touch);
        self.pointer.clear_chrome_press();
        self.input_state
            .clear_help_overlay_press_for(HelpOverlayPressSource::Touch);

        if !matches!(
            target,
            TouchTarget::Overlay | TouchTarget::Toolbar | TouchTarget::InlineToolbar
        ) {
            return;
        }

        if target == TouchTarget::Toolbar {
            self.toolbar.pointer_leave(&end.surface);
            self.toolbar.mark_dirty();
            self.set_pointer_over_toolbar(false);
        }
        if target == TouchTarget::InlineToolbar {
            self.inline_toolbar_leave();
        }
        self.finish_toolbar_item_drag(false);
        self.set_toolbar_dragging(false);
        self.cancel_toolbar_move_drag();
        if self.pointer.board_pan_active() {
            self.pointer.stop_board_pan();
        }
        self.input_state.cancel_active_interaction();
        self.input_state.needs_redraw = true;
    }

    fn classify_touch_surface(&self, surface: &wl_surface::WlSurface) -> TouchTarget {
        if self.toolbar.is_toolbar_surface(surface) {
            TouchTarget::Toolbar
        } else if self
            .surface
            .wl_surface()
            .is_some_and(|overlay| overlay == surface)
        {
            TouchTarget::Overlay
        } else {
            TouchTarget::Other
        }
    }

    fn touch_screen_position(
        &self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
        target: TouchTarget,
    ) -> Option<(f64, f64)> {
        match target {
            TouchTarget::Overlay | TouchTarget::InlineToolbar => Some(position),
            TouchTarget::Toolbar => self.toolbar_surface_screen_coords(surface, position),
            TouchTarget::None | TouchTarget::Other => None,
        }
    }

    fn handle_touch_down(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
    ) -> TouchTarget {
        // A finished scan card is transient chrome: the next interaction of any
        // kind takes it away rather than making the user wait it out.
        self.input_state.dismiss_ocr_scan_result();
        let target = self.classify_touch_surface(surface);
        let Some(screen_position) = self.touch_screen_position(surface, position, target) else {
            return TouchTarget::Other;
        };
        let screen_x = screen_position.0.round() as i32;
        let screen_y = screen_position.1.round() as i32;
        self.pointer.set_position((screen_x, screen_y));

        if !self.input_state.help_overlay.is_visible() {
            // A new touch supersedes any consume-only help ownership left by
            // a sequence whose release/cancel was not delivered.
            self.input_state
                .clear_help_overlay_press_for(HelpOverlayPressSource::Touch);
        }

        if self.input_state.region_is_active() {
            let inline_active = self.inline_toolbars_active() && self.toolbar.is_visible();
            let inline_hit = target == TouchTarget::Overlay
                && inline_active
                && self.inline_toolbar_motion(screen_position);
            if target == TouchTarget::Toolbar || inline_hit {
                self.cancel_region_for_toolbar_interaction();
            } else if target == TouchTarget::Overlay {
                match self.consume_region_review_press(RegionInputSource::Touch, screen_position) {
                    RegionReviewPress::NotReview | RegionReviewPress::Fallthrough => {
                        self.begin_region_selection(
                            RegionInputSource::Touch,
                            screen_position.0,
                            screen_position.1,
                        );
                    }
                    RegionReviewPress::Consumed { suppress_release } => {
                        if suppress_release {
                            self.pointer.suppress_release(RegionInputSource::Touch);
                        }
                    }
                }
                // Unlike the one-shot eyedropper sample, an OCR region is a
                // drag: report the real target so motion and release still
                // resolve to screen coordinates and reach the selector.
                return TouchTarget::Overlay;
            }
        }

        if self.input_state.eyedropper_is_active() {
            let inline_active = self.inline_toolbars_active() && self.toolbar.is_visible();
            let inline_hit = target == TouchTarget::Overlay
                && inline_active
                && self.inline_toolbar_motion(screen_position);
            if target == TouchTarget::Toolbar || inline_hit {
                self.cancel_eyedropper();
            } else if target == TouchTarget::Overlay {
                self.sample_eyedropper(screen_position.0, screen_position.1);
                return TouchTarget::Other;
            }
        }

        if self.input_state.tour.is_active() {
            return TouchTarget::Other;
        }

        // Help is modal for every pointing modality. Record the same
        // screen-space target as the mouse path and swallow the touch so it
        // cannot operate the toolbar or canvas underneath.
        if self.input_state.help_overlay.is_visible() {
            self.input_state.note_help_overlay_press(
                HelpOverlayPressSource::Touch,
                screen_x,
                screen_y,
            );
            return target;
        }

        if self.input_state.command_palette.open {
            let screen_width = self.surface.width();
            let screen_height = self.surface.height();
            if self.input_state.handle_command_palette_click(
                screen_x,
                screen_y,
                screen_width,
                screen_height,
            ) {
                self.pointer.suppress_release(RegionInputSource::Touch);
            }
            return TouchTarget::Other;
        }

        let inline_active = self.inline_toolbars_active() && self.toolbar.is_visible();
        if target == TouchTarget::Overlay
            && inline_active
            && self.inline_toolbar_press(screen_position, Some(conn), Some(qh))
        {
            return TouchTarget::InlineToolbar;
        }

        if target == TouchTarget::Toolbar {
            self.set_pointer_over_toolbar(true);
            if let Some((intent, drag)) = self.toolbar.pointer_press(surface, position) {
                let toolbar_event = intent_to_event(intent, self.toolbar.last_snapshot());
                self.set_toolbar_dragging(drag);
                self.handle_toolbar_event(toolbar_event, Some(conn), Some(qh));
                self.toolbar.mark_dirty();
                self.input_state.needs_redraw = true;
                self.refresh_keyboard_interactivity();
            }
            return TouchTarget::Toolbar;
        }

        self.set_pointer_over_toolbar(false);
        if target != TouchTarget::Overlay {
            return target;
        }

        // Canvas click-away: a tap on the canvas with a top popover open
        // (Canvas/Session/Settings) dismisses it and swallows the tap, exactly
        // like the mouse and tablet pen-down paths — otherwise the tap would
        // start a stray stroke instead of closing the popover.
        if self.dismiss_top_toolbar_menus() {
            self.input_state.needs_redraw = true;
            return target;
        }

        if self.press_overlay_chrome(screen_x, screen_y) {
            return target;
        }

        if self.pointer.board_pan_key_held() && self.can_start_board_pan() {
            self.pointer
                .start_board_pan((screen_position.0, screen_position.1));
            self.input_state.needs_redraw = true;
            return target;
        }

        let (wx, wy) = self.zoomed_world_coords(screen_position.0, screen_position.1);
        self.input_state
            .on_mouse_press_with_canvas(MouseButton::Left, screen_x, screen_y, wx, wy);
        self.input_state.needs_redraw = true;
        target
    }

    fn handle_touch_motion(
        &mut self,
        conn: &Connection,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
        target: TouchTarget,
    ) {
        let Some(screen_position) = self.touch_screen_position(surface, position, target) else {
            return;
        };
        let screen_x = screen_position.0.round() as i32;
        let screen_y = screen_position.1.round() as i32;
        self.pointer.set_position((screen_x, screen_y));

        if self.input_state.region_is_active() {
            if target == TouchTarget::Overlay {
                self.update_region_selection(
                    RegionInputSource::Touch,
                    screen_position.0,
                    screen_position.1,
                );
            }
            return;
        }

        if self.input_state.eyedropper_is_active() {
            if target == TouchTarget::Overlay {
                self.update_eyedropper_hover(screen_position.0, screen_position.1);
            }
            return;
        }

        if self.is_move_dragging()
            && let Some(kind) = self.active_move_drag_kind()
        {
            if target == TouchTarget::Toolbar {
                self.handle_toolbar_move(kind, position);
            } else {
                self.handle_toolbar_move_screen(kind, screen_position);
            }
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            return;
        }

        if target == TouchTarget::InlineToolbar {
            let _ = self.inline_toolbar_motion(screen_position);
            return;
        }

        if target == TouchTarget::Toolbar {
            self.set_pointer_over_toolbar(true);
            let (wx, wy) = self.zoomed_world_coords(screen_position.0, screen_position.1);
            self.input_state
                .update_pointer_positions(screen_x, screen_y, wx, wy);
            let evt = self.toolbar.pointer_motion(surface, position);
            if self.toolbar_dragging() {
                let intent = evt.or_else(|| self.move_drag_intent(position.0, position.1));
                if let Some(intent) = intent {
                    let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                    self.handle_toolbar_event(evt, Some(conn), None);
                }
            } else {
                self.toolbar.mark_dirty();
            }
            self.input_state.needs_redraw = true;
            self.refresh_keyboard_interactivity();
            return;
        }

        if target != TouchTarget::Overlay {
            return;
        }

        if self.pointer.board_pan_active() {
            let (dx, dy) = self
                .pointer
                .advance_board_pan((screen_position.0, screen_position.1));
            let _ = self.pan_board_by_screen_delta(dx, dy);
            let (wx, wy) = self.zoomed_world_coords(screen_position.0, screen_position.1);
            self.input_state
                .update_pointer_positions(screen_x, screen_y, wx, wy);
            return;
        }

        if self.input_state.help_overlay.is_visible()
            || self.input_state.command_palette.open
            || self.input_state.tour.is_active()
        {
            return;
        }

        let (wx, wy) = self.zoomed_world_coords(screen_position.0, screen_position.1);
        self.input_state
            .update_pointer_positions(screen_x, screen_y, wx, wy);
        self.input_state
            .on_mouse_motion_with_canvas(screen_x, screen_y, wx, wy);
        self.record_perf_input_sample(PerfInputSource::Touch, screen_x, screen_y, wx, wy, false);
    }

    fn handle_touch_up(
        &mut self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
        target: TouchTarget,
    ) {
        if self.input_state.region_is_active() {
            if self
                .pointer
                .take_suppressed_release(RegionInputSource::Touch)
            {
                return;
            }
            if let Some((x, y)) = self.touch_screen_position(surface, position, target) {
                self.finish_region_selection(RegionInputSource::Touch, x, y);
            } else {
                self.cancel_region_selection_from(RegionInputSource::Touch);
            }
            return;
        }

        if self
            .pointer
            .take_suppressed_release(RegionInputSource::Touch)
        {
            self.pointer.clear_chrome_press();
            return;
        }

        // Resolve help ownership even after help closes, before routing into a
        // popup that may have opened in the meantime.
        let help_owned_release = match self.touch_screen_position(surface, position, target) {
            Some((screen_x, screen_y)) => self.handle_help_overlay_release(
                HelpOverlayPressSource::Touch,
                screen_x.round() as i32,
                screen_y.round() as i32,
            ),
            None => self
                .input_state
                .clear_help_overlay_press_for(HelpOverlayPressSource::Touch),
        };
        if help_owned_release {
            self.pointer.clear_chrome_press();
            return;
        }

        if self.input_state.command_palette.open || self.input_state.tour.is_active() {
            self.pointer.clear_chrome_press();
            self.cancel_active_touch_sequence();
            return;
        }

        if target == TouchTarget::Toolbar {
            if debug_toolbar_drag_logging_enabled() {
                debug!(
                    "touch release: target={:?}, drag_active={}, toolbar_dragging={}",
                    target,
                    self.is_move_dragging(),
                    self.toolbar_dragging()
                );
            }
            self.toolbar.pointer_leave(surface);
            self.set_pointer_over_toolbar(false);
            self.finish_toolbar_item_drag(true);
            self.set_toolbar_dragging(false);
            self.end_toolbar_move_drag();
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            return;
        }

        let Some(screen_position) = self.touch_screen_position(surface, position, target) else {
            return;
        };
        let screen_x = screen_position.0.round() as i32;
        let screen_y = screen_position.1.round() as i32;

        if self.release_overlay_chrome(screen_x, screen_y) {
            return;
        }

        if debug_toolbar_drag_logging_enabled() {
            debug!(
                "touch release: target={:?}, drag_active={}, toolbar_dragging={}",
                target,
                self.is_move_dragging(),
                self.toolbar_dragging()
            );
        }

        if target == TouchTarget::InlineToolbar {
            let _ = self.inline_toolbar_release(screen_position);
            return;
        }

        if self.is_move_dragging() {
            self.finish_toolbar_item_drag(true);
            self.set_toolbar_dragging(false);
            self.end_toolbar_move_drag();
            return;
        }

        if target != TouchTarget::Overlay {
            return;
        }

        if self.pointer.board_pan_active() {
            self.pointer.stop_board_pan();
            self.input_state.needs_redraw = true;
            return;
        }

        let (wx, wy) = self.zoomed_world_coords(screen_position.0, screen_position.1);
        self.input_state.on_mouse_release_with_canvas(
            MouseButton::Left,
            screen_x,
            screen_y,
            wx,
            wy,
        );
        self.input_state.needs_redraw = true;
    }
}
