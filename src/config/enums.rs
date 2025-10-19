//! Configuration enum types.

use crate::draw::{Color, color::*};
use log::warn;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Status bar position on screen.
///
/// Controls where the status bar appears relative to screen edges.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum StatusPosition {
    /// Top-left corner
    TopLeft,
    /// Top-right corner
    TopRight,
    /// Bottom-left corner
    BottomLeft,
    /// Bottom-right corner
    BottomRight,
}

/// Color specification - either a named color or RGB values.
///
/// # Examples
/// ```toml
/// # Named color
/// default_color = "red"
///
/// # Custom RGB color (0-255 per component)
/// default_color = [255, 128, 0]  # Orange
/// ```
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(untagged)]
pub enum ColorSpec {
    /// Named color: red, green, blue, yellow, orange, pink, white, black
    Name(String),
    /// RGB color as [red, green, blue] where each component is 0-255
    Rgb([u8; 3]),
}

impl ColorSpec {
    /// Converts the color specification to a [`Color`] struct.
    ///
    /// Named colors are mapped to predefined RGBA values using `util::name_to_color()`.
    /// Unknown color names default to red with a warning. RGB arrays are converted from
    /// 0-255 range to 0.0-1.0 range with full opacity.
    pub fn to_color(&self) -> Color {
        match self {
            ColorSpec::Name(name) => crate::util::name_to_color(name).unwrap_or_else(|| {
                warn!("Unknown color '{}', using red", name);
                RED
            }),
            ColorSpec::Rgb([r, g, b]) => Color {
                r: *r as f64 / 255.0,
                g: *g as f64 / 255.0,
                b: *b as f64 / 255.0,
                a: 1.0,
            },
        }
    }
}
