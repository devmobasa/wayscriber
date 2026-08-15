use smithay_client_toolkit::seat::keyboard::Keysym;

use crate::input::Key;

pub(in crate::backend::wayland) fn keysym_to_key(keysym: Keysym) -> Key {
    match keysym {
        Keysym::Escape => Key::Escape,
        Keysym::Return | Keysym::KP_Enter => Key::Return,
        Keysym::BackSpace => Key::Backspace,
        Keysym::Tab | Keysym::KP_Tab => Key::Tab,
        Keysym::space => Key::Space,
        Keysym::Up | Keysym::KP_Up => Key::Up,
        Keysym::Down | Keysym::KP_Down => Key::Down,
        Keysym::Left | Keysym::KP_Left => Key::Left,
        Keysym::Right | Keysym::KP_Right => Key::Right,
        Keysym::Delete | Keysym::KP_Delete => Key::Delete,
        Keysym::Home | Keysym::KP_Home => Key::Home,
        Keysym::End | Keysym::KP_End => Key::End,
        Keysym::Page_Up | Keysym::KP_Page_Up => Key::PageUp,
        Keysym::Page_Down | Keysym::KP_Page_Down => Key::PageDown,
        Keysym::Shift_L | Keysym::Shift_R => Key::Shift,
        Keysym::Control_L | Keysym::Control_R => Key::Ctrl,
        Keysym::Alt_L | Keysym::Alt_R => Key::Alt,
        Keysym::Super_L | Keysym::Super_R | Keysym::Hyper_L | Keysym::Hyper_R => Key::Super,
        Keysym::Menu => Key::Menu,
        Keysym::F1 => Key::F1,
        Keysym::F2 => Key::F2,
        Keysym::F3 => Key::F3,
        Keysym::F4 => Key::F4,
        Keysym::F5 => Key::F5,
        Keysym::F6 => Key::F6,
        Keysym::F7 => Key::F7,
        Keysym::F8 => Key::F8,
        Keysym::F9 => Key::F9,
        Keysym::F10 => Key::F10,
        Keysym::F11 => Key::F11,
        Keysym::F12 => Key::F12,
        _ => keysym.key_char().map_or(Key::Unknown, Key::Char),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn super_and_hyper_keysyms_map_to_the_super_modifier() {
        assert_eq!(keysym_to_key(Keysym::Super_L), Key::Super);
        assert_eq!(keysym_to_key(Keysym::Super_R), Key::Super);
        assert_eq!(keysym_to_key(Keysym::Hyper_L), Key::Super);
        assert_eq!(keysym_to_key(Keysym::Hyper_R), Key::Super);
        assert_eq!(keysym_to_key(Keysym::Alt_L), Key::Alt);
    }

    #[test]
    fn keypad_navigation_keysyms_match_the_non_keypad_names() {
        assert_eq!(keysym_to_key(Keysym::KP_Home), Key::Home);
        assert_eq!(keysym_to_key(Keysym::Home), Key::Home);
        assert_eq!(keysym_to_key(Keysym::KP_Left), Key::Left);
        assert_eq!(keysym_to_key(Keysym::KP_Delete), Key::Delete);
        assert_eq!(keysym_to_key(Keysym::KP_End), Key::End);
        assert_eq!(keysym_to_key(Keysym::KP_Up), Key::Up);
        assert_eq!(keysym_to_key(Keysym::KP_Down), Key::Down);
        assert_eq!(keysym_to_key(Keysym::KP_Right), Key::Right);
        assert_eq!(keysym_to_key(Keysym::KP_Page_Up), Key::PageUp);
        assert_eq!(keysym_to_key(Keysym::KP_Page_Down), Key::PageDown);
    }
}
