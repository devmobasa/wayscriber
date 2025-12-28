// Feeds pointer events (motion/buttons/scroll) into the drawing state to keep the canvas reactive.
use log::{debug, warn};
use smithay_client_toolkit::seat::pointer::{
    BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, CursorIcon, PointerEvent, PointerEventKind, PointerHandler,
};
use wayland_client::{Connection, QueueHandle, protocol::wl_pointer};

use crate::backend::wayland::state::{debug_toolbar_drag_logging_enabled, surface_id};
use crate::backend::wayland::toolbar_intent::intent_to_event;
use crate::input::{EraserMode, MouseButton, Tool};
use crate::ui::toolbar::ToolbarEvent;

use super::super::state::WaylandState;

impl PointerHandler for WaylandState {
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let update_cursor = |toolbar_hover: bool, conn: &Connection, this: &mut WaylandState| {
            if let Some(pointer) = this.themed_pointer.as_ref() {
                let icon = if toolbar_hover {
                    CursorIcon::Default
                } else {
                    CursorIcon::Crosshair
                };
                if this.current_pointer_shape.map_or(true, |s| s != icon) {
                    if let Err(err) = pointer.set_cursor(conn, icon) {
                        warn!("Failed to set cursor icon: {}", err);
                    } else {
                        this.current_pointer_shape = Some(icon);
                    }
                }
            }
        };

        for event in events {
            let on_toolbar = self.toolbar.is_toolbar_surface(&event.surface);
            let inline_active = self.inline_toolbars_active() && self.toolbar.is_visible();
            if debug_toolbar_drag_logging_enabled() {
                debug!(
                    "pointer {:?}: seat={:?}, surface={}, on_toolbar={}, inline_active={}, pos=({:.1}, {:.1}), drag_active={}, toolbar_dragging={}, pointer_over_toolbar={}",
                    event.kind,
                    self.current_seat_id(),
                    surface_id(&event.surface),
                    on_toolbar,
                    inline_active,
                    event.position.0,
                    event.position.1,
                    self.is_move_dragging(),
                    self.toolbar_dragging(),
                    self.pointer_over_toolbar()
                );
            }
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    debug!(
                        "Pointer entered at ({}, {}), on_toolbar={}, is_move_dragging={}",
                        event.position.0,
                        event.position.1,
                        on_toolbar,
                        self.is_move_dragging()
                    );
                    self.set_pointer_focus(true);
                    self.set_pointer_over_toolbar(on_toolbar);
                    self.set_current_mouse(event.position.0 as i32, event.position.1 as i32);
                    if on_toolbar {
                        // Ensure pointer-driven visuals (e.g. eraser hover) update once on enter.
                        self.input_state.needs_redraw = true;
                    }
                    if !on_toolbar {
                        let (wx, wy) = self.zoomed_world_coords(event.position.0, event.position.1);
                        self.input_state.update_pointer_position(wx, wy);
                        if self.input_state.eraser_mode == EraserMode::Stroke
                            && self.input_state.active_tool() == Tool::Eraser
                        {
                            self.input_state.needs_redraw = true;
                        }
                    }
                    update_cursor(on_toolbar, conn, self);
                    if inline_active {
                        self.inline_toolbar_motion(event.position);
                    }
                }
                PointerEventKind::Leave { .. } => {
                    debug!(
                        "Pointer left surface: on_toolbar={}, is_move_dragging={}",
                        on_toolbar,
                        self.is_move_dragging()
                    );
                    self.set_pointer_focus(false);
                    if on_toolbar {
                        self.set_pointer_over_toolbar(false);
                        self.toolbar.pointer_leave(&event.surface);
                        // Don't clear drag state if we're in a move drag - the user may be
                        // dragging the toolbar and their pointer left the toolbar surface.
                        // The drag will continue on the main surface.
                        if !self.is_move_dragging() {
                            debug!("Clearing toolbar drag state on leave");
                            self.set_toolbar_dragging(false);
                            self.end_toolbar_move_drag();
                        } else {
                            debug!("Preserving move drag state on toolbar leave");
                        }
                        self.toolbar.mark_dirty();
                        // Ensure pointer-driven visuals (e.g. eraser hover) update once on leave.
                        self.input_state.needs_redraw = true;
                    }
                    if !on_toolbar
                        && self.input_state.eraser_mode == EraserMode::Stroke
                        && self.input_state.active_tool() == Tool::Eraser
                    {
                        self.input_state.needs_redraw = true;
                    }
                    if inline_active {
                        self.inline_toolbar_leave();
                    }
                    if (on_toolbar || inline_active) && !self.is_move_dragging() {
                        self.end_toolbar_move_drag();
                    }
                    self.current_pointer_shape = None;
                }
                PointerEventKind::Motion { .. } => {
                    if self.is_move_dragging()
                        && let Some(kind) = self.active_move_drag_kind()
                    {
                        debug!(
                            "Move drag motion: kind={:?}, pos=({}, {}), on_toolbar={}",
                            kind, event.position.0, event.position.1, on_toolbar
                        );
                        // On toolbar surface: coords are toolbar-local, need conversion
                        // On main surface: coords are already screen-relative (fullscreen overlay)
                        if on_toolbar {
                            self.handle_toolbar_move(kind, event.position);
                        } else {
                            self.handle_toolbar_move_screen(kind, event.position);
                        }
                        self.toolbar.mark_dirty();
                        if inline_active {
                            self.input_state.needs_redraw = true;
                        }
                        continue;
                    }
                    if inline_active && self.inline_toolbar_motion(event.position) {
                        update_cursor(true, conn, self);
                        continue;
                    }
                    if on_toolbar {
                        self.set_pointer_over_toolbar(true);
                        let evt = self.toolbar.pointer_motion(&event.surface, event.position);
                        if self.toolbar_dragging() {
                            // Use move_drag_intent if pointer_motion didn't return an intent
                            // This allows dragging to continue when mouse moves outside hit region
                            let intent = evt.or_else(|| {
                                self.move_drag_intent(event.position.0, event.position.1)
                            });
                            if let Some(intent) = intent {
                                let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                                self.handle_toolbar_event(evt);
                            }
                        } else {
                            self.toolbar.mark_dirty();
                        }
                        if inline_active {
                            self.input_state.needs_redraw = true;
                        }
                        self.refresh_keyboard_interactivity();
                        update_cursor(true, conn, self);
                        continue;
                    }
                    if self.pointer_over_toolbar() {
                        let evt = self.toolbar.pointer_motion(&event.surface, event.position);
                        if self.toolbar_dragging() {
                            // Use move_drag_intent if pointer_motion didn't return an intent
                            // This allows dragging to continue when mouse moves outside hit region
                            let intent = evt.or_else(|| {
                                self.move_drag_intent(event.position.0, event.position.1)
                            });
                            if let Some(intent) = intent {
                                let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                                self.handle_toolbar_event(evt);
                            }
                        } else {
                            self.toolbar.mark_dirty();
                        }
                        if inline_active {
                            self.input_state.needs_redraw = true;
                        }
                        self.refresh_keyboard_interactivity();
                        update_cursor(true, conn, self);
                        continue;
                    }
                    update_cursor(false, conn, self);
                    // Handle move drag that continues on the main surface after leaving toolbar
                    if self.is_move_dragging() {
                        if let Some(intent) =
                            self.move_drag_intent(event.position.0, event.position.1)
                        {
                            let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                            self.handle_toolbar_event(evt);
                            self.toolbar.mark_dirty();
                            self.input_state.needs_redraw = true;
                        }
                        continue;
                    }
                    if self.zoom.panning {
                        self.set_current_mouse(event.position.0 as i32, event.position.1 as i32);
                        let (dx, dy) = self
                            .zoom
                            .update_pan_position(event.position.0, event.position.1);
                        self.zoom.pan_by_screen_delta(
                            dx,
                            dy,
                            self.surface.width(),
                            self.surface.height(),
                        );
                        self.input_state.dirty_tracker.mark_full();
                        self.input_state.needs_redraw = true;
                        continue;
                    }
                    self.set_current_mouse(event.position.0 as i32, event.position.1 as i32);
                    let (wx, wy) = self.zoomed_world_coords(event.position.0, event.position.1);
                    self.input_state.update_pointer_position(wx, wy);
                    self.input_state.on_mouse_motion(wx, wy);
                }
                PointerEventKind::Press { button, .. } => {
                    if debug_toolbar_drag_logging_enabled() {
                        debug!(
                            "pointer press: button={}, on_toolbar={}, inline_active={}, drag_active={}",
                            button,
                            on_toolbar,
                            inline_active,
                            self.is_move_dragging()
                        );
                    }
                    if inline_active
                        && ((button == BTN_LEFT && self.inline_toolbar_press(event.position))
                            || self.pointer_over_toolbar())
                    {
                        continue;
                    }
                    if on_toolbar {
                        if button == BTN_LEFT
                            && let Some((intent, drag)) =
                                self.toolbar.pointer_press(&event.surface, event.position)
                        {
                            let toolbar_event =
                                intent_to_event(intent, self.toolbar.last_snapshot());
                            if matches!(
                                toolbar_event,
                                ToolbarEvent::MoveTopToolbar { .. }
                                    | ToolbarEvent::MoveSideToolbar { .. }
                            ) && drag
                            {
                                self.lock_pointer_for_drag(qh, &event.surface);
                            }
                            log::info!(
                                "toolbar press: drag_start={}, surface={}, seat={:?}, inline_active={}",
                                drag,
                                surface_id(&event.surface),
                                self.current_seat_id(),
                                self.inline_toolbars_active()
                            );
                            self.set_toolbar_dragging(drag);
                            self.handle_toolbar_event(toolbar_event);
                            self.toolbar.mark_dirty();
                            self.input_state.needs_redraw = true;
                            self.refresh_keyboard_interactivity();
                        }
                        continue;
                    } else if self.pointer_over_toolbar() {
                        self.set_toolbar_dragging(false);
                        continue;
                    }
                    debug!(
                        "Button {} pressed at ({}, {})",
                        button, event.position.0, event.position.1
                    );
                    if self.zoom.active && button == BTN_MIDDLE && !self.zoom.locked {
                        self.zoom.start_pan(event.position.0, event.position.1);
                        self.input_state.dirty_tracker.mark_full();
                        self.input_state.needs_redraw = true;
                        continue;
                    }

                    let mb = match button {
                        BTN_LEFT => MouseButton::Left,
                        BTN_MIDDLE => MouseButton::Middle,
                        BTN_RIGHT => MouseButton::Right,
                        _ => continue,
                    };

                    let (wx, wy) = self.zoomed_world_coords(event.position.0, event.position.1);
                    self.input_state.on_mouse_press(mb, wx, wy);
                    self.input_state.needs_redraw = true;
                }
                PointerEventKind::Release { button, .. } => {
                    if debug_toolbar_drag_logging_enabled() {
                        debug!(
                            "pointer release: button={}, on_toolbar={}, inline_active={}, drag_active={}, toolbar_dragging={}, pointer_over_toolbar={}",
                            button,
                            on_toolbar,
                            inline_active,
                            self.is_move_dragging(),
                            self.toolbar_dragging(),
                            self.pointer_over_toolbar()
                        );
                    }
                    if inline_active {
                        if button == BTN_LEFT && self.inline_toolbar_release(event.position) {
                            self.unlock_pointer();
                            continue;
                        }
                        if self.pointer_over_toolbar() || self.toolbar_dragging() {
                            self.end_toolbar_move_drag();
                            self.unlock_pointer();
                            continue;
                        }
                    }
                    if on_toolbar || self.pointer_over_toolbar() {
                        if button == BTN_LEFT {
                            self.set_toolbar_dragging(false);
                        }
                        self.end_toolbar_move_drag();
                        self.unlock_pointer();
                        continue;
                    }
                    // End move drag if released on the main surface
                    if button == BTN_LEFT && self.is_move_dragging() {
                        self.set_toolbar_dragging(false);
                        self.end_toolbar_move_drag();
                        self.unlock_pointer();
                        continue;
                    }
                    debug!("Button {} released", button);
                    if self.zoom.active && button == BTN_MIDDLE {
                        if self.zoom.panning {
                            self.zoom.stop_pan();
                            self.input_state.dirty_tracker.mark_full();
                            self.input_state.needs_redraw = true;
                        }
                        continue;
                    }

                    let mb = match button {
                        BTN_LEFT => MouseButton::Left,
                        BTN_MIDDLE => MouseButton::Middle,
                        BTN_RIGHT => MouseButton::Right,
                        _ => continue,
                    };

                    let (wx, wy) = self.zoomed_world_coords(event.position.0, event.position.1);
                    self.input_state.on_mouse_release(mb, wx, wy);
                    self.input_state.needs_redraw = true;
                }
                PointerEventKind::Axis { vertical, .. } => {
                    if on_toolbar || self.pointer_over_toolbar() {
                        continue;
                    }
                    let scroll_direction = if vertical.discrete != 0 {
                        vertical.discrete
                    } else if vertical.absolute.abs() > 0.1 {
                        if vertical.absolute > 0.0 { 1 } else { -1 }
                    } else {
                        0
                    };
                    if self.input_state.modifiers.ctrl && self.input_state.modifiers.alt {
                        if scroll_direction != 0 {
                            let zoom_in = scroll_direction < 0;
                            self.handle_zoom_scroll(zoom_in, event.position.0, event.position.1);
                        }
                        continue;
                    }

                    match scroll_direction.cmp(&0) {
                        std::cmp::Ordering::Greater if self.input_state.modifiers.shift => {
                            let prev_font_size = self.input_state.current_font_size;
                            self.input_state.adjust_font_size(-2.0);
                            debug!(
                                "Font size decreased: {:.1}px",
                                self.input_state.current_font_size
                            );
                            if (self.input_state.current_font_size - prev_font_size).abs()
                                > f64::EPSILON
                            {
                                self.save_drawing_preferences();
                            }
                        }
                        std::cmp::Ordering::Less if self.input_state.modifiers.shift => {
                            let prev_font_size = self.input_state.current_font_size;
                            self.input_state.adjust_font_size(2.0);
                            debug!(
                                "Font size increased: {:.1}px",
                                self.input_state.current_font_size
                            );
                            if (self.input_state.current_font_size - prev_font_size).abs()
                                > f64::EPSILON
                            {
                                self.save_drawing_preferences();
                            }
                        }
                        std::cmp::Ordering::Greater | std::cmp::Ordering::Less => {
                            let delta = if scroll_direction > 0 { -1.0 } else { 1.0 };
                            let eraser_active = self.input_state.active_tool() == Tool::Eraser;
                            #[cfg(tablet)]
                            let prev_thickness = self.input_state.current_thickness;

                            if self.input_state.nudge_thickness_for_active_tool(delta) {
                                if eraser_active {
                                    debug!(
                                        "Eraser size adjusted: {:.0}px",
                                        self.input_state.eraser_size
                                    );
                                } else {
                                    debug!(
                                        "Thickness adjusted: {:.0}px",
                                        self.input_state.current_thickness
                                    );
                                }
                                self.input_state.needs_redraw = true;
                                if !eraser_active {
                                    self.save_drawing_preferences();
                                }
                            }
                            #[cfg(tablet)]
                            if !eraser_active
                                && (self.input_state.current_thickness - prev_thickness).abs()
                                    > f64::EPSILON
                            {
                                self.stylus_base_thickness =
                                    Some(self.input_state.current_thickness);
                                if self.stylus_tip_down {
                                    self.stylus_pressure_thickness =
                                        Some(self.input_state.current_thickness);
                                    self.record_stylus_peak(self.input_state.current_thickness);
                                } else {
                                    self.stylus_pressure_thickness = None;
                                    self.stylus_peak_thickness = None;
                                }
                            }
                        }
                        std::cmp::Ordering::Equal => {}
                    }
                }
            }
        }
    }
}
