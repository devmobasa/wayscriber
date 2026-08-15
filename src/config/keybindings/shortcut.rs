//! Single-trigger shortcuts: keyboard chords, auxiliary pointer buttons, and
//! stylus barrel buttons.
//!
//! Existing keyboard strings keep parsing as [`ShortcutTrigger::Keyboard`].
//! Pointer and stylus names are reserved and never stored as key names.

use std::fmt;
use std::hash::{Hash, Hasher};

use super::binding::{
    ChordParts, KeyBinding, format_modifiers, parse_chord_parts, suggest_key_name,
};

/// Highest `MouseExtraN` index accepted in this release (`MouseExtra1`..=`MouseExtra4`).
pub const MAX_POINTER_EXTRA: u8 = 4;

/// Auxiliary mouse buttons that can dispatch configured actions.
///
/// Left, middle, and right stay drawing/UI controls and cannot be bound here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Back,
    Forward,
    Extra(u8),
}

impl PointerButton {
    pub fn parse_name(name: &str) -> Result<Self, String> {
        if name.eq_ignore_ascii_case("MouseBack") {
            return Ok(Self::Back);
        }
        if name.eq_ignore_ascii_case("MouseForward") {
            return Ok(Self::Forward);
        }
        if let Some(index) = mouse_extra_index(name) {
            if (1..=MAX_POINTER_EXTRA).contains(&index) {
                return Ok(Self::Extra(index));
            }
            return Err(format!(
                "Unsupported mouse button `{name}`. Extra buttons are MouseExtra1 through MouseExtra{MAX_POINTER_EXTRA}."
            ));
        }
        if is_primary_mouse_name(name) {
            return Err(format!(
                "`{name}` cannot be bound as an action. Left, middle, and right already own drawing and toolbar input."
            ));
        }
        if name.to_ascii_lowercase().starts_with("mouse") {
            return Err(format!(
                "Unknown mouse button `{name}`. Use MouseBack, MouseForward, or MouseExtra1 through MouseExtra{MAX_POINTER_EXTRA}."
            ));
        }
        Err(format!("Unknown mouse button `{name}`."))
    }

    pub fn name(self) -> String {
        match self {
            Self::Back => "MouseBack".to_string(),
            Self::Forward => "MouseForward".to_string(),
            Self::Extra(index) => format!("MouseExtra{index}"),
        }
    }
}

/// Stylus barrel buttons that can dispatch configured actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StylusButton {
    Primary,
    Secondary,
}

impl StylusButton {
    pub fn parse_name(name: &str) -> Result<Self, String> {
        if name.eq_ignore_ascii_case("StylusPrimary") {
            return Ok(Self::Primary);
        }
        if name.eq_ignore_ascii_case("StylusSecondary") {
            return Ok(Self::Secondary);
        }
        if name.to_ascii_lowercase().starts_with("stylus") {
            return Err(format!(
                "Unknown stylus button `{name}`. Use StylusPrimary or StylusSecondary."
            ));
        }
        Err(format!("Unknown stylus button `{name}`."))
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Primary => "StylusPrimary",
            Self::Secondary => "StylusSecondary",
        }
    }
}

/// One auxiliary mouse button plus the same modifier set as a keyboard chord.
#[derive(Debug, Clone, Eq)]
pub struct PointerTrigger {
    pub button: PointerButton,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub logo: bool,
}

impl PartialEq for PointerTrigger {
    fn eq(&self, other: &Self) -> bool {
        self.button == other.button
            && self.ctrl == other.ctrl
            && self.shift == other.shift
            && self.alt == other.alt
            && self.logo == other.logo
    }
}

impl Hash for PointerTrigger {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.button.hash(state);
        self.ctrl.hash(state);
        self.shift.hash(state);
        self.alt.hash(state);
        self.logo.hash(state);
    }
}

/// One stylus barrel button plus the same modifier set as a keyboard chord.
#[derive(Debug, Clone, Eq)]
pub struct StylusTrigger {
    pub button: StylusButton,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub logo: bool,
}

impl PartialEq for StylusTrigger {
    fn eq(&self, other: &Self) -> bool {
        self.button == other.button
            && self.ctrl == other.ctrl
            && self.shift == other.shift
            && self.alt == other.alt
            && self.logo == other.logo
    }
}

impl Hash for StylusTrigger {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.button.hash(state);
        self.ctrl.hash(state);
        self.shift.hash(state);
        self.alt.hash(state);
        self.logo.hash(state);
    }
}

/// One complete shortcut identity used for maps, conflicts, and display.
#[derive(Debug, Clone, Eq)]
pub enum ShortcutTrigger {
    Keyboard(KeyBinding),
    Pointer(PointerTrigger),
    Stylus(StylusTrigger),
}

impl PartialEq for ShortcutTrigger {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Keyboard(left), Self::Keyboard(right)) => left == right,
            (Self::Pointer(left), Self::Pointer(right)) => left == right,
            (Self::Stylus(left), Self::Stylus(right)) => left == right,
            _ => false,
        }
    }
}

impl Hash for ShortcutTrigger {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Keyboard(binding) => binding.hash(state),
            Self::Pointer(trigger) => trigger.hash(state),
            Self::Stylus(trigger) => trigger.hash(state),
        }
    }
}

impl From<KeyBinding> for ShortcutTrigger {
    fn from(binding: KeyBinding) -> Self {
        Self::Keyboard(binding)
    }
}

impl ShortcutTrigger {
    /// Parse one shortcut string: a keyboard chord or a reserved device name.
    pub fn parse(s: &str) -> Result<Self, String> {
        let parts = parse_chord_parts(s)?;
        if looks_like_pointer_name(&parts.key) {
            return Ok(Self::Pointer(PointerTrigger {
                button: PointerButton::parse_name(&parts.key)?,
                ctrl: parts.ctrl,
                shift: parts.shift,
                alt: parts.alt,
                logo: parts.logo,
            }));
        }
        if looks_like_stylus_name(&parts.key) {
            return Ok(Self::Stylus(StylusTrigger {
                button: StylusButton::parse_name(&parts.key)?,
                ctrl: parts.ctrl,
                shift: parts.shift,
                alt: parts.alt,
                logo: parts.logo,
            }));
        }
        Ok(Self::Keyboard(key_binding_from_parts(parts)))
    }

    pub fn as_key_binding(&self) -> Option<&KeyBinding> {
        match self {
            Self::Keyboard(binding) => Some(binding),
            Self::Pointer(_) | Self::Stylus(_) => None,
        }
    }

    /// Whether an input event can currently deliver this trigger.
    pub fn is_deliverable(&self) -> bool {
        match self {
            Self::Keyboard(binding) => super::binding::is_deliverable_key_name(&binding.key),
            Self::Pointer(_) | Self::Stylus(_) => true,
        }
    }

    pub fn unknown_key_suggestion(&self) -> Option<String> {
        match self {
            Self::Keyboard(binding) => suggest_key_name(&binding.key),
            Self::Pointer(_) | Self::Stylus(_) => None,
        }
    }

    pub fn unknown_key_name(&self) -> Option<String> {
        match self {
            Self::Keyboard(binding) => Some(binding.key.clone()),
            Self::Pointer(_) | Self::Stylus(_) => None,
        }
    }
}

impl fmt::Display for ShortcutTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Keyboard(binding) => write!(f, "{binding}"),
            Self::Pointer(trigger) => write_device_binding(
                f,
                trigger.ctrl,
                trigger.shift,
                trigger.alt,
                trigger.logo,
                &trigger.button.name(),
            ),
            Self::Stylus(trigger) => write_device_binding(
                f,
                trigger.ctrl,
                trigger.shift,
                trigger.alt,
                trigger.logo,
                trigger.button.name(),
            ),
        }
    }
}

fn write_device_binding(
    f: &mut fmt::Formatter<'_>,
    ctrl: bool,
    shift: bool,
    alt: bool,
    logo: bool,
    name: &str,
) -> fmt::Result {
    let mut parts = format_modifiers(ctrl, shift, alt, logo);
    parts.push(name);
    write!(f, "{}", parts.join("+"))
}

fn key_binding_from_parts(parts: ChordParts) -> KeyBinding {
    KeyBinding {
        key: parts.key,
        ctrl: parts.ctrl,
        shift: parts.shift,
        alt: parts.alt,
        logo: parts.logo,
    }
}

fn looks_like_pointer_name(name: &str) -> bool {
    name.to_ascii_lowercase().starts_with("mouse")
}

fn looks_like_stylus_name(name: &str) -> bool {
    name.to_ascii_lowercase().starts_with("stylus")
}

fn is_primary_mouse_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("MouseLeft")
        || name.eq_ignore_ascii_case("MouseMiddle")
        || name.eq_ignore_ascii_case("MouseRight")
        || name.eq_ignore_ascii_case("MousePrimary")
        || name.eq_ignore_ascii_case("MouseSecondary")
}

fn mouse_extra_index(name: &str) -> Option<u8> {
    let lowered = name.to_ascii_lowercase();
    let digits = lowered.strip_prefix("mouseextra")?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Linux evdev / Wayland `wl_pointer.button` codes for auxiliary mice.
pub mod linux {
    use super::{PointerButton, StylusButton};

    pub const BTN_SIDE: u32 = 0x113;
    pub const BTN_EXTRA: u32 = 0x114;
    pub const BTN_FORWARD: u32 = 0x115;
    pub const BTN_BACK: u32 = 0x116;
    pub const BTN_TASK: u32 = 0x117;
    pub const BTN_EXTRA2: u32 = 0x118;
    pub const BTN_EXTRA3: u32 = 0x119;
    pub const BTN_EXTRA4: u32 = 0x11a;
    pub const BTN_STYLUS: u32 = 0x14b;
    pub const BTN_STYLUS2: u32 = 0x14c;

    pub fn pointer_button(code: u32) -> Option<PointerButton> {
        match code {
            BTN_SIDE | BTN_BACK => Some(PointerButton::Back),
            BTN_EXTRA | BTN_FORWARD => Some(PointerButton::Forward),
            BTN_TASK => Some(PointerButton::Extra(1)),
            BTN_EXTRA2 => Some(PointerButton::Extra(2)),
            BTN_EXTRA3 => Some(PointerButton::Extra(3)),
            BTN_EXTRA4 => Some(PointerButton::Extra(4)),
            _ => None,
        }
    }

    pub fn stylus_button(code: u32) -> Option<StylusButton> {
        match code {
            BTN_STYLUS => Some(StylusButton::Primary),
            BTN_STYLUS2 => Some(StylusButton::Secondary),
            _ => None,
        }
    }
}

/// GTK/GDK 1-based button numbers (X11-style: 8 back, 9 forward, 10+ extras).
pub mod gdk {
    use super::{MAX_POINTER_EXTRA, PointerButton, StylusButton};

    pub fn pointer_button(button: u32) -> Option<PointerButton> {
        match button {
            8 => Some(PointerButton::Back),
            9 => Some(PointerButton::Forward),
            extra if (10..10 + u32::from(MAX_POINTER_EXTRA)).contains(&extra) => {
                Some(PointerButton::Extra((extra - 9) as u8))
            }
            _ => None,
        }
    }

    /// Barrel buttons on a GDK tablet/pen source. Tip (1) is not bindable here.
    pub fn stylus_button(button: u32) -> Option<StylusButton> {
        match button {
            2 => Some(StylusButton::Primary),
            3 => Some(StylusButton::Secondary),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_strings_still_parse_as_keyboard_triggers() {
        let trigger = ShortcutTrigger::parse("Ctrl+Shift+X").unwrap();
        assert_eq!(trigger.to_string(), "Ctrl+Shift+X");
        assert!(trigger.as_key_binding().is_some());
    }

    #[test]
    fn pointer_and_stylus_names_round_trip() {
        for text in [
            "MouseBack",
            "Ctrl+MouseForward",
            "Shift+Super+MouseExtra1",
            "StylusPrimary",
            "Alt+StylusSecondary",
        ] {
            let trigger = ShortcutTrigger::parse(text).unwrap();
            assert_eq!(trigger.to_string(), text);
            assert!(trigger.is_deliverable());
        }
    }

    #[test]
    fn primary_mouse_buttons_are_rejected() {
        for name in ["MouseLeft", "MouseMiddle", "MouseRight"] {
            let error = ShortcutTrigger::parse(name).unwrap_err();
            assert!(error.contains("cannot be bound"), "{name}: {error}");
        }
    }

    #[test]
    fn unknown_mouse_and_stylus_names_suggest_the_supported_set() {
        let mouse = ShortcutTrigger::parse("MouseThumb").unwrap_err();
        assert!(mouse.contains("MouseBack"), "{mouse}");
        let stylus = ShortcutTrigger::parse("StylusBarrel").unwrap_err();
        assert!(stylus.contains("StylusPrimary"), "{stylus}");
    }

    #[test]
    fn linux_and_gdk_codes_share_semantic_names() {
        assert_eq!(
            linux::pointer_button(linux::BTN_SIDE),
            Some(PointerButton::Back)
        );
        assert_eq!(
            linux::pointer_button(linux::BTN_BACK),
            Some(PointerButton::Back)
        );
        assert_eq!(
            linux::pointer_button(linux::BTN_EXTRA),
            Some(PointerButton::Forward)
        );
        assert_eq!(gdk::pointer_button(8), Some(PointerButton::Back));
        assert_eq!(gdk::pointer_button(9), Some(PointerButton::Forward));
        assert_eq!(gdk::pointer_button(10), Some(PointerButton::Extra(1)));
        assert_eq!(gdk::pointer_button(1), None);
        assert_eq!(
            linux::stylus_button(linux::BTN_STYLUS),
            Some(StylusButton::Primary)
        );
        assert_eq!(gdk::stylus_button(2), Some(StylusButton::Primary));
    }
}
