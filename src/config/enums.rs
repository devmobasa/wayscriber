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

/// Accent color of the overlay chrome (`[ui] accent_color`).
///
/// `"system"` (the default) follows the desktop accent color via the
/// settings portal and falls back to the built-in blue when no portal or
/// preference exists. `"default"` pins the built-in blue. Anything else is
/// a fixed custom accent: a `#RRGGBB` hex string or a palette color name
/// (the [`ColorSpec`] name set).
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct AccentColor(String);

/// What an [`AccentColor`] string resolves to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccentColorMode {
    /// Follow the desktop accent from the settings portal.
    System,
    /// The built-in accent.
    Default,
    /// A fixed custom accent.
    Custom(Color),
}

impl Default for AccentColor {
    fn default() -> Self {
        Self("system".to_string())
    }
}

impl AccentColor {
    /// Builds the setting from any accepted string form; validation happens
    /// in [`AccentColor::try_mode`], while [`AccentColor::mode`] provides the
    /// runtime fallback.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The configured string as written (for editors like the configurator).
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Resolves the configured string, reporting an unrecognized value as an
    /// error message. Editors (the configurator) surface the error to the
    /// user; the runtime goes through [`AccentColor::mode`] instead.
    pub fn try_mode(&self) -> Result<AccentColorMode, String> {
        let raw = self.0.trim();
        if raw.eq_ignore_ascii_case("system") {
            return Ok(AccentColorMode::System);
        }
        if raw.eq_ignore_ascii_case("default") {
            return Ok(AccentColorMode::Default);
        }
        match crate::util::parse_config_hex_color(raw) {
            Ok(color) => Ok(AccentColorMode::Custom(color)),
            Err(ConfigHexColorError::MissingHash) => match crate::util::name_to_color(raw) {
                Some(color) => Ok(AccentColorMode::Custom(color)),
                None => Err(format!(
                    "Unknown accent color '{raw}': use \"system\", \"default\", \
                     a #RRGGBB hex color, or a palette color name"
                )),
            },
            Err(err) => Err(format!(
                "Invalid accent hex color '{raw}' ({err:?}): use \"system\", \
                 \"default\", a #RRGGBB hex color, or a palette color name"
            )),
        }
    }

    /// Resolves the configured string. Unrecognized values warn and follow
    /// the system accent (the default), mirroring [`ColorSpec::to_color`]'s
    /// warn-and-fall-back contract.
    pub fn mode(&self) -> AccentColorMode {
        match self.try_mode() {
            Ok(mode) => mode,
            Err(message) => {
                warn!("{message}; following the system accent");
                AccentColorMode::System
            }
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

/// Color specification - either a named color, a `#RRGGBB` / `#RRGGBBAA` hex
/// string, or RGB(A) values.
///
/// # Examples
/// ```toml
/// # Named color
/// default_color = "red"
///
/// # Hex color
/// default_color = "#FFB3BA"
///
/// # Hex color with alpha
/// default_color = "#FFB3BA80"
///
/// # Custom RGB color (0-255 per component)
/// default_color = [255, 128, 0]  # Orange
///
/// # Custom RGBA color (0-255 per component)
/// default_color = [255, 128, 0, 128]  # Half-transparent orange
/// ```
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum ColorSpec {
    /// Named color, or `#RRGGBB` / `#RRGGBBAA` hex color
    Name(String),
    /// RGB color as [red, green, blue] where each component is 0-255
    Rgb([u8; 3]),
    /// RGBA color as [red, green, blue, alpha] where each component is 0-255.
    ///
    /// Only written for colors that are actually translucent, so an opaque
    /// palette serializes exactly as it did before alpha existed.
    Rgba([u8; 4]),
}

impl ColorSpec {
    /// Converts the color specification to a [`Color`] struct.
    ///
    /// Hex colors accept `#RRGGBB`, or `#RRGGBBAA` to carry alpha. Named colors are
    /// mapped to the tuned palette values using `util::name_to_color()`. Unknown color
    /// names and invalid hex values default to the tuned palette red with a warning.
    /// Three-component RGB arrays are converted from 0-255 range to 0.0-1.0 range with
    /// full opacity; four-component arrays take their alpha from the fourth.
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
            ColorSpec::Rgba([r, g, b, a]) => Color {
                r: *r as f64 / 255.0,
                g: *g as f64 / 255.0,
                b: *b as f64 / 255.0,
                a: *a as f64 / 255.0,
            },
        }
    }
}

impl From<Color> for ColorSpec {
    /// Opaque colors keep the three-component form they have always had, so
    /// adding alpha rewrites nobody's config file. Only a genuinely translucent
    /// color produces the four-component form.
    fn from(color: Color) -> Self {
        let clamp = |v: f64| -> u8 { (v.clamp(0.0, 1.0) * 255.0).round().min(255.0) as u8 };
        let (r, g, b) = (clamp(color.r), clamp(color.g), clamp(color.b));
        let alpha = clamp(color.a);
        if alpha == u8::MAX {
            ColorSpec::Rgb([r, g, b])
        } else {
            ColorSpec::Rgba([r, g, b, alpha])
        }
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

    #[test]
    fn accent_color_defaults_to_system() {
        assert_eq!(AccentColor::default().mode(), AccentColorMode::System);
    }

    #[test]
    fn accent_color_keywords_are_case_insensitive_and_trimmed() {
        assert_eq!(AccentColor::new(" System ").mode(), AccentColorMode::System);
        assert_eq!(AccentColor::new("DEFAULT").mode(), AccentColorMode::Default);
    }

    #[test]
    fn accent_color_accepts_hex_and_palette_names() {
        assert_eq!(
            AccentColor::new("#FF7800").mode(),
            AccentColorMode::Custom(Color {
                r: 1.0,
                g: 120.0 / 255.0,
                b: 0.0,
                a: 1.0,
            })
        );
        assert_eq!(
            AccentColor::new("orange").mode(),
            AccentColorMode::Custom(PALETTE_ORANGE)
        );
    }

    #[test]
    fn accent_color_falls_back_to_system_for_unrecognized_values() {
        for value in ["chartreuse", "#12345", "#GG0000"] {
            assert_eq!(
                AccentColor::new(value).mode(),
                AccentColorMode::System,
                "{value} should fall back to the system accent"
            );
        }
    }

    #[test]
    fn accent_color_round_trips_through_toml_as_a_plain_string() {
        #[derive(Serialize, Deserialize)]
        struct Wrapper {
            accent_color: AccentColor,
        }
        let parsed: Wrapper = toml::from_str("accent_color = \"#3584E4\"")
            .expect("the fixture TOML above is a valid accent_color assignment");
        assert!(matches!(
            parsed.accent_color.mode(),
            AccentColorMode::Custom(_)
        ));
        let serialized = toml::to_string(&parsed)
            .expect("the fixture wrapper holds only a string-backed accent color");
        assert_eq!(serialized.trim(), "accent_color = \"#3584E4\"");
    }

    #[test]
    fn accent_color_try_mode_reports_unrecognized_values() {
        for value in ["chartreuse", "#12345", "#GG0000"] {
            let result = AccentColor::new(value).try_mode();
            assert!(
                result.is_err(),
                "{value} should be reported as invalid, got {result:?}"
            );
        }
        assert_eq!(
            AccentColor::new("system").try_mode(),
            Ok(AccentColorMode::System)
        );
    }
}
