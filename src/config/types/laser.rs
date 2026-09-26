use serde::{Deserialize, Serialize};

/// Narrowest laser core accepted from config, in pixels.
pub const LASER_WIDTH_MIN: f64 = 2.0;
/// Widest laser core accepted from config, in pixels.
pub const LASER_WIDTH_MAX: f64 = 30.0;
/// Longest time finished laser ink may stay fully visible, in milliseconds.
pub const LASER_HOLD_MS_MAX: u64 = 30_000;
/// Longest laser fade-out accepted from config, in milliseconds.
pub const LASER_FADE_MS_MAX: u64 = 5_000;

/// Laser pointer tool settings.
///
/// Laser ink is presenter feedback, not a drawing: it glows while you draw,
/// stays for `hold_ms` after the last stroke is released, then fades over
/// `fade_ms`. Strokes drawn before the ink has faded keep the whole group on
/// screen, so a gesture made of several strokes disappears together.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaserConfig {
    /// Ink color as `[red, green, blue, alpha]`, each 0.0 - 1.0. Independent
    /// of the pen color.
    #[serde(default = "default_laser_color")]
    pub color: [f64; 4],

    /// Width of the bright core in pixels (valid range: 2.0 - 30.0). The glow
    /// around it is about three times as wide.
    #[serde(default = "default_laser_width")]
    pub width: f64,

    /// How long finished ink stays fully visible after the last stroke is
    /// released, in milliseconds (valid range: 0 - 30000).
    #[serde(default = "default_laser_hold_ms")]
    pub hold_ms: u64,

    /// How long the ink takes to fade out once the hold ends, in milliseconds
    /// (valid range: 0 - 5000). 0 removes it at once.
    #[serde(default = "default_laser_fade_ms")]
    pub fade_ms: u64,
}

impl Default for LaserConfig {
    fn default() -> Self {
        Self {
            color: default_laser_color(),
            width: default_laser_width(),
            hold_ms: default_laser_hold_ms(),
            fade_ms: default_laser_fade_ms(),
        }
    }
}

fn default_laser_color() -> [f64; 4] {
    [1.0, 0.16, 0.12, 1.0]
}

fn default_laser_width() -> f64 {
    6.0
}

fn default_laser_hold_ms() -> u64 {
    1200
}

fn default_laser_fade_ms() -> u64 {
    500
}
