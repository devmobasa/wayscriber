use serde::{Deserialize, Serialize};

/// Display state of the top toolbar strip.
///
/// `Full` and `Micro` are persisted forms of the strip; `Hidden` is the
/// runtime-only state the cycle action (`cycle_toolbar_display`, default
/// F2) reaches between `Micro` and `Full`. Hidden is never written back to
/// config — startup visibility is governed by `top_pinned`, the same pins
/// the plain visibility toggle (F9) records durably.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TopDisplayMode {
    /// The regular pill-island strip.
    #[default]
    Full,
    /// One 44px round chip: active tool glyph inside a ring stroked in the
    /// current color (ring width follows stroke thickness).
    Micro,
    /// The strip is not shown (runtime-only; not persisted).
    Hidden,
}

impl TopDisplayMode {
    /// The form persisted to config: `Hidden` collapses to `Full` so a
    /// cycle-hidden strip comes back visible on the next start (visibility
    /// at startup stays `top_pinned`'s job).
    pub fn persisted(self) -> Self {
        match self {
            Self::Hidden => Self::Full,
            other => other,
        }
    }
}

/// How the style pill exposes pen smoothing and Shape Pen recognition
/// sensitivity while a tool that uses them is active.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ToolbarStrokeControls {
    /// One compact "Pen feel" chip that opens a panel with level meters, a
    /// live smoothing preview, and a hint for each level.
    #[default]
    Panel,
    /// Inline level meters: one bar per level, clicked or scrolled.
    #[serde(alias = "meters")]
    Meter,
    /// Inline −/value/+ steppers.
    #[serde(alias = "steppers")]
    Stepper,
}

impl ToolbarStrokeControls {
    /// Every style, in the order the configurator lists them.
    pub const ALL: [Self; 3] = [Self::Panel, Self::Meter, Self::Stepper];
}

/// When the bottom-right zoom chip is shown (while `show_zoom_actions` is
/// on and the persisted master preference has not been hidden via
/// `Action::ToggleZoomChip`).
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ZoomChipDisplay {
    /// Persistent fixed-corner control (the default): the chip is also the
    /// mouse entry point for zooming in from 100%.
    #[default]
    Always,
    /// Show the chip only while zoom is active. At 100% the corner stays
    /// clean; zooming still works via keyboard/scroll bindings, and the
    /// chip appears as soon as zoom engages.
    WhileZoomed,
}
