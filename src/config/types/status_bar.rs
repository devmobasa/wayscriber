use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Status bar styling configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StatusBarStyle {
    /// Font size for status bar text
    #[serde(default = "default_status_font_size")]
    pub font_size: f64,

    /// Padding around status bar text
    #[serde(default = "default_status_padding")]
    pub padding: f64,

    /// Background color [R, G, B, A] (0.0-1.0 range)
    #[serde(default = "default_status_bg_color")]
    pub bg_color: [f64; 4],

    /// Text color [R, G, B, A] (0.0-1.0 range)
    #[serde(default = "default_status_text_color")]
    pub text_color: [f64; 4],

    /// Color indicator dot radius
    #[serde(default = "default_status_dot_radius")]
    pub dot_radius: f64,
}

impl Default for StatusBarStyle {
    fn default() -> Self {
        Self {
            font_size: default_status_font_size(),
            padding: default_status_padding(),
            bg_color: default_status_bg_color(),
            text_color: default_status_text_color(),
            dot_radius: default_status_dot_radius(),
        }
    }
}

fn default_status_font_size() -> f64 {
    21.0 // 50% larger than previous 14.0
}

fn default_status_padding() -> f64 {
    15.0 // 50% larger than previous 10.0
}

fn default_status_bg_color() -> [f64; 4] {
    [0.0, 0.0, 0.0, 0.85] // More opaque (was 0.7) for better visibility
}

fn default_status_text_color() -> [f64; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn default_status_dot_radius() -> f64 {
    6.0 // 50% larger than previous 4.0
}
