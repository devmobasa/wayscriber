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

use super::route::{InputSurface, RoutedInput};

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
        let routed = self.route_input(&surface, position);
        if !self
            .pointer
            .begin_touch(id, position, surface.clone(), TouchTarget::None)
        {
            debug!("Ignoring secondary touch down id={id}");
            return;
        }

        self.focus.note_activation_serial(serial);
        let target = self.handle_touch_down(conn, qh, &surface, position, routed);
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

        let mut routed = self.route_input(&end.surface, end.position);
        if matches!(end.target, TouchTarget::None | TouchTarget::Foreign) {
            routed.screen = None;
        }
        self.handle_touch_up(&end.surface, end.position, end.target, routed);
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
        let mut routed = self.route_input(&surface, position);
        if matches!(target, TouchTarget::None | TouchTarget::Foreign) {
            routed.screen = None;
        }
        self.handle_touch_motion(conn, &surface, position, target, routed);
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
            TouchTarget::Canvas | TouchTarget::Toolbar | TouchTarget::InlineToolbar
        ) {
            return;
        }

        if target == TouchTarget::Toolbar {
            self.toolbar.pointer_leave(&end.surface);
            self.toolbar.mark_dirty();
            self.toolbar_chrome.set_pointer_over_toolbar(false);
        }
        if target == TouchTarget::InlineToolbar {
            self.inline_toolbar_leave();
        }
        self.finish_toolbar_item_drag(false);
        self.toolbar_drag.set_item_dragging(false);
        self.cancel_toolbar_move_drag();
        if self.pointer.board_pan_active() {
            self.pointer.stop_board_pan();
        }
        self.input_state.cancel_active_interaction();
        self.input_state.needs_redraw = true;
    }

    fn handle_touch_down(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
        routed: RoutedInput,
    ) -> TouchTarget {
        self.input_state.dismiss_ocr_scan_result();
        let target = match routed.surface {
            InputSurface::Canvas => TouchTarget::Canvas,
            InputSurface::Toolbar => TouchTarget::Toolbar,
            InputSurface::Foreign => TouchTarget::Foreign,
        };
        let Some((screen_x, screen_y)) = routed.screen else {
            return TouchTarget::Foreign;
        };
        let screen_position = (screen_x, screen_y);
        let screen_x = screen_x.round() as i32;
        let screen_y = screen_y.round() as i32;
        self.pointer.set_position((screen_x, screen_y));
        if !self.input_state.help_overlay.is_visible() {
            self.input_state
                .clear_help_overlay_press_for(HelpOverlayPressSource::Touch);
        }
        if let Some(target) = self.touch_down_screen_modal(target, routed, screen_position) {
            return target;
        }
        if let Some(target) = self.touch_down_modal_chrome(target, screen_x, screen_y) {
            return target;
        }
        if let Some(target) = self.touch_down_toolbar(conn, qh, surface, position, routed) {
            return target;
        }
        self.touch_down_canvas(target, screen_position, screen_x, screen_y)
    }

    fn touch_down_screen_modal(
        &mut self,
        target: TouchTarget,
        routed: RoutedInput,
        screen_position: (f64, f64),
    ) -> Option<TouchTarget> {
        if self.input_state.region_is_active() {
            let inline_hit = target == TouchTarget::Canvas
                && routed.inline_toolbars
                && self.inline_toolbar_motion(screen_position);
            if target == TouchTarget::Toolbar || inline_hit {
                self.cancel_region_for_toolbar_interaction();
            } else if target == TouchTarget::Canvas {
                match self.consume_region_review_press(RegionInputSource::Touch, screen_position) {
                    RegionReviewPress::NotReview | RegionReviewPress::Fallthrough => {
                        self.begin_region_selection(
                            RegionInputSource::Touch,
                            screen_position.0,
                            screen_position.1,
                        );
                    }
                    RegionReviewPress::Consumed { suppress_release } if suppress_release => {
                        self.pointer.suppress_release(RegionInputSource::Touch);
                    }
                    RegionReviewPress::Consumed { .. } => {}
                }
                return Some(TouchTarget::Canvas);
            }
        }
        if !self.input_state.eyedropper_is_active() {
            return None;
        }
        let inline_hit = target == TouchTarget::Canvas
            && routed.inline_toolbars
            && self.inline_toolbar_motion(screen_position);
        if target == TouchTarget::Toolbar || inline_hit {
            self.cancel_eyedropper();
            None
        } else if target == TouchTarget::Canvas {
            self.sample_eyedropper(screen_position.0, screen_position.1);
            Some(TouchTarget::Foreign)
        } else {
            None
        }
    }

    fn touch_down_modal_chrome(
        &mut self,
        target: TouchTarget,
        screen_x: i32,
        screen_y: i32,
    ) -> Option<TouchTarget> {
        if self.input_state.tour.is_active() {
            return Some(TouchTarget::Foreign);
        }
        if self.input_state.help_overlay.is_visible() {
            self.input_state.note_help_overlay_press(
                HelpOverlayPressSource::Touch,
                screen_x,
                screen_y,
            );
            return Some(target);
        }
        if !self.input_state.command_palette.open {
            return None;
        }
        if self.input_state.handle_command_palette_click(
            screen_x,
            screen_y,
            self.surface.width(),
            self.surface.height(),
        ) {
            self.pointer.suppress_release(RegionInputSource::Touch);
        }
        Some(TouchTarget::Foreign)
    }

    fn touch_down_toolbar(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
        routed: RoutedInput,
    ) -> Option<TouchTarget> {
        if routed.surface == InputSurface::Canvas
            && routed.inline_toolbars
            && self.inline_toolbar_press(routed.screen?, Some(conn), Some(qh))
        {
            return Some(TouchTarget::InlineToolbar);
        }
        if routed.surface != InputSurface::Toolbar {
            return None;
        }
        self.toolbar_chrome.set_pointer_over_toolbar(true);
        if let Some((intent, drag)) = self.toolbar.pointer_press(surface, position) {
            let toolbar_event = intent_to_event(intent, self.toolbar.last_snapshot());
            self.toolbar_drag.set_item_dragging(drag);
            self.handle_toolbar_event(toolbar_event, Some(conn), Some(qh));
            self.toolbar.mark_dirty();
            self.input_state.needs_redraw = true;
            self.refresh_keyboard_interactivity();
        }
        Some(TouchTarget::Toolbar)
    }

    fn touch_down_canvas(
        &mut self,
        target: TouchTarget,
        screen_position: (f64, f64),
        screen_x: i32,
        screen_y: i32,
    ) -> TouchTarget {
        self.toolbar_chrome.set_pointer_over_toolbar(false);
        if target != TouchTarget::Canvas {
            return target;
        }
        if self.dismiss_top_toolbar_menus() {
            self.input_state.needs_redraw = true;
            return target;
        }
        if self.press_overlay_chrome(screen_x, screen_y) {
            return target;
        }
        if self.pointer.board_pan_key_held() && self.can_start_board_pan() {
            self.pointer.start_board_pan(screen_position);
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
        routed: RoutedInput,
    ) {
        let Some(screen_position) = routed.screen else {
            return;
        };
        let screen_x = screen_position.0.round() as i32;
        let screen_y = screen_position.1.round() as i32;
        self.pointer.set_position((screen_x, screen_y));

        if self.input_state.region_is_active() {
            if target == TouchTarget::Canvas {
                self.update_region_selection(
                    RegionInputSource::Touch,
                    screen_position.0,
                    screen_position.1,
                );
            }
            return;
        }

        if self.input_state.eyedropper_is_active() {
            if target == TouchTarget::Canvas {
                self.update_eyedropper_hover(screen_position.0, screen_position.1);
            }
            return;
        }

        if self.toolbar_drag.is_moving()
            && let Some(kind) = self.toolbar_drag.kind()
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
            self.toolbar_chrome.set_pointer_over_toolbar(true);
            let (wx, wy) = self.zoomed_world_coords(screen_position.0, screen_position.1);
            self.input_state
                .update_pointer_positions(screen_x, screen_y, wx, wy);
            let evt = self.toolbar.pointer_motion(surface, position);
            if self.toolbar_drag.item_dragging() {
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

        if target != TouchTarget::Canvas {
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
        _position: (f64, f64),
        target: TouchTarget,
        routed: RoutedInput,
    ) {
        if self.touch_up_screen_modal(routed) || self.touch_up_consumed(routed) {
            return;
        }
        if self.touch_up_toolbar(surface, target) {
            return;
        }
        self.touch_up_canvas(target, routed);
    }

    fn touch_up_screen_modal(&mut self, routed: RoutedInput) -> bool {
        if !self.input_state.region_is_active() {
            return false;
        }
        if self
            .pointer
            .take_suppressed_release(RegionInputSource::Touch)
        {
            return true;
        }
        if let Some((x, y)) = routed.screen {
            self.finish_region_selection(RegionInputSource::Touch, x, y);
        } else {
            self.cancel_region_selection_from(RegionInputSource::Touch);
        }
        true
    }

    fn touch_up_consumed(&mut self, routed: RoutedInput) -> bool {
        if self
            .pointer
            .take_suppressed_release(RegionInputSource::Touch)
        {
            self.pointer.clear_chrome_press();
            return true;
        }
        let help_owned = match routed.screen {
            Some((x, y)) => self.handle_help_overlay_release(
                HelpOverlayPressSource::Touch,
                x.round() as i32,
                y.round() as i32,
            ),
            None => self
                .input_state
                .clear_help_overlay_press_for(HelpOverlayPressSource::Touch),
        };
        if help_owned {
            self.pointer.clear_chrome_press();
            return true;
        }
        if !self.input_state.command_palette.open && !self.input_state.tour.is_active() {
            return false;
        }
        self.pointer.clear_chrome_press();
        self.cancel_active_touch_sequence();
        true
    }

    fn touch_up_toolbar(&mut self, surface: &wl_surface::WlSurface, target: TouchTarget) -> bool {
        if target != TouchTarget::Toolbar {
            return false;
        }
        if debug_toolbar_drag_logging_enabled() {
            debug!(
                "touch release: target={:?}, drag_active={}, toolbar_dragging={}",
                target,
                self.toolbar_drag.is_moving(),
                self.toolbar_drag.item_dragging()
            );
        }
        self.toolbar.pointer_leave(surface);
        self.toolbar_chrome.set_pointer_over_toolbar(false);
        self.finish_toolbar_item_drag(true);
        self.toolbar_drag.set_item_dragging(false);
        self.end_toolbar_move_drag();
        self.toolbar.mark_dirty();
        self.input_state.needs_redraw = true;
        true
    }

    fn touch_up_canvas(&mut self, target: TouchTarget, routed: RoutedInput) {
        let Some(screen_position) = routed.screen else {
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
                self.toolbar_drag.is_moving(),
                self.toolbar_drag.item_dragging()
            );
        }
        if target == TouchTarget::InlineToolbar {
            let _ = self.inline_toolbar_release(screen_position);
            return;
        }
        if self.toolbar_drag.is_moving() {
            self.finish_toolbar_item_drag(true);
            self.toolbar_drag.set_item_dragging(false);
            self.end_toolbar_move_drag();
            return;
        }
        if target != TouchTarget::Canvas {
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
