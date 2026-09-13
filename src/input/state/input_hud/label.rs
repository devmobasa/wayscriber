//! Display labels for input HUD chips.
//!
//! A chip reads like the binding it would match: the key's config name run
//! through the same [`crate::config::keybindings::key_display_name`] every
//! other surface uses, so `ArrowUp` is the glyph on the HUD exactly as it is
//! in the help overlay.

use crate::input::events::Key;
use crate::input::modifiers::Modifiers;

/// Modifier prefix in the canonical `Ctrl+Shift+Alt+Super+` order the keybinding
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
    if modifiers.logo {
        prefix.push_str("Super+");
    }
    prefix
}

/// Whether a key is a bare modifier press (its own chip only when
/// `show_bare_modifiers` is on).
pub fn is_bare_modifier(key: Key) -> bool {
    matches!(key, Key::Shift | Key::Ctrl | Key::Alt | Key::Super)
}

/// Display name of a single key without modifiers, or `None` for keys the HUD
/// deliberately skips (unmapped keysyms and control characters that would
/// render as an empty or invisible chip).
///
/// The match names each key the way `config.toml` spells it; the shared
/// display mapping then turns the named keys into their glyph or short name,
/// so this surface can never drift from the rest of the app. `Tab` and the
/// bare modifiers are HUD-only chips no binding can carry, and pass through.
pub(crate) fn key_display_name(key: Key) -> Option<String> {
    let name = match key {
        Key::Char(c) => {
            if c.is_control() || c == '\u{0}' {
                return None;
            }
            return Some(c.to_uppercase().to_string());
        }
        Key::Escape => "Escape",
        Key::Return => "Return",
        Key::Backspace => "Backspace",
        Key::Tab => "Tab",
        Key::Space => "Space",
        Key::Up => "ArrowUp",
        Key::Down => "ArrowDown",
        Key::Left => "ArrowLeft",
        Key::Right => "ArrowRight",
        Key::Delete => "Delete",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::Shift => "Shift",
        Key::Ctrl => "Ctrl",
        Key::Alt => "Alt",
        Key::Super => "Super",
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
    Some(crate::config::keybindings::key_display_name(name).to_string())
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

    fn mods(ctrl: bool, shift: bool, alt: bool, logo: bool) -> Modifiers {
        Modifiers {
            shift,
            ctrl,
            alt,
            logo,
            tab: false,
        }
    }

    #[test]
    fn chord_labels_use_the_canonical_modifier_order() {
        assert_eq!(
            input_hud_key_label(Key::Char('z'), mods(true, true, false, false)).as_deref(),
            Some("Ctrl+Shift+Z")
        );
        assert_eq!(
            input_hud_key_label(Key::F10, mods(false, false, true, false)).as_deref(),
            Some("Alt+F10")
        );
        assert_eq!(
            input_hud_key_label(Key::Char('a'), mods(false, false, false, false)).as_deref(),
            Some("A")
        );
        assert_eq!(
            input_hud_key_label(Key::Char('x'), mods(false, false, false, true)).as_deref(),
            Some("Super+X")
        );
        assert_eq!(
            input_hud_key_label(Key::F5, mods(false, true, false, true)).as_deref(),
            Some("Shift+Super+F5")
        );
    }

    #[test]
    fn special_keys_use_the_shared_display_names_and_arrow_glyphs() {
        assert_eq!(
            input_hud_key_label(Key::Space, mods(false, false, false, false)).as_deref(),
            Some("Space")
        );
        assert_eq!(
            input_hud_key_label(Key::Escape, mods(false, false, false, false)).as_deref(),
            Some("Esc")
        );
        assert_eq!(
            input_hud_key_label(Key::Up, mods(false, false, false, false)).as_deref(),
            Some("\u{2191}")
        );
        assert_eq!(
            input_hud_key_label(Key::Left, mods(false, false, false, false)).as_deref(),
            Some("\u{2190}")
        );
        // The chip agrees with what a binding on the same key would show.
        for (key, name) in [
            (Key::PageUp, "PageUp"),
            (Key::Delete, "Delete"),
            (Key::Backspace, "Backspace"),
            (Key::Return, "Return"),
        ] {
            assert_eq!(
                input_hud_key_label(key, mods(false, false, false, false)).as_deref(),
                Some(crate::config::keybindings::key_display_name(name))
            );
        }
        // Tab is a HUD-only chip: no binding can carry it, so it stays a word.
        assert_eq!(
            input_hud_key_label(Key::Tab, mods(false, false, false, false)).as_deref(),
            Some("Tab")
        );
    }

    #[test]
    fn bare_modifier_presses_do_not_prefix_themselves() {
        assert_eq!(
            input_hud_key_label(Key::Ctrl, mods(true, false, false, false)).as_deref(),
            Some("Ctrl")
        );
        assert_eq!(
            input_hud_key_label(Key::Shift, mods(true, true, false, false)).as_deref(),
            Some("Shift")
        );
        assert_eq!(
            input_hud_key_label(Key::Super, mods(false, false, false, true)).as_deref(),
            Some("Super")
        );
    }

    #[test]
    fn unmapped_keys_are_skipped() {
        assert!(input_hud_key_label(Key::Unknown, mods(false, false, false, false)).is_none());
        assert!(
            input_hud_key_label(Key::Char('\u{1}'), mods(false, false, false, false)).is_none()
        );
    }

    #[test]
    fn mouse_and_scroll_labels_carry_the_modifier_prefix() {
        assert_eq!(
            input_hud_mouse_label("Click", mods(true, false, false, false)),
            "Ctrl+Click"
        );
        assert_eq!(
            input_hud_scroll_label(true, mods(false, false, false, false)),
            "Scroll \u{2191}"
        );
        assert_eq!(
            input_hud_scroll_label(false, mods(false, true, false, false)),
            "Shift+Scroll \u{2193}"
        );
    }
}
