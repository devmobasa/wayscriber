use crate::config::enums::ColorSpec;
use crate::input::EraserMode;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Drawing-related settings.
///
/// Controls the default appearance of drawing tools when the overlay first opens.
/// Users can change these values at runtime using keybindings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DrawingConfig {
    /// Default pen color - either a named color (red, green, blue, yellow, orange, pink, white, black)
    /// or an RGB array like `[255, 0, 0]` for red
    #[serde(default = "default_color")]
    pub default_color: ColorSpec,

    /// Default pen thickness in pixels (valid range: 1.0 - 50.0)
    #[serde(default = "default_thickness")]
    pub default_thickness: f64,

    /// Default eraser size in pixels (valid range: 1.0 - 50.0)
    #[serde(default = "default_eraser_size")]
    pub default_eraser_size: f64,

    /// Default eraser behavior (brush or stroke)
    #[serde(default = "default_eraser_mode")]
    pub default_eraser_mode: EraserMode,

    /// Default marker opacity multiplier (0.05 - 0.9), applied to the current color alpha
    #[serde(default = "default_marker_opacity")]
    pub marker_opacity: f64,

    /// Whether shapes start filled when applicable
    #[serde(default = "default_fill_enabled")]
    pub default_fill_enabled: bool,

    /// Default font size for text mode in points (valid range: 8.0 - 72.0)
    #[serde(default = "default_font_size")]
    pub default_font_size: f64,

    /// Hit-test tolerance in pixels for selection (valid range: 1.0 - 20.0)
    #[serde(default = "default_hit_test_tolerance")]
    pub hit_test_tolerance: f64,

    /// Number of shapes processed linearly before enabling spatial index
    #[serde(default = "default_hit_test_threshold")]
    pub hit_test_linear_threshold: usize,

    /// Maximum undo actions retained (valid range: 10 - 1000)
    #[serde(default = "default_undo_stack_limit")]
    pub undo_stack_limit: usize,

    /// Font family name for text rendering (e.g., "Sans", "Monospace", "JetBrains Mono")
    /// Falls back to "Sans" if the specified font is not available
    /// Note: Install fonts system-wide and reference by family name
    #[serde(default = "default_font_family")]
    pub font_family: String,

    /// Font weight (e.g., "normal", "bold", "light", 400, 700)
    /// Can be a named weight or a numeric value (100-900)
    #[serde(default = "default_font_weight")]
    pub font_weight: String,

    /// Font style (e.g., "normal", "italic", "oblique")
    #[serde(default = "default_font_style")]
    pub font_style: String,

    /// Enable semi-transparent background box behind text for better contrast
    #[serde(default = "default_text_background")]
    pub text_background_enabled: bool,
}

impl Default for DrawingConfig {
    fn default() -> Self {
        Self {
            default_color: default_color(),
            default_thickness: default_thickness(),
            default_eraser_size: default_eraser_size(),
            default_eraser_mode: default_eraser_mode(),
            marker_opacity: default_marker_opacity(),
            default_fill_enabled: default_fill_enabled(),
            default_font_size: default_font_size(),
            hit_test_tolerance: default_hit_test_tolerance(),
            hit_test_linear_threshold: default_hit_test_threshold(),
            undo_stack_limit: default_undo_stack_limit(),
            font_family: default_font_family(),
            font_weight: default_font_weight(),
            font_style: default_font_style(),
            text_background_enabled: default_text_background(),
        }
    }
}

fn default_color() -> ColorSpec {
    ColorSpec::Name("red".to_string())
}

fn default_thickness() -> f64 {
    3.0
}

fn default_eraser_size() -> f64 {
    12.0
}

fn default_eraser_mode() -> EraserMode {
    EraserMode::Brush
}

fn default_marker_opacity() -> f64 {
    0.32
}

fn default_fill_enabled() -> bool {
    false
}

fn default_font_size() -> f64 {
    32.0
}

fn default_font_family() -> String {
    "Sans".to_string()
}

fn default_font_weight() -> String {
    "bold".to_string()
}

fn default_font_style() -> String {
    "normal".to_string()
}

fn default_text_background() -> bool {
    false
}

fn default_hit_test_tolerance() -> f64 {
    6.0
}

fn default_hit_test_threshold() -> usize {
    400
}

fn default_undo_stack_limit() -> usize {
    100
}
