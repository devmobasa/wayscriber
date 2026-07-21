//! Configuration enum types.

use crate::domain::{Color, color::*};
use crate::util::ConfigHexColorError;
use log::warn;
use serde::{Deserialize, Serialize};

/// Status bar position on screen.
///
/// Controls where the status bar appears relative to screen edges.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
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

/// Mouse button used to toggle the radial menu.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RadialMenuMouseBinding {
    /// Toggle radial menu with middle click.
    Middle,
    /// Toggle radial menu with right click.
    Right,
    /// Disable mouse-button toggling (keyboard action only).
    Disabled,
}

/// Behavior when the GNOME/xdg fallback overlay loses keyboard focus.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum XdgFocusLossBehavior {
    /// Close the overlay when focus moves away (legacy/default behavior).
    #[default]
    Exit,
    /// Keep the overlay open after focus loss and let users reactivate it manually.
    Stay,
}

/// Overlay chrome theme (`[ui] theme`).
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum UiTheme {
    /// Follow context. Currently resolves to dark chrome; context-aware
    /// selection lands when surfaces consume the runtime theme.
    #[default]
    Auto,
    /// Always dark chrome.
    Dark,
    /// Always light chrome.
    Light,
}

impl UiTheme {
    /// Maps the config value onto the runtime theme mode.
    pub fn to_theme_mode(self) -> crate::ui::theme::ThemeMode {
        match self {
            UiTheme::Auto => crate::ui::theme::ThemeMode::Auto,
            UiTheme::Dark => crate::ui::theme::ThemeMode::Dark,
            UiTheme::Light => crate::ui::theme::ThemeMode::Light,
        }
    }
}

/// Reduced-motion preference (`[ui] reduced_motion`).
///
/// `on` disables UI animations. `auto` is reserved for a future desktop-portal
/// (system preference) query and currently behaves like `off` (full motion).
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ReducedMotion {
    /// Follow the system preference once desktop-portal support lands;
    /// full motion today.
    #[default]
    Auto,
    /// Reduce motion: disable UI animations.
    On,
    /// Full motion.
    Off,
}

impl ReducedMotion {
    /// Whether UI animations should run.
    pub fn motion_enabled(self) -> bool {
        !matches!(self, ReducedMotion::On)
    }
}

/// Color specification - either a named color, `#RRGGBB` hex string, or RGB values.
///
/// # Examples
/// ```toml
/// # Named color
/// default_color = "red"
///
/// # Hex color
/// default_color = "#FFB3BA"
///
/// # Custom RGB color (0-255 per component)
/// default_color = [255, 128, 0]  # Orange
/// ```
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum ColorSpec {
    /// Named color, or `#RRGGBB` hex color
    Name(String),
    /// RGB color as [red, green, blue] where each component is 0-255
    Rgb([u8; 3]),
}

impl ColorSpec {
    /// Converts the color specification to a [`Color`] struct.
    ///
    /// Hex colors accept only `#RRGGBB`. Named colors are mapped to the tuned palette
    /// values using `util::name_to_color()`. Unknown color names and invalid hex values
    /// default to the tuned palette red with a warning. RGB arrays are converted from
    /// 0-255 range to 0.0-1.0 range with full opacity.
    pub fn to_color(&self) -> Color {
        match self {
            ColorSpec::Name(name) => match crate::util::parse_config_hex_color(name) {
                Ok(color) => color,
                Err(ConfigHexColorError::MissingHash) => crate::util::name_to_color(name)
                    .unwrap_or_else(|| {
                        warn!("Unknown color '{}', using red", name);
                        PALETTE_RED
                    }),
                Err(err) => {
                    warn!("Invalid hex color '{}': {:?}; using red", name, err);
                    PALETTE_RED
                }
            },
            ColorSpec::Rgb([r, g, b]) => Color {
                r: *r as f64 / 255.0,
                g: *g as f64 / 255.0,
                b: *b as f64 / 255.0,
                a: 1.0,
            },
        }
    }
}

impl From<Color> for ColorSpec {
    fn from(color: Color) -> Self {
        let clamp = |v: f64| -> u8 { (v.clamp(0.0, 1.0) * 255.0).round().min(255.0) as u8 };
        ColorSpec::Rgb([clamp(color.r), clamp(color.g), clamp(color.b)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_spec_from_color_clamps_components() {
        let spec = ColorSpec::from(Color {
            r: 1.2,
            g: -0.1,
            b: 0.5,
            a: 1.0,
        });
        match spec {
            ColorSpec::Rgb([r, g, b]) => {
                assert_eq!(r, 255);
                assert_eq!(g, 0);
                assert_eq!(b, 128);
            }
            _ => panic!("expected rgb variant"),
        }
    }

    #[test]
    fn color_spec_to_color_falls_back_to_red_for_unknown_name() {
        let spec = ColorSpec::Name("chartreuse".to_string());
        let color = spec.to_color();
        assert_eq!(color, PALETTE_RED);
    }

    #[test]
    fn color_spec_to_color_accepts_hash_rrggbb_hex() {
        let spec = ColorSpec::Name("#FFB3BA".to_string());
        let color = spec.to_color();
        assert_eq!(
            color,
            Color {
                r: 1.0,
                g: 179.0 / 255.0,
                b: 186.0 / 255.0,
                a: 1.0,
            }
        );
    }

    #[test]
    fn color_spec_to_color_falls_back_to_red_for_invalid_hex() {
        for value in ["#GG0000", "#12345", "0xFFB3BA"] {
            let spec = ColorSpec::Name(value.to_string());
            let color = spec.to_color();
            assert_eq!(color, PALETTE_RED, "{value} should fall back to red");
        }
    }

    #[test]
    fn color_spec_from_color_rounds_components() {
        let spec = ColorSpec::from(Color {
            r: 0.0,
            g: 0.5,
            b: 0.499,
            a: 1.0,
        });
        match spec {
            ColorSpec::Rgb([r, g, b]) => {
                assert_eq!(r, 0);
                assert_eq!(g, 128);
                assert_eq!(b, 127);
            }
            _ => panic!("expected rgb variant"),
        }
    }
}
