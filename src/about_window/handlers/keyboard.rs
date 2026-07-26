use smithay_client_toolkit::seat::keyboard::{
    KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers,
};
use wayland_client::{
    Connection, QueueHandle,
    protocol::{wl_keyboard, wl_surface},
};

use super::super::AboutWindowState;

/// What a key press means to the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    FocusNext,
    FocusPrevious,
    Activate,
    Close,
}

/// Translate a key press. `shift` disambiguates Tab on layouts that do not
/// send `ISO_Left_Tab`.
fn command_for(keysym: Keysym, shift: bool) -> Option<Command> {
    match keysym {
        Keysym::Escape | Keysym::q => Some(Command::Close),
        Keysym::Tab | Keysym::KP_Tab if shift => Some(Command::FocusPrevious),
        Keysym::Tab | Keysym::KP_Tab => Some(Command::FocusNext),
        Keysym::ISO_Left_Tab => Some(Command::FocusPrevious),
        Keysym::Down | Keysym::Right => Some(Command::FocusNext),
        Keysym::Up | Keysym::Left => Some(Command::FocusPrevious),
        Keysym::Return | Keysym::KP_Enter | Keysym::space => Some(Command::Activate),
        _ => None,
    }
}

impl AboutWindowState {
    fn handle_key(&mut self, keysym: Keysym) {
        match command_for(keysym, self.shift_held) {
            Some(Command::FocusNext) => self.move_focus(1),
            Some(Command::FocusPrevious) => self.move_focus(-1),
            Some(Command::Activate) => self.activate_focus(),
            Some(Command::Close) => self.should_exit = true,
            None => {}
        }
    }
}

impl KeyboardHandler for AboutWindowState {
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
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        // Modifier state is only valid while focused.
        self.shift_held = false;
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        self.handle_key(event.keysym);
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
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
        self.shift_held = modifiers.shift;
    }

    fn repeat_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        // Holding Tab should keep walking the focus ring.
        self.handle_key(event.keysym);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_walks_the_focus_ring_in_both_directions() {
        assert_eq!(command_for(Keysym::Tab, false), Some(Command::FocusNext));
        assert_eq!(command_for(Keysym::Tab, true), Some(Command::FocusPrevious));
        assert_eq!(
            command_for(Keysym::ISO_Left_Tab, false),
            Some(Command::FocusPrevious)
        );
    }

    #[test]
    fn arrows_activation_and_close_are_mapped() {
        assert_eq!(command_for(Keysym::Down, false), Some(Command::FocusNext));
        assert_eq!(command_for(Keysym::Up, false), Some(Command::FocusPrevious));
        assert_eq!(command_for(Keysym::Return, false), Some(Command::Activate));
        assert_eq!(command_for(Keysym::space, false), Some(Command::Activate));
        assert_eq!(command_for(Keysym::Escape, false), Some(Command::Close));
        assert_eq!(command_for(Keysym::q, false), Some(Command::Close));
        assert_eq!(command_for(Keysym::a, false), None);
    }
}
