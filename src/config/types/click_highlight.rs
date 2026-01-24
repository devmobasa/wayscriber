use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Click highlight configuration for mouse press indicator.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClickHighlightConfig {
    /// Whether the highlight effect starts enabled
    #[serde(default = "default_click_highlight_enabled")]
    pub enabled: bool,

    /// Show a persistent ring while the highlight tool is active
    #[serde(default = "default_click_highlight_show_on_highlight_tool")]
    pub show_on_highlight_tool: bool,

    /// Radius of the highlight circle in pixels
    #[serde(default = "default_click_highlight_radius")]
    pub radius: f64,

    /// Outline thickness in pixels
    #[serde(default = "default_click_highlight_outline")]
    pub outline_thickness: f64,

    /// Lifetime of the highlight in milliseconds
    #[serde(default = "default_click_highlight_duration_ms")]
    pub duration_ms: u64,

    /// Fill color RGBA (0.0-1.0)
    #[serde(default = "default_click_highlight_fill_color")]
    pub fill_color: [f64; 4],

    /// Outline color RGBA (0.0-1.0)
    #[serde(default = "default_click_highlight_outline_color")]
    pub outline_color: [f64; 4],

    /// Derive highlight color from current pen color
    #[serde(default = "default_click_highlight_use_pen_color")]
    pub use_pen_color: bool,
}

impl Default for ClickHighlightConfig {
    fn default() -> Self {
        Self {
            enabled: default_click_highlight_enabled(),
            show_on_highlight_tool: default_click_highlight_show_on_highlight_tool(),
            radius: default_click_highlight_radius(),
            outline_thickness: default_click_highlight_outline(),
            duration_ms: default_click_highlight_duration_ms(),
            fill_color: default_click_highlight_fill_color(),
            outline_color: default_click_highlight_outline_color(),
            use_pen_color: default_click_highlight_use_pen_color(),
        }
    }
}

fn default_click_highlight_enabled() -> bool {
    false
}

fn default_click_highlight_show_on_highlight_tool() -> bool {
    false
}

fn default_click_highlight_radius() -> f64 {
    24.0
}

fn default_click_highlight_outline() -> f64 {
    4.0
}

fn default_click_highlight_duration_ms() -> u64 {
    750
}

fn default_click_highlight_fill_color() -> [f64; 4] {
    [1.0, 0.8, 0.0, 0.35]
}

fn default_click_highlight_outline_color() -> [f64; 4] {
    [1.0, 0.6, 0.0, 0.9]
}

fn default_click_highlight_use_pen_color() -> bool {
    true
}
