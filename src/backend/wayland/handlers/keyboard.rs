// Bridges Wayland key events into our `InputState`, including capture-action plumbing.
use log::{debug, warn};
use smithay_client_toolkit::seat::keyboard::{
    KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers,
};
use wayland_client::{
    Connection, QueueHandle,
    protocol::{wl_keyboard, wl_surface},
};

use crate::{input::Key, notification};

use super::super::state::WaylandState;

impl KeyboardHandler for WaylandState {
    fn enter(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
        debug!("Keyboard focus entered");
        self.set_keyboard_focus(true);
        self.set_last_activation_serial(Some(serial));
        self.maybe_retry_activation(qh);
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        debug!("Keyboard focus left");
        self.set_keyboard_focus(false);

        // When the compositor moves focus away from our surface (e.g. to a portal
        // dialog, another layer surface, or a different window), it's possible for
        // us to miss some key release events. To avoid leaving modifiers "stuck"
        // and breaking shortcuts/tools, aggressively reset our modifier state on
        // focus loss.
        self.input_state.reset_modifiers();

        if self.surface.is_xdg_window() {
            warn!("Keyboard focus lost in xdg fallback; exiting overlay");
            notification::send_notification_async(
                &self.tokio_handle,
                "Wayscriber lost focus".to_string(),
                "GNOME could not keep the overlay focused; closing fallback window.".to_string(),
                Some("dialog-warning".to_string()),
            );
            self.input_state.should_exit = true;
        }
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        #[cfg(tablet)]
        let prev_thickness = self.input_state.current_thickness;
        let key = keysym_to_key(event.keysym);
        if self.zoom.is_engaged() {
            match key {
                Key::Escape => {
                    self.exit_zoom();
                    return;
                }
                Key::Up | Key::Down | Key::Left | Key::Right => {
                    if !self.zoom.active {
                        return;
                    }
                    if self.zoom.locked {
                        return;
                    }
                    let step = if self.input_state.modifiers.shift {
                        WaylandState::ZOOM_PAN_STEP_LARGE
                    } else {
                        WaylandState::ZOOM_PAN_STEP
                    };
                    let (dx, dy) = match key {
                        Key::Up => (0.0, step),
                        Key::Down => (0.0, -step),
                        Key::Left => (step, 0.0),
                        Key::Right => (-step, 0.0),
                        _ => (0.0, 0.0),
                    };
                    self.zoom.pan_by_screen_delta(
                        dx,
                        dy,
                        self.surface.width(),
                        self.surface.height(),
                    );
                    self.input_state.dirty_tracker.mark_full();
                    self.input_state.needs_redraw = true;
                    return;
                }
                _ => {}
            }
        }
        debug!("Key pressed: {:?}", key);
        let prefs_before = (
            self.input_state.current_color,
            self.input_state.current_thickness,
            self.input_state.eraser_mode,
            self.input_state.marker_opacity,
            self.input_state.current_font_size,
            self.input_state.font_descriptor.clone(),
            self.input_state.fill_enabled,
        );
        self.input_state.on_key_press(key);
        self.input_state.needs_redraw = true;
        let prefs_changed = prefs_before.0 != self.input_state.current_color
            || (prefs_before.1 - self.input_state.current_thickness).abs() > f64::EPSILON
            || prefs_before.2 != self.input_state.eraser_mode
            || (prefs_before.3 - self.input_state.marker_opacity).abs() > f64::EPSILON
            || (prefs_before.4 - self.input_state.current_font_size).abs() > f64::EPSILON
            || prefs_before.5 != self.input_state.font_descriptor
            || prefs_before.6 != self.input_state.fill_enabled;
        if prefs_changed {
            self.save_drawing_preferences();
        }

        #[cfg(tablet)]
        if (self.input_state.current_thickness - prev_thickness).abs() > f64::EPSILON {
            self.stylus_base_thickness = Some(self.input_state.current_thickness);
            if self.stylus_tip_down {
                self.stylus_pressure_thickness = Some(self.input_state.current_thickness);
                self.record_stylus_peak(self.input_state.current_thickness);
            } else {
                self.stylus_pressure_thickness = None;
                self.stylus_peak_thickness = None;
            }
        }

        if let Some(action) = self.input_state.take_pending_capture_action() {
            self.handle_capture_action(action);
        }
        if let Some(action) = self.input_state.take_pending_zoom_action() {
            self.handle_zoom_action(action);
        }
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        let key = keysym_to_key(event.keysym);
        debug!("Key released: {:?}", key);
        self.input_state.on_key_release(key);
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
        _layout: RawModifiers,
        _group: u32,
    ) {
        debug!(
            "Modifiers: ctrl={} alt={} shift={}",
            modifiers.ctrl, modifiers.alt, modifiers.shift
        );
        // Trust compositor-reported modifier state to reconcile any missed key release
        // events and avoid "stuck" modifiers.
        self.input_state
            .sync_modifiers(modifiers.shift, modifiers.ctrl, modifiers.alt);
    }

    fn repeat_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        #[cfg(tablet)]
        let prev_thickness = self.input_state.current_thickness;
        let key = keysym_to_key(event.keysym);
        if self.zoom.active {
            match key {
                Key::Up | Key::Down | Key::Left | Key::Right => {
                    if self.zoom.locked {
                        return;
                    }
                    let step = if self.input_state.modifiers.shift {
                        WaylandState::ZOOM_PAN_STEP_LARGE
                    } else {
                        WaylandState::ZOOM_PAN_STEP
                    };
                    let (dx, dy) = match key {
                        Key::Up => (0.0, step),
                        Key::Down => (0.0, -step),
                        Key::Left => (step, 0.0),
                        Key::Right => (-step, 0.0),
                        _ => (0.0, 0.0),
                    };
                    self.zoom.pan_by_screen_delta(
                        dx,
                        dy,
                        self.surface.width(),
                        self.surface.height(),
                    );
                    self.input_state.dirty_tracker.mark_full();
                    self.input_state.needs_redraw = true;
                    return;
                }
                _ => {}
            }
        }
        debug!("Key repeated: {:?}", key);
        let prefs_before = (
            self.input_state.current_color,
            self.input_state.current_thickness,
            self.input_state.eraser_mode,
            self.input_state.marker_opacity,
            self.input_state.current_font_size,
            self.input_state.font_descriptor.clone(),
            self.input_state.fill_enabled,
        );
        self.input_state.on_key_press(key);
        self.input_state.needs_redraw = true;
        let prefs_changed = prefs_before.0 != self.input_state.current_color
            || (prefs_before.1 - self.input_state.current_thickness).abs() > f64::EPSILON
            || prefs_before.2 != self.input_state.eraser_mode
            || (prefs_before.3 - self.input_state.marker_opacity).abs() > f64::EPSILON
            || (prefs_before.4 - self.input_state.current_font_size).abs() > f64::EPSILON
            || prefs_before.5 != self.input_state.font_descriptor
            || prefs_before.6 != self.input_state.fill_enabled;
        if prefs_changed {
            self.save_drawing_preferences();
        }

        #[cfg(tablet)]
        if (self.input_state.current_thickness - prev_thickness).abs() > f64::EPSILON {
            self.stylus_base_thickness = Some(self.input_state.current_thickness);
            if self.stylus_tip_down {
                self.stylus_pressure_thickness = Some(self.input_state.current_thickness);
                self.record_stylus_peak(self.input_state.current_thickness);
            } else {
                self.stylus_pressure_thickness = None;
                self.stylus_peak_thickness = None;
            }
        }

        if let Some(action) = self.input_state.take_pending_zoom_action() {
            self.handle_zoom_action(action);
        }
    }
}

fn keysym_to_key(keysym: Keysym) -> Key {
    match keysym {
        Keysym::Escape => Key::Escape,
        Keysym::Return => Key::Return,
        Keysym::BackSpace => Key::Backspace,
        Keysym::Tab => Key::Tab,
        Keysym::space => Key::Space,
        Keysym::Up => Key::Up,
        Keysym::Down => Key::Down,
        Keysym::Left => Key::Left,
        Keysym::Right => Key::Right,
        Keysym::Delete => Key::Delete,
        Keysym::Home => Key::Home,
        Keysym::End => Key::End,
        Keysym::Shift_L | Keysym::Shift_R => Key::Shift,
        Keysym::Control_L | Keysym::Control_R => Key::Ctrl,
        Keysym::Alt_L | Keysym::Alt_R => Key::Alt,
        Keysym::Menu => Key::Menu,
        Keysym::plus => Key::Char('+'),
        Keysym::equal => Key::Char('='),
        Keysym::minus => Key::Char('-'),
        Keysym::underscore => Key::Char('_'),
        Keysym::_0 | Keysym::KP_0 => Key::Char('0'),
        Keysym::t => Key::Char('t'),
        Keysym::T => Key::Char('T'),
        Keysym::e => Key::Char('e'),
        Keysym::E => Key::Char('E'),
        Keysym::r => Key::Char('r'),
        Keysym::R => Key::Char('R'),
        Keysym::g => Key::Char('g'),
        Keysym::G => Key::Char('G'),
        Keysym::b => Key::Char('b'),
        Keysym::B => Key::Char('B'),
        Keysym::y => Key::Char('y'),
        Keysym::Y => Key::Char('Y'),
        Keysym::o => Key::Char('o'),
        Keysym::O => Key::Char('O'),
        Keysym::p => Key::Char('p'),
        Keysym::P => Key::Char('P'),
        Keysym::w => Key::Char('w'),
        Keysym::W => Key::Char('W'),
        Keysym::k => Key::Char('k'),
        Keysym::K => Key::Char('K'),
        Keysym::z => Key::Char('z'),
        Keysym::Z => Key::Char('Z'),
        Keysym::F1 => Key::F1,
        Keysym::F2 => Key::F2,
        Keysym::F4 => Key::F4,
        Keysym::F9 => Key::F9,
        Keysym::F10 => Key::F10,
        Keysym::F11 => Key::F11,
        Keysym::F12 => Key::F12,
        _ => {
            let raw = keysym.raw();
            if (0x20..=0x7E).contains(&raw) {
                Key::Char(raw as u8 as char)
            } else {
                Key::Unknown
            }
        }
    }
}
