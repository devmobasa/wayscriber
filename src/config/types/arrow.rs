use serde::{Deserialize, Serialize};

use crate::draw::ArrowStyle;

/// Smallest arrowhead length in pixels accepted from config, presets, and sessions.
pub const ARROW_LENGTH_MIN: f64 = 5.0;
/// Largest arrowhead length in pixels accepted from config, presets, and sessions.
pub const ARROW_LENGTH_MAX: f64 = 50.0;
/// Narrowest arrowhead angle in degrees accepted from config, presets, and sessions.
pub const ARROW_ANGLE_MIN: f64 = 15.0;
/// Widest arrowhead angle in degrees accepted from config, presets, and sessions.
pub const ARROW_ANGLE_MAX: f64 = 60.0;

/// Arrow drawing settings.
///
/// Controls the appearance of arrowheads when using the arrow tool.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrowConfig {
    /// Arrowhead length in pixels (valid range: 5.0 - 50.0)
    #[serde(default = "default_arrow_length")]
    pub length: f64,

    /// Arrowhead angle in degrees (valid range: 15.0 - 60.0)
    /// Smaller angles create narrower arrowheads, larger angles create wider ones
    #[serde(default = "default_arrow_angle")]
    pub angle_degrees: f64,

    /// Place the arrowhead at the end of the line instead of the start
    #[serde(default = "default_arrow_head_at_end")]
    pub head_at_end: bool,

    /// Shape of the arrow drawn by the arrow tool: `"standard"`, `"pointy"`,
    /// `"curved"`, or `"double"`. This is the startup default only — the style
    /// is a per-shape property, so cycling it at runtime restyles the selection
    /// or the next arrow without rewriting anything already drawn.
    #[serde(default)]
    pub style: ArrowStyle,
}

impl Default for ArrowConfig {
    fn default() -> Self {
        Self {
            length: default_arrow_length(),
            angle_degrees: default_arrow_angle(),
            head_at_end: default_arrow_head_at_end(),
            style: ArrowStyle::default(),
        }
    }
}

fn default_arrow_length() -> f64 {
    20.0
}

fn default_arrow_angle() -> f64 {
    // tan(26 deg) ~= 0.49, so the head is about as wide as it is long. Wider
    // than that and the barbs stop reading as part of one head and start to
    // flare off the sides of the shaft.
    26.0
}

fn default_arrow_head_at_end() -> bool {
    true
}
