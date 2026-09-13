use std::fmt;
use std::hash::{Hash, Hasher};

/// A single keybinding: a key character with optional modifiers.
#[derive(Debug, Clone, Eq)]
pub struct KeyBinding {
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Super / Meta / Windows (Wayland logo modifier). Displayed as `Super`.
    pub logo: bool,
}

/// Equality and hashing ignore the key name's case, exactly like [`Self::matches`]
/// and the config-file contract ("key names are case-insensitive"). Deriving
/// them over the authored spelling instead would let `["ctrl+z"]` and
/// `["Ctrl+Z"]` coexist as two map entries that no conflict check sees and
/// dispatch picks between nondeterministically. `key` still holds the authored
/// spelling so display keeps the user's casing.
impl PartialEq for KeyBinding {
    fn eq(&self, other: &Self) -> bool {
        self.key.eq_ignore_ascii_case(&other.key)
            && self.ctrl == other.ctrl
            && self.shift == other.shift
            && self.alt == other.alt
            && self.logo == other.logo
    }
}

impl Hash for KeyBinding {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.to_ascii_lowercase().hash(state);
        self.ctrl.hash(state);
        self.shift.hash(state);
        self.alt.hash(state);
        self.logo.hash(state);
    }
}

/// Key names the input layer can actually deliver.
///
/// Anything else can be typed into `config.toml` and will parse, but no key
/// event ever carries that name, so the binding can never fire. Kept next to
/// [`KeyBinding::parse`] and pinned against the input layer's own key-to-name
/// mapping by `key_to_action_label_only_produces_recognized_names`.
pub const NAMED_KEYS: &[&str] = &[
    "Escape",
    "Return",
    "Backspace",
    "Space",
    "Menu",
    "Delete",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
];

/// Named keys whose config spelling is replaced by a glyph or a short name in
/// every user-facing label, paired as (config name, display form).
///
/// Only the display side changes: config files, [`fmt::Display`], and anything
/// persisted keep the left column. Names absent here (`Space`, `Home`, `End`,
/// the function keys) already read well and stay as they are.
const KEY_DISPLAY_NAMES: &[(&str, &str)] = &[
    ("ArrowLeft", "←"),
    ("ArrowRight", "→"),
    ("ArrowUp", "↑"),
    ("ArrowDown", "↓"),
    ("Return", "Enter"),
    ("Escape", "Esc"),
    ("Backspace", "⌫"),
    ("Delete", "Del"),
    ("PageUp", "PgUp"),
    ("PageDown", "PgDn"),
];

/// How a key name is shown to the user.
///
/// Case-insensitive like every other key-name comparison, so a config that
/// spells `arrowleft` still displays `←`. Anything without a display form —
/// single characters, `+`, `Space`, the function keys — comes back unchanged.
pub fn key_display_name(key: &str) -> &str {
    KEY_DISPLAY_NAMES
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .map_or(key, |(_, display)| *display)
}

/// The config key names behind the display forms in `label`, or `None` when it
/// has none.
///
/// Search boxes see the label a surface renders, not the binding behind it, so
/// a query still has to reach the name the config file spells: typing
/// `arrowleft` (or `left`) must find the action shown as `←`. Rewriting the
/// rendered label is what lets pre-formatted label text — joined alternatives,
/// compacted ranges, `then` sequences — stay searchable without every surface
/// carrying a second string.
pub fn canonical_key_names(label: &str) -> Option<String> {
    let mut restored: Option<String> = None;
    for (name, display) in KEY_DISPLAY_NAMES {
        let current = restored.as_deref().unwrap_or(label);
        if let Some(next) = replace_key_token(current, display, name) {
            restored = Some(next);
        }
    }
    restored
}

/// Swap every whole-word `display` in `label` for `name`, or `None` when there
/// is none.
///
/// Whole-word because a short display form can sit inside a longer word the
/// label wrote itself: hand-written help text spells `Backspace/Delete`, and a
/// blind substring swap would turn the `Del` in it into `Deleteete`.
fn replace_key_token(label: &str, display: &str, name: &str) -> Option<String> {
    let mut restored = String::new();
    let mut rest = label;
    let mut replaced = false;
    while let Some(index) = rest.find(display) {
        let (before, at_match) = rest.split_at(index);
        let after = &at_match[display.len()..];
        let stands_alone = !ends_in_word_char(before) && !starts_with_word_char(after);
        restored.push_str(before);
        if stands_alone {
            restored.push_str(name);
            replaced = true;
        } else {
            restored.push_str(display);
        }
        rest = after;
    }
    if !replaced {
        return None;
    }
    restored.push_str(rest);
    Some(restored)
}

fn ends_in_word_char(text: &str) -> bool {
    text.chars().next_back().is_some_and(char::is_alphanumeric)
}

fn starts_with_word_char(text: &str) -> bool {
    text.chars().next().is_some_and(char::is_alphanumeric)
}

/// Whether a key event can ever carry this name.
///
/// Single characters come through as themselves, so any one-character key is
/// deliverable; everything else has to be one of [`NAMED_KEYS`]. Matching is
/// case-insensitive because so is [`KeyBinding::matches`].
pub fn is_deliverable_key_name(key: &str) -> bool {
    key.chars().count() == 1
        || NAMED_KEYS
            .iter()
            .any(|known| known.eq_ignore_ascii_case(key))
}

/// A likely intended spelling for a key name no event can carry.
///
/// Covers the two mistakes that actually happen: a misspelled modifier, which
/// the parser folds into the key because it does not recognise it, and a
/// near-miss on a named key.
pub fn suggest_key_name(key: &str) -> Option<String> {
    if let Some((head, rest)) = key.split_once('+') {
        // The parser only leaves a `+` in the key when a segment was not a
        // modifier it knows, so the head is almost always a typo for one.
        let canonical = ["Ctrl", "Shift", "Alt", "Super"]
            .into_iter()
            .find(|modifier| within_one_edit(&head.to_lowercase(), &modifier.to_lowercase()))?;
        return Some(format!("{canonical}+{rest}"));
    }
    NAMED_KEYS
        .iter()
        .find(|known| within_one_edit(&key.to_lowercase(), &known.to_lowercase()))
        .map(|known| (*known).to_string())
}

/// Whether one string becomes the other with a single insertion, deletion,
/// substitution, or adjacent transposition — the shapes a typo takes.
fn within_one_edit(a: &str, b: &str) -> bool {
    if a == b {
        return false;
    }
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let mut ai = 0;
    let mut bi = 0;
    let mut edited = false;
    while ai < a.len() && bi < b.len() {
        if a[ai] == b[bi] {
            ai += 1;
            bi += 1;
            continue;
        }
        if edited {
            return false;
        }
        edited = true;
        if a.len() == b.len() {
            // Substitution, or a transposition of this pair.
            if ai + 1 < a.len() && a[ai] == b[bi + 1] && a[ai + 1] == b[bi] {
                ai += 2;
                bi += 2;
                continue;
            }
            ai += 1;
            bi += 1;
        } else if a.len() > b.len() {
            ai += 1;
        } else {
            bi += 1;
        }
    }
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChordParts {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub logo: bool,
    pub key: String,
}

pub(crate) fn parse_chord_parts(s: &str) -> Result<ChordParts, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Empty keybinding string".to_string());
    }

    // Normalize by removing spaces around '+'
    let s_normalized = s.replace(" + ", "+").replace("+ ", "+").replace(" +", "+");
    let parts: Vec<&str> = s_normalized.split('+').collect();
    if parts.is_empty() {
        return Err("Empty keybinding string".to_string());
    }

    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut logo = false;
    let mut key_parts = Vec::new();

    for part in parts {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" => alt = true,
            "super" | "meta" | "logo" | "win" | "windows" => logo = true,
            _ => key_parts.push(part),
        }
    }

    if key_parts.is_empty() {
        return Err(format!("No key specified in: {s}"));
    }

    let key = key_parts.join("+");
    Ok(ChordParts {
        ctrl,
        shift,
        alt,
        logo,
        key: if key.is_empty() { "+".to_string() } else { key },
    })
}

pub(crate) fn format_modifiers(
    ctrl: bool,
    shift: bool,
    alt: bool,
    logo: bool,
) -> Vec<&'static str> {
    let mut parts = Vec::new();
    if ctrl {
        parts.push("Ctrl");
    }
    if shift {
        parts.push("Shift");
    }
    if alt {
        parts.push("Alt");
    }
    if logo {
        parts.push("Super");
    }
    parts
}

impl KeyBinding {
    /// Parse a keybinding string like "Ctrl+Shift+W" or "Escape".
    /// Modifiers can appear in any order: "Shift+Ctrl+W", "Alt+Shift+Ctrl+W", etc.
    /// Supports spaces around '+' (e.g., "Ctrl + Shift + W")
    pub fn parse(s: &str) -> Result<Self, String> {
        let parts = parse_chord_parts(s)?;
        Ok(Self {
            key: parts.key,
            ctrl: parts.ctrl,
            shift: parts.shift,
            alt: parts.alt,
            logo: parts.logo,
        })
    }

    /// Label for chips, keycaps, menus, and help: modifiers as words, the key
    /// as its glyph or short name (`Ctrl+Alt+←`).
    ///
    /// [`fmt::Display`] stays the canonical config spelling, because that is
    /// what gets written back to `config.toml`.
    pub fn display_label(&self) -> String {
        format_modifiers(self.ctrl, self.shift, self.alt, self.logo)
            .into_iter()
            .chain(std::iter::once(key_display_name(&self.key)))
            .collect::<Vec<_>>()
            .join("+")
    }

    /// Check if this keybinding matches the current input state.
    pub fn matches(&self, key: &str, ctrl: bool, shift: bool, alt: bool, logo: bool) -> bool {
        self.key.eq_ignore_ascii_case(key)
            && self.ctrl == ctrl
            && self.shift == shift
            && self.alt == alt
            && self.logo == logo
    }
}

impl fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            format_modifiers(self.ctrl, self.shift, self.alt, self.logo)
                .into_iter()
                .chain(std::iter::once(self.key.as_str()))
                .collect::<Vec<_>>()
                .join("+")
        )
    }
}
