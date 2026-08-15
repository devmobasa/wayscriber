//! GDK key-event normalization for the configurator shortcut recorder.
//!
//! The runtime still matches [`wayscriber::config::KeyBinding`]. This module
//! only translates a keyval plus modifier flags into that type, so the
//! recorder can be tested without a GTK display.

use wayscriber::config::KeyBinding;

use super::field::KeybindingField;

/// Modifier flags taken from a GDK key event.
///
/// Super, Meta, and Hyper all map onto the runtime Super/logo modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyboardModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub super_held: bool,
}

impl KeyboardModifiers {
    /// Canonical modifier prefix, including the trailing `+` when any
    /// supported modifier is down.
    pub fn prefix(self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.super_held {
            parts.push("Super");
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("{}+", parts.join("+"))
        }
    }
}

/// Result of one recorder key event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordedKeyboard {
    /// Bare modifiers: keep the recorder open and show `preview`.
    Pending { preview: String },
    /// A complete chord ready to add.
    Chord(KeyBinding),
    /// The key cannot be stored; the recorder stays open.
    Unsupported { message: String },
}

/// Live recorder owned by [`crate::app::state::ConfiguratorApp`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutRecorderState {
    pub field: KeybindingField,
    pub prompt: String,
}

impl ShortcutRecorderState {
    pub fn new(field: KeybindingField) -> Self {
        Self {
            field,
            prompt: waiting_prompt(),
        }
    }
}

pub fn waiting_prompt() -> String {
    "Press a shortcut.".to_string()
}

/// Map a GDK/X11 keyval and current modifiers onto the shared binding type.
pub fn normalize_key_event(keyval: u32, modifiers: KeyboardModifiers) -> RecordedKeyboard {
    if is_supported_modifier_key(keyval) {
        let preview = pending_preview(modifiers);
        return RecordedKeyboard::Pending { preview };
    }

    match key_name_from_keyval(keyval, modifiers.shift) {
        Some(key) => {
            let binding = KeyBinding {
                key,
                ctrl: modifiers.ctrl,
                shift: modifiers.shift,
                alt: modifiers.alt,
                logo: modifiers.super_held,
            };
            RecordedKeyboard::Chord(binding)
        }
        None => RecordedKeyboard::Unsupported {
            message: unsupported_key_message().to_string(),
        },
    }
}

pub fn pending_preview(modifiers: KeyboardModifiers) -> String {
    let prefix = modifiers.prefix();
    if prefix.is_empty() {
        waiting_prompt()
    } else {
        format!("{prefix}…")
    }
}

pub fn super_consumed_hint() -> &'static str {
    "If Super never appears, the desktop captured it. Use Edit as Text."
}

pub fn unsupported_key_message() -> &'static str {
    "This key cannot be recorded. Use Edit as Text if Wayscriber names it."
}

fn is_supported_modifier_key(keyval: u32) -> bool {
    matches!(
        keyval,
        keyval::SHIFT_L
            | keyval::SHIFT_R
            | keyval::CONTROL_L
            | keyval::CONTROL_R
            | keyval::CAPS_LOCK
            | keyval::SHIFT_LOCK
            | keyval::ALT_L
            | keyval::ALT_R
            | keyval::META_L
            | keyval::META_R
            | keyval::SUPER_L
            | keyval::SUPER_R
            | keyval::HYPER_L
            | keyval::HYPER_R
            | keyval::ISO_LEVEL3_SHIFT
            | keyval::NUM_LOCK
    )
}

fn key_name_from_keyval(keyval: u32, shift: bool) -> Option<String> {
    if let Some(name) = named_key(keyval) {
        return Some(name.to_string());
    }

    let character = keyval_to_unicode(keyval)?;
    if character.is_control() {
        return None;
    }

    if character == ' ' {
        return Some("Space".to_string());
    }

    if shift
        && let Some(unshifted) = unshifted_punctuation(character)
        && character != '+'
    {
        return Some(unshifted.to_string());
    }

    if character.is_ascii_alphabetic() {
        return Some(character.to_ascii_uppercase().to_string());
    }

    Some(character.to_string())
}

fn named_key(keyval: u32) -> Option<&'static str> {
    Some(match keyval {
        keyval::ESCAPE => "Escape",
        keyval::RETURN | keyval::KP_ENTER => "Return",
        keyval::BACKSPACE => "Backspace",
        keyval::MENU => "Menu",
        keyval::DELETE | keyval::KP_DELETE => "Delete",
        keyval::HOME | keyval::KP_HOME => "Home",
        keyval::END | keyval::KP_END => "End",
        keyval::PAGE_UP | keyval::KP_PAGE_UP => "PageUp",
        keyval::PAGE_DOWN | keyval::KP_PAGE_DOWN => "PageDown",
        keyval::LEFT | keyval::KP_LEFT => "ArrowLeft",
        keyval::UP | keyval::KP_UP => "ArrowUp",
        keyval::RIGHT | keyval::KP_RIGHT => "ArrowRight",
        keyval::DOWN | keyval::KP_DOWN => "ArrowDown",
        keyval::F1 => "F1",
        keyval::F2 => "F2",
        keyval::F3 => "F3",
        keyval::F4 => "F4",
        keyval::F5 => "F5",
        keyval::F6 => "F6",
        keyval::F7 => "F7",
        keyval::F8 => "F8",
        keyval::F9 => "F9",
        keyval::F10 => "F10",
        keyval::F11 => "F11",
        keyval::F12 => "F12",
        keyval::KP_ADD => "+",
        keyval::KP_SUBTRACT => "-",
        keyval::KP_MULTIPLY => "*",
        keyval::KP_DIVIDE => "/",
        keyval::KP_DECIMAL => ".",
        keyval::KP_0 => "0",
        keyval::KP_1 => "1",
        keyval::KP_2 => "2",
        keyval::KP_3 => "3",
        keyval::KP_4 => "4",
        keyval::KP_5 => "5",
        keyval::KP_6 => "6",
        keyval::KP_7 => "7",
        keyval::KP_8 => "8",
        keyval::KP_9 => "9",
        _ => return None,
    })
}

fn keyval_to_unicode(keyval: u32) -> Option<char> {
    if (0x20..0x7f).contains(&keyval) {
        return char::from_u32(keyval);
    }
    if (0x0100_0000..0x0111_0000).contains(&keyval) {
        return char::from_u32(keyval - 0x0100_0000);
    }
    None
}

/// Inverse of the runtime shifted-symbol fallback: Shift+1 records as `1`
/// with Shift, so dispatch that sees `!` can still match through that table.
fn unshifted_punctuation(character: char) -> Option<char> {
    Some(match character {
        '!' => '1',
        '@' => '2',
        '#' => '3',
        '$' => '4',
        '%' => '5',
        '^' => '6',
        '&' => '7',
        '*' => '8',
        '(' => '9',
        ')' => '0',
        '_' => '-',
        // '+' maps to '=' in the runtime fallback, but Ctrl+Shift++ is the
        // stored spelling for the plus key, so '+' is kept as-is by the
        // caller.
        '{' => '[',
        '}' => ']',
        '|' => '\\',
        ':' => ';',
        '"' => '\'',
        '<' => ',',
        '>' => '.',
        '?' => '/',
        '~' => '`',
        _ => return None,
    })
}

/// GDK/X11 keyvals used by the recorder. Numbers are the public GDK contract,
/// so tests do not need a display.
#[allow(dead_code)]
pub mod keyval {
    pub const ESCAPE: u32 = 0xff1b;
    pub const RETURN: u32 = 0xff0d;
    pub const KP_ENTER: u32 = 0xff8d;
    pub const BACKSPACE: u32 = 0xff08;
    pub const TAB: u32 = 0xff09;
    pub const MENU: u32 = 0xff67;
    pub const DELETE: u32 = 0xffff;
    pub const KP_DELETE: u32 = 0xff9f;
    pub const HOME: u32 = 0xff50;
    pub const END: u32 = 0xff57;
    pub const PAGE_UP: u32 = 0xff55;
    pub const PAGE_DOWN: u32 = 0xff56;
    pub const LEFT: u32 = 0xff51;
    pub const UP: u32 = 0xff52;
    pub const RIGHT: u32 = 0xff53;
    pub const DOWN: u32 = 0xff54;
    pub const KP_HOME: u32 = 0xff95;
    pub const KP_LEFT: u32 = 0xff96;
    pub const KP_UP: u32 = 0xff97;
    pub const KP_RIGHT: u32 = 0xff98;
    pub const KP_DOWN: u32 = 0xff99;
    pub const KP_PAGE_UP: u32 = 0xff9a;
    pub const KP_PAGE_DOWN: u32 = 0xff9b;
    pub const KP_END: u32 = 0xff9c;
    pub const F1: u32 = 0xffbe;
    pub const F2: u32 = 0xffbf;
    pub const F3: u32 = 0xffc0;
    pub const F4: u32 = 0xffc1;
    pub const F5: u32 = 0xffc2;
    pub const F6: u32 = 0xffc3;
    pub const F7: u32 = 0xffc4;
    pub const F8: u32 = 0xffc5;
    pub const F9: u32 = 0xffc6;
    pub const F10: u32 = 0xffc7;
    pub const F11: u32 = 0xffc8;
    pub const F12: u32 = 0xffc9;
    pub const SHIFT_L: u32 = 0xffe1;
    pub const SHIFT_R: u32 = 0xffe2;
    pub const CONTROL_L: u32 = 0xffe3;
    pub const CONTROL_R: u32 = 0xffe4;
    pub const CAPS_LOCK: u32 = 0xffe5;
    pub const SHIFT_LOCK: u32 = 0xffe6;
    pub const META_L: u32 = 0xffe7;
    pub const META_R: u32 = 0xffe8;
    pub const ALT_L: u32 = 0xffe9;
    pub const ALT_R: u32 = 0xffea;
    pub const SUPER_L: u32 = 0xffeb;
    pub const SUPER_R: u32 = 0xffec;
    pub const HYPER_L: u32 = 0xffed;
    pub const HYPER_R: u32 = 0xffee;
    pub const ISO_LEVEL3_SHIFT: u32 = 0xfe03;
    pub const NUM_LOCK: u32 = 0xff7f;
    pub const KP_0: u32 = 0xffb0;
    pub const KP_1: u32 = 0xffb1;
    pub const KP_2: u32 = 0xffb2;
    pub const KP_3: u32 = 0xffb3;
    pub const KP_4: u32 = 0xffb4;
    pub const KP_5: u32 = 0xffb5;
    pub const KP_6: u32 = 0xffb6;
    pub const KP_7: u32 = 0xffb7;
    pub const KP_8: u32 = 0xffb8;
    pub const KP_9: u32 = 0xffb9;
    pub const KP_DECIMAL: u32 = 0xffae;
    pub const KP_DIVIDE: u32 = 0xffaf;
    pub const KP_MULTIPLY: u32 = 0xffaa;
    pub const KP_SUBTRACT: u32 = 0xffad;
    pub const KP_ADD: u32 = 0xffab;
    pub const PLUS: u32 = 0x002b;
    pub const SPACE: u32 = 0x0020;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods(ctrl: bool, shift: bool, alt: bool, super_held: bool) -> KeyboardModifiers {
        KeyboardModifiers {
            ctrl,
            shift,
            alt,
            super_held,
        }
    }

    fn chord(keyval: u32, modifiers: KeyboardModifiers) -> KeyBinding {
        match normalize_key_event(keyval, modifiers) {
            RecordedKeyboard::Chord(binding) => binding,
            other => panic!("expected a chord, got {other:?}"),
        }
    }

    #[test]
    fn f5_records_as_f5() {
        let binding = chord(keyval::F5, KeyboardModifiers::default());
        assert_eq!(binding.to_string(), "F5");
    }

    #[test]
    fn ctrl_shift_x_uses_canonical_modifier_order() {
        let binding = chord(u32::from(b'x'), mods(true, true, false, false));
        assert_eq!(binding.to_string(), "Ctrl+Shift+X");
        let from_uppercase = chord(u32::from(b'X'), mods(true, true, false, false));
        assert_eq!(from_uppercase.to_string(), "Ctrl+Shift+X");
        assert_eq!(binding, from_uppercase);
    }

    #[test]
    fn shifted_punctuation_records_the_unshifted_key() {
        let binding = chord(u32::from(b'!'), mods(false, true, false, false));
        assert_eq!(binding.to_string(), "Shift+1");
        assert!(binding.matches("1", false, true, false, false));
        assert!(!binding.matches("!", false, true, false, false));
    }

    #[test]
    fn ctrl_shift_plus_round_trips_the_plus_key() {
        let binding = chord(keyval::PLUS, mods(true, true, false, false));
        assert_eq!(binding.key, "+");
        assert_eq!(binding.to_string(), "Ctrl+Shift++");
        let parsed = KeyBinding::parse("Ctrl+Shift++").expect("plus key parses");
        assert_eq!(binding, parsed);
    }

    #[test]
    fn bare_modifiers_stay_pending() {
        for keyval in [
            keyval::CONTROL_L,
            keyval::CONTROL_R,
            keyval::SHIFT_L,
            keyval::SHIFT_R,
            keyval::ALT_L,
            keyval::ALT_R,
            keyval::SUPER_L,
            keyval::SUPER_R,
            keyval::META_L,
            keyval::HYPER_L,
        ] {
            match normalize_key_event(keyval, mods(true, false, false, false)) {
                RecordedKeyboard::Pending { preview } => {
                    assert_eq!(preview, "Ctrl+…");
                }
                other => panic!("modifier {keyval:#x} should stay pending, got {other:?}"),
            }
        }
    }

    #[test]
    fn escape_and_backspace_are_recordable() {
        assert_eq!(
            chord(keyval::ESCAPE, KeyboardModifiers::default()).to_string(),
            "Escape"
        );
        assert_eq!(
            chord(keyval::BACKSPACE, KeyboardModifiers::default()).to_string(),
            "Backspace"
        );
    }

    #[test]
    fn unknown_keyvals_are_unsupported() {
        match normalize_key_event(0xffff_fffe, KeyboardModifiers::default()) {
            RecordedKeyboard::Unsupported { message } => {
                assert!(message.contains("cannot be recorded"));
            }
            other => panic!("expected unsupported, got {other:?}"),
        }
        match normalize_key_event(keyval::TAB, KeyboardModifiers::default()) {
            RecordedKeyboard::Unsupported { .. } => {}
            other => panic!("Tab is not a named binding key, got {other:?}"),
        }
    }

    #[test]
    fn super_chords_record_as_canonical_super() {
        let binding = chord(u32::from(b'x'), mods(false, false, false, true));
        assert_eq!(binding.to_string(), "Super+X");
        assert!(binding.logo);
        assert_eq!(binding, KeyBinding::parse("Meta+X").expect("Meta alias"));

        assert_eq!(
            chord(keyval::F5, mods(false, true, false, true)).to_string(),
            "Shift+Super+F5"
        );
    }

    #[test]
    fn bare_super_stays_pending() {
        match normalize_key_event(keyval::SUPER_L, mods(false, false, false, true)) {
            RecordedKeyboard::Pending { preview } => assert_eq!(preview, "Super+…"),
            other => panic!("expected Super to stay pending, got {other:?}"),
        }
    }

    #[test]
    fn space_and_return_use_named_keys() {
        assert_eq!(
            chord(keyval::SPACE, KeyboardModifiers::default()).to_string(),
            "Space"
        );
        assert_eq!(
            chord(keyval::RETURN, KeyboardModifiers::default()).to_string(),
            "Return"
        );
    }
}
