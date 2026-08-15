//! Pure translation from libinput/xkb values to input HUD chip labels.
//!
//! Kept free of FFI handles so everything around the libinput edge stays unit
//! testable: keysyms and button codes in, display labels out.

use xkbcommon::xkb;

use super::super::handlers::keyboard::keysym_to_key;
use crate::input::Modifiers;
use crate::input::state::{input_hud_key_label, input_hud_mouse_label, input_hud_scroll_label};

/// evdev keycodes are offset by 8 from xkb keycodes.
pub(super) const EVDEV_KEYCODE_OFFSET: u32 = 8;

/// evdev button codes for the three primary pointer buttons.
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;

/// Effective modifier state from an xkb state, mapped onto the same
/// `Modifiers` value the overlay path formats chords from.
pub(super) fn modifiers_from_xkb(state: &xkb::State) -> Modifiers {
    Modifiers {
        shift: state.mod_name_is_active(xkb::MOD_NAME_SHIFT, xkb::STATE_MODS_EFFECTIVE),
        ctrl: state.mod_name_is_active(xkb::MOD_NAME_CTRL, xkb::STATE_MODS_EFFECTIVE),
        alt: state.mod_name_is_active(xkb::MOD_NAME_ALT, xkb::STATE_MODS_EFFECTIVE),
        logo: state.mod_name_is_active(xkb::MOD_NAME_LOGO, xkb::STATE_MODS_EFFECTIVE),
        // Tab is a drag modifier the overlay tracks itself; it is not an xkb
        // modifier and never contributes to a chord label.
        tab: false,
    }
}

/// Chip label for a key press, or `None` for keysyms the HUD skips.
pub(super) fn key_label(keysym: xkb::Keysym, modifiers: Modifiers) -> Option<(String, bool)> {
    let key = keysym_to_key(keysym);
    let label = input_hud_key_label(key, modifiers)?;
    Some((label, crate::input::state::is_bare_modifier(key)))
}

/// Chip label for a pointer button press.
pub(super) fn button_label(button: u32, modifiers: Modifiers) -> String {
    let name = match button {
        BTN_LEFT => "Click".to_string(),
        BTN_RIGHT => "Right Click".to_string(),
        BTN_MIDDLE => "Middle Click".to_string(),
        other => format!("Button {other}"),
    };
    input_hud_mouse_label(&name, modifiers)
}

/// Chip label for a stylus tip-down, matching the overlay tablet hook's
/// "Pen" chip vocabulary.
pub(super) fn pen_label(modifiers: Modifiers) -> String {
    input_hud_mouse_label("Pen", modifiers)
}

/// Chip label for a scroll tick. libinput reports positive vertical values for
/// downward scrolling, matching the Wayland axis convention.
pub(super) fn scroll_label(value: f64, modifiers: Modifiers) -> Option<String> {
    if value == 0.0 {
        return None;
    }
    Some(input_hud_scroll_label(value < 0.0, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_modifiers() -> Modifiers {
        Modifiers::new()
    }

    /// A default RMLVO keymap is available on any machine with xkeyboard-config
    /// installed; skip rather than fail where it is not (headless CI images).
    fn default_state() -> Option<xkb::State> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "",
            "",
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )?;
        Some(xkb::State::new(&keymap))
    }

    #[test]
    fn keysyms_translate_to_the_shared_chip_vocabulary() {
        let mut modifiers = no_modifiers();
        modifiers.ctrl = true;
        let (label, bare) =
            key_label(xkb::Keysym::z, modifiers).expect("printable keysym has a label");
        assert_eq!(label, "Ctrl+Z");
        assert!(!bare);

        let (space, _) = key_label(xkb::Keysym::space, no_modifiers()).expect("space has a label");
        assert_eq!(space, "Space");

        let (up, _) = key_label(xkb::Keysym::Up, no_modifiers()).expect("arrow has a label");
        assert_eq!(up, "\u{2191}");
    }

    #[test]
    fn bare_modifier_keysyms_are_flagged() {
        let (label, bare) =
            key_label(xkb::Keysym::Control_L, no_modifiers()).expect("modifier has a label");
        assert_eq!(label, "Ctrl");
        assert!(bare);
    }

    #[test]
    fn button_labels_name_the_primary_buttons_and_number_the_rest() {
        assert_eq!(button_label(BTN_LEFT, no_modifiers()), "Click");
        assert_eq!(button_label(BTN_RIGHT, no_modifiers()), "Right Click");
        assert_eq!(button_label(BTN_MIDDLE, no_modifiers()), "Middle Click");
        assert_eq!(button_label(0x116, no_modifiers()), "Button 278");
    }

    #[test]
    fn pen_labels_match_the_overlay_tablet_vocabulary() {
        assert_eq!(pen_label(no_modifiers()), "Pen");
        let mut modifiers = no_modifiers();
        modifiers.ctrl = true;
        assert_eq!(pen_label(modifiers), "Ctrl+Pen");
    }

    #[test]
    fn scroll_labels_follow_the_wayland_axis_convention() {
        assert_eq!(
            scroll_label(-1.0, no_modifiers()).as_deref(),
            Some("Scroll \u{2191}")
        );
        assert_eq!(
            scroll_label(1.0, no_modifiers()).as_deref(),
            Some("Scroll \u{2193}")
        );
        assert!(scroll_label(0.0, no_modifiers()).is_none());
    }

    /// The full keysym path against a real keymap: an evdev keycode resolves to
    /// the same label the overlay path would print for that key.
    #[test]
    fn a_default_keymap_resolves_evdev_keycodes_to_labels() {
        let Some(state) = default_state() else {
            eprintln!("no default xkb keymap available; skipping");
            return;
        };
        // evdev KEY_ESC is 1.
        let keysym = state.key_get_one_sym(xkb::Keycode::new(1 + EVDEV_KEYCODE_OFFSET));
        let (label, bare) = key_label(keysym, modifiers_from_xkb(&state)).expect("escape label");
        assert_eq!(label, "Esc");
        assert!(!bare);
    }
}
