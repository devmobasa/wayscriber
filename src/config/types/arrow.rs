use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Arrow drawing settings.
///
/// Controls the appearance of arrowheads when using the arrow tool (Ctrl+Shift+Drag).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
}

impl Default for ArrowConfig {
    fn default() -> Self {
        Self {
            length: default_arrow_length(),
            angle_degrees: default_arrow_angle(),
            head_at_end: default_arrow_head_at_end(),
        }
    }
}

fn default_arrow_length() -> f64 {
    20.0
}

fn default_arrow_angle() -> f64 {
    30.0
}

fn default_arrow_head_at_end() -> bool {
    false
}
