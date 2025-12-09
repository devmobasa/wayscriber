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
        debug!("Key pressed: {:?}", key);
        self.input_state.on_key_press(key);
        self.input_state.needs_redraw = true;

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
        debug!("Key repeated: {:?}", key);
        self.input_state.on_key_press(key);
        self.input_state.needs_redraw = true;

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
