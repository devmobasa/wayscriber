use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::input::state::{PressureThicknessEditMode, PressureThicknessEntryMode};

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

    /// Threshold (in pixels) before saving a stroke as pressure-sensitive.
    #[serde(default = "default_pressure_variation_threshold")]
    pub pressure_variation_threshold: f64,

    /// How thickness edits apply to pressure-sensitive strokes.
    #[serde(default)]
    pub pressure_thickness_edit_mode: PressureThicknessEditMode,

    /// When to show a thickness entry for pressure-sensitive selections.
    #[serde(default)]
    pub pressure_thickness_entry_mode: PressureThicknessEntryMode,

    /// Per-step scale factor when using pressure thickness scale mode.
    #[serde(default = "default_pressure_thickness_scale_step")]
    pub pressure_thickness_scale_step: f64,
}

impl Default for TabletInputConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            pressure_enabled: true,
            min_thickness: 1.0,
            max_thickness: 8.0,
            auto_eraser_switch: true,
            pressure_variation_threshold: 0.1,
            pressure_thickness_edit_mode: PressureThicknessEditMode::Disabled,
            pressure_thickness_entry_mode: PressureThicknessEntryMode::PressureOnly,
            pressure_thickness_scale_step: 0.1,
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

fn default_pressure_variation_threshold() -> f64 {
    0.1
}

fn default_pressure_thickness_scale_step() -> f64 {
    0.1
}
