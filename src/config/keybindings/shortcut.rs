//! Shortcut identity: a single trigger or a short keyboard sequence.
//!
//! Existing keyboard strings keep parsing as
//! [`Shortcut::Single`]`(`[`ShortcutTrigger::Keyboard`]`)`. Pointer and stylus
//! names are reserved and never stored as key names. Sequences use `>` between
//! keyboard chords (`Ctrl+K > Ctrl+C`).

use std::fmt;
use std::hash::{Hash, Hasher};

use super::binding::{
    ChordParts, KeyBinding, format_modifiers, parse_chord_parts, suggest_key_name,
};

/// Highest `MouseExtraN` index accepted in this release (`MouseExtra1`..=`MouseExtra4`).
pub const MAX_POINTER_EXTRA: u8 = 4;

/// Maximum keyboard chords in a sequence (`Ctrl+K > Ctrl+C > Ctrl+V`).
pub const MAX_SEQUENCE_STEPS: usize = 3;

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
            Self::Pointer(_) => true,
            #[cfg(feature = "tablet-input")]
            Self::Stylus(_) => true,
            #[cfg(not(feature = "tablet-input"))]
            Self::Stylus(_) => false,
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
            Self::Pointer(_) | Self::Stylus(_) if !self.is_deliverable() => Some(self.to_string()),
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

/// One configured shortcut: a single trigger or a two-to-three-step keyboard
/// sequence.
///
/// Storage uses spaced `>` between steps. User-facing labels use `then`.
#[derive(Debug, Clone, Eq)]
pub enum Shortcut {
    Single(ShortcutTrigger),
    Sequence(Vec<KeyBinding>),
}

impl PartialEq for Shortcut {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Single(left), Self::Single(right)) => left == right,
            (Self::Sequence(left), Self::Sequence(right)) => left == right,
            _ => false,
        }
    }
}

impl Hash for Shortcut {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Single(trigger) => trigger.hash(state),
            Self::Sequence(steps) => steps.hash(state),
        }
    }
}

impl From<ShortcutTrigger> for Shortcut {
    fn from(trigger: ShortcutTrigger) -> Self {
        Self::Single(trigger)
    }
}

impl From<KeyBinding> for Shortcut {
    fn from(binding: KeyBinding) -> Self {
        Self::Single(ShortcutTrigger::Keyboard(binding))
    }
}

impl Shortcut {
    /// Parse one shortcut string: a single trigger or a keyboard sequence.
    pub fn parse(s: &str) -> Result<Self, String> {
        let Some(parts) = split_sequence_steps(s) else {
            return ShortcutTrigger::parse(s).map(Self::Single);
        };
        if parts.iter().any(|part| part.is_empty()) {
            if parses_as_greater_than_key(s) {
                return ShortcutTrigger::parse(s).map(Self::Single);
            }
            return Err(
                "Empty sequence step. Use a complete chord on each side of `>`, for example Ctrl+K > Ctrl+C."
                    .to_string(),
            );
        }
        if parts.len() == 1 {
            return ShortcutTrigger::parse(parts[0]).map(Self::Single);
        }
        parse_keyboard_sequence(&parts)
    }

    pub fn as_trigger(&self) -> Option<&ShortcutTrigger> {
        match self {
            Self::Single(trigger) => Some(trigger),
            Self::Sequence(_) => None,
        }
    }

    pub fn as_key_binding(&self) -> Option<&KeyBinding> {
        self.as_trigger().and_then(ShortcutTrigger::as_key_binding)
    }

    /// Keyboard chords that make up this shortcut, if it is keyboard-only.
    pub fn keyboard_steps(&self) -> Option<&[KeyBinding]> {
        match self {
            Self::Single(ShortcutTrigger::Keyboard(binding)) => Some(std::slice::from_ref(binding)),
            Self::Sequence(steps) => Some(steps.as_slice()),
            Self::Single(_) => None,
        }
    }

    /// Whether one keyboard shortcut is a strict prefix of the other.
    ///
    /// A standalone chord that starts a sequence, or a shorter sequence that
    /// starts a longer one, cannot be dispatched without delaying the prefix.
    pub fn prefix_conflicts_with(&self, other: &Self) -> bool {
        let Some(left) = self.keyboard_steps() else {
            return false;
        };
        let Some(right) = other.keyboard_steps() else {
            return false;
        };
        left != right && (left.starts_with(right) || right.starts_with(left))
    }

    /// Label for chips, help, and the command palette (`Ctrl+K then Ctrl+C`).
    pub fn display_label(&self) -> String {
        match self {
            Self::Single(trigger) => trigger.to_string(),
            Self::Sequence(steps) => steps
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" then "),
        }
    }

    pub fn is_deliverable(&self) -> bool {
        match self {
            Self::Single(trigger) => trigger.is_deliverable(),
            Self::Sequence(steps) => steps
                .iter()
                .all(|step| super::binding::is_deliverable_key_name(&step.key)),
        }
    }

    pub fn unknown_key_suggestion(&self) -> Option<String> {
        match self {
            Self::Single(trigger) => trigger.unknown_key_suggestion(),
            Self::Sequence(steps) => steps.iter().find_map(|step| suggest_key_name(&step.key)),
        }
    }

    pub fn unknown_key_name(&self) -> Option<String> {
        match self {
            Self::Single(trigger) => trigger.unknown_key_name(),
            Self::Sequence(steps) => steps.iter().find_map(|step| {
                (!super::binding::is_deliverable_key_name(&step.key)).then(|| step.key.clone())
            }),
        }
    }

    /// Action that already claims this identity or a keyboard prefix/extension.
    pub fn claimed_by(
        &self,
        claimed: &std::collections::HashMap<Self, super::Action>,
    ) -> Option<super::Action> {
        if let Some(owner) = claimed.get(self) {
            return Some(*owner);
        }
        claimed
            .iter()
            .find_map(|(existing, owner)| existing.prefix_conflicts_with(self).then_some(*owner))
    }
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single(trigger) => write!(f, "{trigger}"),
            Self::Sequence(steps) => {
                let text = steps
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" > ");
                write!(f, "{text}")
            }
        }
    }
}

fn split_sequence_steps(s: &str) -> Option<Vec<&str>> {
    if s.contains(" > ") {
        return Some(s.split(" > ").map(str::trim).collect());
    }
    if s.contains('>') {
        return Some(s.split('>').map(str::trim).collect());
    }
    None
}

fn parses_as_greater_than_key(s: &str) -> bool {
    ShortcutTrigger::parse(s)
        .ok()
        .and_then(|trigger| trigger.as_key_binding().cloned())
        .is_some_and(|binding| binding.key == ">")
}

fn parse_keyboard_sequence(parts: &[&str]) -> Result<Shortcut, String> {
    if parts.len() > MAX_SEQUENCE_STEPS {
        return Err(format!(
            "Keyboard sequences can have at most {MAX_SEQUENCE_STEPS} steps."
        ));
    }
    let mut steps = Vec::with_capacity(parts.len());
    for part in parts {
        match ShortcutTrigger::parse(part)? {
            ShortcutTrigger::Keyboard(binding) => steps.push(binding),
            ShortcutTrigger::Pointer(_) | ShortcutTrigger::Stylus(_) => {
                return Err(format!(
                    "Sequences are keyboard-only; `{part}` is a device button."
                ));
            }
        }
    }
    Ok(Shortcut::Sequence(steps))
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

/// GTK/GDK button numbers.
///
/// X11-style buttons 8/9 are Back/Forward. GTK's Wayland backend then maps
/// evdev `BTN_FORWARD`/`BTN_BACK` to 10/11 and `BTN_TASK` through `BTN_EXTRA4`
/// to 12 through 15, matching [`linux::pointer_button`].
pub mod gdk {
    use super::{MAX_POINTER_EXTRA, PointerButton, StylusButton};

    pub fn pointer_button(button: u32) -> Option<PointerButton> {
        match button {
            8 | 11 => Some(PointerButton::Back),
            9 | 10 => Some(PointerButton::Forward),
            extra if (12..12 + u32::from(MAX_POINTER_EXTRA)).contains(&extra) => {
                Some(PointerButton::Extra((extra - 11) as u8))
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
            match &trigger {
                ShortcutTrigger::Stylus(_) => {
                    #[cfg(feature = "tablet-input")]
                    assert!(trigger.is_deliverable());
                    #[cfg(not(feature = "tablet-input"))]
                    assert!(!trigger.is_deliverable());
                }
                _ => assert!(trigger.is_deliverable()),
            }
        }
    }

    #[cfg(not(feature = "tablet-input"))]
    #[test]
    fn stylus_triggers_are_undeliverable_without_tablet_input() {
        let trigger = ShortcutTrigger::parse("StylusPrimary").unwrap();
        assert!(!trigger.is_deliverable());
        assert_eq!(trigger.unknown_key_name().as_deref(), Some("StylusPrimary"));
        assert!(
            ShortcutTrigger::parse("MouseBack")
                .unwrap()
                .is_deliverable()
        );
        assert!(!Shortcut::parse("StylusSecondary").unwrap().is_deliverable());
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
        assert_eq!(gdk::pointer_button(10), Some(PointerButton::Forward));
        assert_eq!(gdk::pointer_button(11), Some(PointerButton::Back));
        assert_eq!(gdk::pointer_button(12), Some(PointerButton::Extra(1)));
        assert_eq!(gdk::pointer_button(15), Some(PointerButton::Extra(4)));
        assert_eq!(gdk::pointer_button(16), None);
        assert_eq!(gdk::pointer_button(1), None);
        assert_eq!(
            linux::stylus_button(linux::BTN_STYLUS),
            Some(StylusButton::Primary)
        );
        assert_eq!(gdk::stylus_button(2), Some(StylusButton::Primary));
    }

    #[test]
    fn two_and_three_step_sequences_round_trip() {
        let two = Shortcut::parse("Ctrl+K > Ctrl+C").unwrap();
        assert_eq!(two.to_string(), "Ctrl+K > Ctrl+C");
        assert_eq!(two.display_label(), "Ctrl+K then Ctrl+C");
        assert_eq!(Shortcut::parse("Ctrl+K>Ctrl+C").unwrap(), two);

        let three = Shortcut::parse("Ctrl+K > Ctrl+C > Ctrl+V").unwrap();
        assert_eq!(three.to_string(), "Ctrl+K > Ctrl+C > Ctrl+V");
        assert_eq!(three.display_label(), "Ctrl+K then Ctrl+C then Ctrl+V");
    }

    #[test]
    fn single_chords_remain_byte_compatible() {
        let shortcut = Shortcut::parse("Ctrl+Shift+X").unwrap();
        assert_eq!(shortcut.to_string(), "Ctrl+Shift+X");
        assert!(matches!(shortcut, Shortcut::Single(_)));
        assert_eq!(
            shortcut,
            Shortcut::Single(ShortcutTrigger::parse("Ctrl+Shift+X").unwrap())
        );
    }

    #[test]
    fn empty_oversized_device_and_modifier_only_sequences_fail() {
        let empty = Shortcut::parse("Ctrl+K >").unwrap_err();
        assert!(empty.contains("Empty sequence step"), "{empty}");

        let oversized = Shortcut::parse("A > B > C > D").unwrap_err();
        assert!(oversized.contains("at most"), "{oversized}");

        let device = Shortcut::parse("Ctrl+K > MouseBack").unwrap_err();
        assert!(device.contains("keyboard-only"), "{device}");

        let modifiers = Shortcut::parse("Ctrl+K > Ctrl+Shift").unwrap_err();
        assert!(modifiers.contains("No key specified"), "{modifiers}");
    }

    #[test]
    fn greater_than_key_is_not_a_sequence_separator() {
        let chord = Shortcut::parse("Ctrl+>").unwrap();
        assert_eq!(chord.to_string(), "Ctrl+>");
        assert!(matches!(chord, Shortcut::Single(_)));
    }

    #[test]
    fn prefix_conflicts_are_detected_and_branching_sequences_are_not() {
        let prefix = Shortcut::parse("Ctrl+K").unwrap();
        let sequence = Shortcut::parse("Ctrl+K > Ctrl+C").unwrap();
        let longer = Shortcut::parse("Ctrl+K > Ctrl+C > Ctrl+V").unwrap();
        let branch = Shortcut::parse("Ctrl+K > Ctrl+X").unwrap();

        assert!(prefix.prefix_conflicts_with(&sequence));
        assert!(sequence.prefix_conflicts_with(&prefix));
        assert!(sequence.prefix_conflicts_with(&longer));
        assert!(!sequence.prefix_conflicts_with(&branch));
        assert!(!prefix.prefix_conflicts_with(&prefix));
        assert!(
            !Shortcut::parse("MouseBack")
                .unwrap()
                .prefix_conflicts_with(&sequence)
        );
    }
}
