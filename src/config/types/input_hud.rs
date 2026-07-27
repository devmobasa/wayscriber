use serde::{Deserialize, Serialize};

/// Input HUD configuration for the on-screen keystroke/click chip row.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputHudConfig {
    /// Whether the input HUD starts enabled
    #[serde(default = "default_input_hud_enabled")]
    pub enabled: bool,

    /// Where the HUD reads input from: system-wide capture, overlay-only, or
    /// automatic selection between the two
    #[serde(default)]
    pub mode: InputHudMode,

    /// Screen anchor for the chip row
    #[serde(default)]
    pub position: InputHudPosition,

    /// Show mouse buttons and scroll wheel events
    #[serde(default = "default_input_hud_show_mouse")]
    pub show_mouse: bool,

    /// Show taps of bare modifiers (Ctrl, Shift, Alt) as their own chips
    #[serde(default = "default_input_hud_show_bare_modifiers")]
    pub show_bare_modifiers: bool,

    /// How long a chip stays after its last press, before fading (ms)
    #[serde(default = "default_input_hud_display_ms")]
    pub display_ms: u64,

    /// Fade-out duration (ms)
    #[serde(default = "default_input_hud_fade_ms")]
    pub fade_ms: u64,

    /// Maximum number of simultaneous chips
    #[serde(default = "default_input_hud_max_entries")]
    pub max_entries: usize,

    /// Coalesce immediate repeats into a single chip with a xN counter
    #[serde(default = "default_input_hud_combine_repeats")]
    pub combine_repeats: bool,

    /// Chip label size in points
    #[serde(default = "default_input_hud_font_size")]
    pub font_size: f64,
}

impl Default for InputHudConfig {
    fn default() -> Self {
        Self {
            enabled: default_input_hud_enabled(),
            mode: InputHudMode::default(),
            position: InputHudPosition::default(),
            show_mouse: default_input_hud_show_mouse(),
            show_bare_modifiers: default_input_hud_show_bare_modifiers(),
            display_ms: default_input_hud_display_ms(),
            fade_ms: default_input_hud_fade_ms(),
            max_entries: default_input_hud_max_entries(),
            combine_repeats: default_input_hud_combine_repeats(),
            font_size: default_input_hud_font_size(),
        }
    }
}

/// Which input source feeds the HUD.
///
/// Overlay mode only ever reports the input wayscriber's own surfaces receive;
/// system mode reads `/dev/input` through libinput and reports every key and
/// button on the seat. System mode needs the `input-monitor` build feature and
/// read access to the evdev nodes (usually `input` group membership).
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum InputHudMode {
    /// Use system-wide capture when it is available, otherwise overlay-only.
    #[default]
    Auto,
    /// Never read `/dev/input`; report only what the overlay itself receives.
    Overlay,
    /// Require system-wide capture; fall back to overlay with a warning toast.
    System,
}

/// Screen anchor for the HUD chip row: a full three-by-three grid of edge and
/// center anchors.
///
/// The status HUD only supports the four corners, but the chip row reads best
/// centered, so it carries its own anchor set rather than widening
/// `StatusPosition`.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum InputHudPosition {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    #[default]
    BottomCenter,
    BottomRight,
}

impl InputHudPosition {
    /// Whether the row is anchored to the top edge of the screen.
    pub fn is_top(self) -> bool {
        matches!(self, Self::TopLeft | Self::TopCenter | Self::TopRight)
    }

    /// Whether the row is vertically centered (the middle grid line).
    pub fn is_middle(self) -> bool {
        matches!(self, Self::CenterLeft | Self::Center | Self::CenterRight)
    }

    /// Whether the row is horizontally centered.
    pub fn is_center(self) -> bool {
        matches!(self, Self::TopCenter | Self::Center | Self::BottomCenter)
    }

    /// Whether the row is anchored to the right edge of the screen.
    pub fn is_right(self) -> bool {
        matches!(self, Self::TopRight | Self::CenterRight | Self::BottomRight)
    }
}

fn default_input_hud_enabled() -> bool {
    false
}

fn default_input_hud_show_mouse() -> bool {
    true
}

fn default_input_hud_show_bare_modifiers() -> bool {
    true
}

fn default_input_hud_display_ms() -> u64 {
    1600
}

fn default_input_hud_fade_ms() -> u64 {
    350
}

fn default_input_hud_max_entries() -> usize {
    6
}

fn default_input_hud_combine_repeats() -> bool {
    true
}

fn default_input_hud_font_size() -> f64 {
    18.0
}
