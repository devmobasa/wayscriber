// Feeds pointer events (motion/buttons/scroll) into the drawing state to keep the canvas reactive.
use log::{debug, warn};
use smithay_client_toolkit::seat::pointer::{
    BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, CursorIcon, PointerEvent, PointerEventKind, PointerHandler,
};
use wayland_client::{Connection, QueueHandle, protocol::wl_pointer};

use crate::input::{
    MouseButton,
    state::{MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS},
};

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
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    debug!(
                        "Pointer entered at ({}, {})",
                        event.position.0, event.position.1
                    );
                    self.has_pointer_focus = true;
                    self.current_mouse_x = event.position.0 as i32;
                    self.current_mouse_y = event.position.1 as i32;
                    self.input_state
                        .update_pointer_position(self.current_mouse_x, self.current_mouse_y);
                    if let Some(pointer) = self.themed_pointer.as_ref() {
                        if let Err(err) = pointer.set_cursor(conn, CursorIcon::Crosshair) {
                            warn!("Failed to set cursor icon: {}", err);
                        }
                    }
                }
                PointerEventKind::Leave { .. } => {
                    debug!("Pointer left surface");
                    self.has_pointer_focus = false;
                }
                PointerEventKind::Motion { .. } => {
                    self.current_mouse_x = event.position.0 as i32;
                    self.current_mouse_y = event.position.1 as i32;
                    self.input_state
                        .update_pointer_position(self.current_mouse_x, self.current_mouse_y);
                    self.input_state
                        .on_mouse_motion(self.current_mouse_x, self.current_mouse_y);
                }
                PointerEventKind::Press { button, .. } => {
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
                    let scroll_direction = if vertical.discrete != 0 {
                        vertical.discrete
                    } else if vertical.absolute.abs() > 0.1 {
                        if vertical.absolute > 0.0 { 1 } else { -1 }
                    } else {
                        0
                    };

                    if self.input_state.modifiers.shift {
                        if scroll_direction > 0 {
                            self.input_state.adjust_font_size(-2.0);
                            debug!(
                                "Font size decreased: {:.1}px",
                                self.input_state.current_font_size
                            );
                        } else if scroll_direction < 0 {
                            self.input_state.adjust_font_size(2.0);
                            debug!(
                                "Font size increased: {:.1}px",
                                self.input_state.current_font_size
                            );
                        }
                    } else if scroll_direction > 0 {
                        self.input_state.current_thickness =
                            (self.input_state.current_thickness - 1.0).max(MIN_STROKE_THICKNESS);
                        debug!(
                            "Thickness decreased: {:.0}px",
                            self.input_state.current_thickness
                        );
                        self.input_state.needs_redraw = true;
                    } else if scroll_direction < 0 {
                        self.input_state.current_thickness =
                            (self.input_state.current_thickness + 1.0).min(MAX_STROKE_THICKNESS);
                        debug!(
                            "Thickness increased: {:.0}px",
                            self.input_state.current_thickness
                        );
                        self.input_state.needs_redraw = true;
                    }
                }
            }
        }
    }
}
