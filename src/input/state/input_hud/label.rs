//! Display labels for input HUD chips.
//!
//! Key names follow the vocabulary `docs/CONFIG.md` and the help overlay
//! already print, so a chip always reads like the binding it would match.
//! Arrows are the only deliberate divergence: the HUD is a visual surface, so
//! `ArrowUp` renders as the glyph.

use crate::input::events::Key;
use crate::input::modifiers::Modifiers;

/// Modifier prefix in the canonical `Ctrl+Shift+Alt+` order the keybinding
/// parser and `KeyBinding`'s `Display` both use. Empty when nothing is held.
pub(crate) fn modifier_prefix(modifiers: Modifiers) -> String {
    let mut prefix = String::new();
    if modifiers.ctrl {
        prefix.push_str("Ctrl+");
    }
    if modifiers.shift {
        prefix.push_str("Shift+");
    }
    if modifiers.alt {
        prefix.push_str("Alt+");
    }
    prefix
}

/// Whether a key is a bare modifier press (its own chip only when
/// `show_bare_modifiers` is on).
pub fn is_bare_modifier(key: Key) -> bool {
    matches!(key, Key::Shift | Key::Ctrl | Key::Alt)
}

/// Display name of a single key without modifiers, or `None` for keys the HUD
/// deliberately skips (unmapped keysyms and control characters that would
/// render as an empty or invisible chip).
pub(crate) fn key_display_name(key: Key) -> Option<String> {
    let name = match key {
        Key::Char(c) => {
            if c.is_control() || c == '\u{0}' {
                return None;
            }
            return Some(c.to_uppercase().to_string());
        }
        Key::Escape => "Esc",
        Key::Return => "Enter",
        Key::Backspace => "Backspace",
        Key::Tab => "Tab",
        Key::Space => "Space",
        Key::Up => "\u{2191}",
        Key::Down => "\u{2193}",
        Key::Left => "\u{2190}",
        Key::Right => "\u{2192}",
        Key::Delete => "Delete",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::Shift => "Shift",
        Key::Ctrl => "Ctrl",
        Key::Alt => "Alt",
        Key::Menu => "Menu",
        Key::F1 => "F1",
        Key::F2 => "F2",
        Key::F3 => "F3",
        Key::F4 => "F4",
        Key::F5 => "F5",
        Key::F6 => "F6",
        Key::F7 => "F7",
        Key::F8 => "F8",
        Key::F9 => "F9",
        Key::F10 => "F10",
        Key::F11 => "F11",
        Key::F12 => "F12",
        Key::Unknown => return None,
    };
    Some(name.to_string())
}

/// Chord label for a key press: the held modifiers in canonical order plus the
/// key's display name. A bare modifier press reports only itself, so holding
/// Ctrl never renders as `Ctrl+Ctrl`.
pub fn input_hud_key_label(key: Key, modifiers: Modifiers) -> Option<String> {
    let name = key_display_name(key)?;
    if is_bare_modifier(key) {
        return Some(name);
    }
    Some(format!("{}{}", modifier_prefix(modifiers), name))
}

/// Chord label for a pointer button press.
pub fn input_hud_mouse_label(button: &str, modifiers: Modifiers) -> String {
    format!("{}{}", modifier_prefix(modifiers), button)
}

/// Chord label for a scroll tick.
pub fn input_hud_scroll_label(up: bool, modifiers: Modifiers) -> String {
    let arrow = if up { "\u{2191}" } else { "\u{2193}" };
    format!("{}Scroll {}", modifier_prefix(modifiers), arrow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods(ctrl: bool, shift: bool, alt: bool) -> Modifiers {
        Modifiers {
            shift,
            ctrl,
            alt,
            tab: false,
        }
    }

    #[test]
    fn chord_labels_use_the_canonical_modifier_order() {
        assert_eq!(
            input_hud_key_label(Key::Char('z'), mods(true, true, false)).as_deref(),
            Some("Ctrl+Shift+Z")
        );
        assert_eq!(
            input_hud_key_label(Key::F10, mods(false, false, true)).as_deref(),
            Some("Alt+F10")
        );
        assert_eq!(
            input_hud_key_label(Key::Char('a'), mods(false, false, false)).as_deref(),
            Some("A")
        );
    }

    #[test]
    fn special_keys_use_the_help_overlay_names_and_arrow_glyphs() {
        assert_eq!(
            input_hud_key_label(Key::Space, mods(false, false, false)).as_deref(),
            Some("Space")
        );
        assert_eq!(
            input_hud_key_label(Key::Escape, mods(false, false, false)).as_deref(),
            Some("Esc")
        );
        assert_eq!(
            input_hud_key_label(Key::Up, mods(false, false, false)).as_deref(),
            Some("\u{2191}")
        );
        assert_eq!(
            input_hud_key_label(Key::Left, mods(false, false, false)).as_deref(),
            Some("\u{2190}")
        );
    }

    #[test]
    fn bare_modifier_presses_do_not_prefix_themselves() {
        assert_eq!(
            input_hud_key_label(Key::Ctrl, mods(true, false, false)).as_deref(),
            Some("Ctrl")
        );
        assert_eq!(
            input_hud_key_label(Key::Shift, mods(true, true, false)).as_deref(),
            Some("Shift")
        );
    }

    #[test]
    fn unmapped_keys_are_skipped() {
        assert!(input_hud_key_label(Key::Unknown, mods(false, false, false)).is_none());
        assert!(input_hud_key_label(Key::Char('\u{1}'), mods(false, false, false)).is_none());
    }

    #[test]
    fn mouse_and_scroll_labels_carry_the_modifier_prefix() {
        assert_eq!(
            input_hud_mouse_label("Click", mods(true, false, false)),
            "Ctrl+Click"
        );
        assert_eq!(
            input_hud_scroll_label(true, mods(false, false, false)),
            "Scroll \u{2191}"
        );
        assert_eq!(
            input_hud_scroll_label(false, mods(false, true, false)),
            "Shift+Scroll \u{2193}"
        );
    }
}
