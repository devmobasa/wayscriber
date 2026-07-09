use serde::{Deserialize, Serialize};

pub const MIN_STROKE_THICKNESS: f64 = 1.0;
pub const MAX_STROKE_THICKNESS: f64 = 50.0;

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PressureThicknessEditMode {
    #[default]
    Disabled,
    Add,
    Scale,
}

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PressureThicknessEntryMode {
    Never,
    #[default]
    PressureOnly,
    AnyPressure,
}

pub(crate) fn default_step_marker_size(font_size: f64) -> f64 {
    (font_size * 0.6).clamp(12.0, 36.0)
}
