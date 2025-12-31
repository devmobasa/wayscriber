use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Help overlay styling configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HelpOverlayStyle {
    /// Font size for help overlay text
    #[serde(default = "default_help_font_size")]
    pub font_size: f64,

    /// Font family for help overlay text (comma-separated fallback list)
    #[serde(default = "default_help_font_family")]
    pub font_family: String,

    /// Line height for help text
    #[serde(default = "default_help_line_height")]
    pub line_height: f64,

    /// Padding around help box
    #[serde(default = "default_help_padding")]
    pub padding: f64,

    /// Background color [R, G, B, A] (0.0-1.0 range)
    #[serde(default = "default_help_bg_color")]
    pub bg_color: [f64; 4],

    /// Border color [R, G, B, A] (0.0-1.0 range)
    #[serde(default = "default_help_border_color")]
    pub border_color: [f64; 4],

    /// Border line width
    #[serde(default = "default_help_border_width")]
    pub border_width: f64,

    /// Text color [R, G, B, A] (0.0-1.0 range)
    #[serde(default = "default_help_text_color")]
    pub text_color: [f64; 4],
}

impl Default for HelpOverlayStyle {
    fn default() -> Self {
        Self {
            font_size: default_help_font_size(),
            font_family: default_help_font_family(),
            line_height: default_help_line_height(),
            padding: default_help_padding(),
            bg_color: default_help_bg_color(),
            border_color: default_help_border_color(),
            border_width: default_help_border_width(),
            text_color: default_help_text_color(),
        }
    }
}

fn default_help_font_size() -> f64 {
    14.0
}

fn default_help_font_family() -> String {
    "Noto Sans, DejaVu Sans, Liberation Sans, Sans".to_string()
}

fn default_help_line_height() -> f64 {
    22.0
}

fn default_help_padding() -> f64 {
    32.0
}

fn default_help_bg_color() -> [f64; 4] {
    [0.09, 0.1, 0.13, 0.92]
}

fn default_help_border_color() -> [f64; 4] {
    [0.33, 0.39, 0.52, 0.88]
}

fn default_help_border_width() -> f64 {
    2.0
}

fn default_help_text_color() -> [f64; 4] {
    [0.95, 0.96, 0.98, 1.0]
}
