use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Tablet/stylus input configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TabletInputConfig {
    /// Enable tablet/stylus events at runtime (feature must be compiled in).
    #[serde(default = "default_tablet_enabled")]
    pub enabled: bool,

    /// Enable pressure-to-thickness mapping.
    #[serde(default = "default_tablet_pressure_enabled")]
    pub pressure_enabled: bool,

    /// Minimum thickness when pressure is near 0.
    #[serde(default = "default_tablet_min_thickness")]
    pub min_thickness: f64,

    /// Maximum thickness when pressure is 1.0.
    #[serde(default = "default_tablet_max_thickness")]
    pub max_thickness: f64,

    /// Automatically switch to eraser tool when physical eraser is detected.
    #[serde(default = "default_auto_eraser_switch")]
    pub auto_eraser_switch: bool,
}

impl Default for TabletInputConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            pressure_enabled: true,
            min_thickness: 1.0,
            max_thickness: 8.0,
            auto_eraser_switch: true,
        }
    }
}

fn default_tablet_enabled() -> bool {
    false
}

fn default_tablet_pressure_enabled() -> bool {
    true
}

fn default_tablet_min_thickness() -> f64 {
    1.0
}

fn default_tablet_max_thickness() -> f64 {
    8.0
}

fn default_auto_eraser_switch() -> bool {
    true
}
