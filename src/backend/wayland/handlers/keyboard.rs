// Bridges Wayland key events into our `InputState`, including capture-action plumbing.
use log::debug;
use smithay_client_toolkit::seat::keyboard::{
    KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers,
};
use wayland_client::{
    Connection, QueueHandle,
    protocol::{wl_keyboard, wl_surface},
};

use crate::input::Key;

use super::super::state::WaylandState;

impl KeyboardHandler for WaylandState {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
        debug!("Keyboard focus entered");
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
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        let key = keysym_to_key(event.keysym);
        debug!("Key pressed: {:?}", key);
        self.input_state.on_key_press(key);
        self.input_state.needs_redraw = true;

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
    }

    fn repeat_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        let key = keysym_to_key(event.keysym);
        debug!("Key repeated: {:?}", key);
        self.input_state.on_key_press(key);
        self.input_state.needs_redraw = true;
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
