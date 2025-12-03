// Feeds pointer events (motion/buttons/scroll) into the drawing state to keep the canvas reactive.
use log::{debug, warn};
use smithay_client_toolkit::seat::pointer::{
    BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, CursorIcon, PointerEvent, PointerEventKind, PointerHandler,
};
use wayland_client::{Connection, QueueHandle, protocol::wl_pointer};

use crate::backend::wayland::toolbar_intent::intent_to_event;
use crate::input::{MouseButton, Tool};

use super::super::state::WaylandState;

impl PointerHandler for WaylandState {
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let on_toolbar = self.toolbar.is_toolbar_surface(&event.surface);
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    debug!(
                        "Pointer entered at ({}, {})",
                        event.position.0, event.position.1
                    );
                    self.set_pointer_focus(true);
                    self.set_pointer_over_toolbar(on_toolbar);
                    self.set_current_mouse(event.position.0 as i32, event.position.1 as i32);
                    if !on_toolbar {
                        let (mx, my) = self.current_mouse();
                        self.input_state.update_pointer_position(mx, my);
                    }
                    if let Some(pointer) = self.themed_pointer.as_ref() {
                        if let Err(err) = pointer.set_cursor(conn, CursorIcon::Crosshair) {
                            warn!("Failed to set cursor icon: {}", err);
                        }
                        if on_toolbar {
                            if let Err(err) = pointer.set_cursor(conn, CursorIcon::Default) {
                                warn!("Failed to set toolbar cursor icon: {}", err);
                            }
                        }
                    }
                }
                PointerEventKind::Leave { .. } => {
                    debug!("Pointer left surface");
                    self.set_pointer_focus(false);
                    if on_toolbar {
                        self.set_pointer_over_toolbar(false);
                        self.toolbar.pointer_leave(&event.surface);
                        self.set_toolbar_dragging(false);
                        self.toolbar.mark_dirty();
                        self.input_state.needs_redraw = true;
                    }
                }
                PointerEventKind::Motion { .. } => {
                    if on_toolbar {
                        self.set_pointer_over_toolbar(true);
                        let evt = self.toolbar.pointer_motion(&event.surface, event.position);
                        if self.toolbar_dragging() {
                            if let Some(intent) = evt {
                                let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                                self.handle_toolbar_event(evt);
                            }
                        } else {
                            self.toolbar.mark_dirty();
                        }
                        self.input_state.needs_redraw = true;
                        self.refresh_keyboard_interactivity();
                        continue;
                    }
                    if self.pointer_over_toolbar() {
                        let evt = self.toolbar.pointer_motion(&event.surface, event.position);
                        if self.toolbar_dragging() {
                            if let Some(intent) = evt {
                                let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                                self.handle_toolbar_event(evt);
                            }
                        } else {
                            self.toolbar.mark_dirty();
                        }
                        self.input_state.needs_redraw = true;
                        self.refresh_keyboard_interactivity();
                        continue;
                    }
                    self.set_current_mouse(event.position.0 as i32, event.position.1 as i32);
                    let (mx, my) = self.current_mouse();
                    self.input_state.update_pointer_position(mx, my);
                    self.input_state.on_mouse_motion(mx, my);
                }
                PointerEventKind::Press { button, .. } => {
                    if on_toolbar {
                        if button == BTN_LEFT {
                            if let Some((intent, drag)) =
                                self.toolbar.pointer_press(&event.surface, event.position)
                            {
                                self.set_toolbar_dragging(drag);
                                let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                                self.handle_toolbar_event(evt);
                                self.toolbar.mark_dirty();
                                self.input_state.needs_redraw = true;
                                self.refresh_keyboard_interactivity();
                            }
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

                    let mb = match button {
                        BTN_LEFT => MouseButton::Left,
                        BTN_MIDDLE => MouseButton::Middle,
                        BTN_RIGHT => MouseButton::Right,
                        _ => continue,
                    };

                    self.input_state.on_mouse_press(
                        mb,
                        event.position.0 as i32,
                        event.position.1 as i32,
                    );
                    self.input_state.needs_redraw = true;
                }
                PointerEventKind::Release { button, .. } => {
                    if on_toolbar || self.pointer_over_toolbar() {
                        if button == BTN_LEFT {
                            self.set_toolbar_dragging(false);
                        }
                        continue;
                    }
                    debug!("Button {} released", button);

                    let mb = match button {
                        BTN_LEFT => MouseButton::Left,
                        BTN_MIDDLE => MouseButton::Middle,
                        BTN_RIGHT => MouseButton::Right,
                        _ => continue,
                    };

                    self.input_state.on_mouse_release(
                        mb,
                        event.position.0 as i32,
                        event.position.1 as i32,
                    );
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

                    match scroll_direction.cmp(&0) {
                        std::cmp::Ordering::Greater if self.input_state.modifiers.shift => {
                            self.input_state.adjust_font_size(-2.0);
                            debug!(
                                "Font size decreased: {:.1}px",
                                self.input_state.current_font_size
                            );
                        }
                        std::cmp::Ordering::Less if self.input_state.modifiers.shift => {
                            self.input_state.adjust_font_size(2.0);
                            debug!(
                                "Font size increased: {:.1}px",
                                self.input_state.current_font_size
                            );
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
