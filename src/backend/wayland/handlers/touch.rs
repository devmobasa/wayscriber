use log::debug;
use smithay_client_toolkit::seat::touch::TouchHandler;
use wayland_client::{
    Connection, QueueHandle,
    protocol::{wl_surface, wl_touch},
};

use crate::backend::wayland::state::{
    PerfInputSource, TouchTarget, WaylandState, debug_toolbar_drag_logging_enabled,
};
use crate::backend::wayland::toolbar_intent::intent_to_event;
use crate::input::MouseButton;
use crate::input::state::HelpOverlayPressSource;
use crate::ui::ZoomChipPress;

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
        if !self.active_touch.begin(id, position) {
            debug!("Ignoring secondary touch down id={}", id);
            return;
        }

        self.set_last_activation_serial(Some(serial));
        self.active_touch_surface = Some(surface.clone());
        let target = self.handle_touch_down(conn, qh, &surface, position);
        self.active_touch.set_target(target);
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
        if !self.active_touch.is_active_id(id) {
            debug!("Ignoring inactive touch up id={}", id);
            return;
        }

        let surface = self.active_touch_surface.clone();
        let position = self.active_touch.last_position();
        let target = self.active_touch.target();
        if let (Some(surface), Some(position)) = (surface.as_ref(), position) {
            self.handle_touch_up(surface, position, target);
        }
        self.clear_active_touch();
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
        if !self.active_touch.update_position(id, position) {
            return;
        }

        let Some(surface) = self.active_touch_surface.clone() else {
            return;
        };
        self.handle_touch_motion(conn, &surface, position, self.active_touch.target());
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
        if !self.active_touch.is_active() {
            self.active_touch_surface = None;
            self.input_state
                .clear_help_overlay_press_for(HelpOverlayPressSource::Touch);
            return;
        }

        let target = self.active_touch.target();
        self.set_pending_toast_press(None);
        self.set_pending_status_hud_press(false);
        self.set_pending_zoom_chip_press(ZoomChipPress::None);
        self.set_suppress_next_release(false);
        self.input_state
            .clear_help_overlay_press_for(HelpOverlayPressSource::Touch);

        if !matches!(
            target,
            TouchTarget::Overlay | TouchTarget::Toolbar | TouchTarget::InlineToolbar
        ) {
            self.clear_active_touch();
            return;
        }

        if let Some(surface) = self.active_touch_surface.take()
            && target == TouchTarget::Toolbar
        {
            self.toolbar.pointer_leave(&surface);
            self.toolbar.mark_dirty();
            self.set_pointer_over_toolbar(false);
        }
        if target == TouchTarget::InlineToolbar {
            self.inline_toolbar_leave();
        }
        self.set_toolbar_dragging(false);
        self.end_toolbar_move_drag();
        if self.board_panning_active() {
            self.stop_board_pan();
        }
        self.input_state.cancel_active_interaction();
        self.input_state.needs_redraw = true;
        self.clear_active_touch();
    }

    fn clear_active_touch(&mut self) {
        self.active_touch.clear();
        self.active_touch_surface = None;
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
        let target = self.classify_touch_surface(surface);
        let Some(screen_position) = self.touch_screen_position(surface, position, target) else {
            return TouchTarget::Other;
        };
        let screen_x = screen_position.0.round() as i32;
        let screen_y = screen_position.1.round() as i32;
        self.set_current_mouse(screen_x, screen_y);

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

        if self.input_state.tour_active {
            return TouchTarget::Other;
        }

        // Help is modal for every pointing modality. Record the same
        // screen-space target as the mouse path and swallow the touch so it
        // cannot operate the toolbar or canvas underneath.
        if self.input_state.show_help {
            self.input_state.note_help_overlay_press(
                HelpOverlayPressSource::Touch,
                screen_x,
                screen_y,
            );
            return target;
        }

        if self.input_state.command_palette_open {
            let screen_width = self.surface.width();
            let screen_height = self.surface.height();
            if self.input_state.handle_command_palette_click(
                screen_x,
                screen_y,
                screen_width,
                screen_height,
            ) {
                self.set_suppress_next_release(true);
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

        self.set_pending_toast_press(None);
        if let Some(pressed) = self.input_state.toast_press_at(screen_x, screen_y) {
            self.set_pending_toast_press(Some(pressed));
            return target;
        }

        // Interactive status HUD: press reports the hit; release activates.
        self.set_pending_status_hud_press(false);
        if self.input_state.status_hud_contains(screen_x, screen_y) {
            self.set_pending_status_hud_press(true);
            return target;
        }

        // Interactive zoom chip: any press inside the pill is swallowed and
        // recorded as `Passive` (the `NN%` readout / inter-piece gap) or
        // `Button(kind)`, so its release stays consumed either way; a `Button`
        // release activates only when it lands on the SAME button.
        self.set_pending_zoom_chip_press(ZoomChipPress::None);
        if self.input_state.zoom_chip_contains(screen_x, screen_y) {
            let pressed = self.input_state.zoom_chip_press_at(screen_x, screen_y);
            self.set_pending_zoom_chip_press(pressed);
            return target;
        }

        if self.board_pan_key_held() && self.can_start_board_pan() {
            self.start_board_pan(screen_position.0, screen_position.1);
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
        self.set_current_mouse(screen_x, screen_y);

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

        if self.board_panning_active() {
            let (dx, dy) = self.update_board_pan_position(screen_position.0, screen_position.1);
            let _ = self.pan_board_by_screen_delta(dx, dy);
            let (wx, wy) = self.zoomed_world_coords(screen_position.0, screen_position.1);
            self.input_state
                .update_pointer_positions(screen_x, screen_y, wx, wy);
            return;
        }

        if self.input_state.show_help
            || self.input_state.command_palette_open
            || self.input_state.tour_active
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
        if self.take_suppress_next_release() {
            self.set_pending_toast_press(None);
            self.set_pending_status_hud_press(false);
            self.set_pending_zoom_chip_press(ZoomChipPress::None);
            return;
        }

        // Mirror the mouse help release contract before toolbar/canvas
        // routing. Touch presses can originate on either the overlay or a GTK
        // toolbar surface, so resolve through the same screen conversion used
        // on touch-down.
        if self.input_state.show_help {
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
                self.set_pending_toast_press(None);
                self.set_pending_status_hud_press(false);
                self.set_pending_zoom_chip_press(ZoomChipPress::None);
                return;
            }
            // Help opened after this touch began. Finish the pre-help gesture
            // through the normal release path below.
        }

        if self.input_state.command_palette_open || self.input_state.tour_active {
            self.set_pending_toast_press(None);
            self.set_pending_status_hud_press(false);
            self.set_pending_zoom_chip_press(ZoomChipPress::None);
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

        if let Some(pressed) = self.take_pending_toast_press() {
            let (hit, action) = self
                .input_state
                .resolve_toast_release(pressed, screen_x, screen_y);
            if hit && let Some(action) = action {
                self.dispatch_input_action(action);
            }
            return;
        }

        if self.take_pending_status_hud_press() {
            let (hit, action) = self.input_state.check_status_hud_click(screen_x, screen_y);
            if hit && let Some(action) = action {
                self.dispatch_input_action(action);
            }
            return;
        }

        let pressed = self.take_pending_zoom_chip_press();
        if pressed.is_pending() {
            // Any pending chip press (`Passive` or `Button`) consumes its
            // release; only a `Button` release on the SAME button dispatches.
            if let ZoomChipPress::Button(kind) = pressed {
                let (_, action) = self
                    .input_state
                    .check_zoom_chip_click(kind, screen_x, screen_y);
                if let Some(action) = action {
                    self.dispatch_input_action(action);
                }
            }
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
            self.set_toolbar_dragging(false);
            self.end_toolbar_move_drag();
            return;
        }

        if target != TouchTarget::Overlay {
            return;
        }

        if self.board_panning_active() {
            self.stop_board_pan();
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
