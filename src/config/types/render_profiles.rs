use serde::{Deserialize, Serialize};

/// Configurable final-render color profiles.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderProfilesConfig {
    /// Profile id to enable when the overlay starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,

    /// Apply the active profile to board backgrounds and annotation pixels.
    #[serde(default = "default_render_profile_target_enabled")]
    pub apply_to_canvas: bool,

    /// Apply the active profile to Wayscriber UI chrome, toolbars, popups, and status text.
    #[serde(default = "default_render_profile_target_enabled")]
    pub apply_to_ui: bool,

    /// Available render color profiles.
    #[serde(default)]
    pub items: Vec<RenderProfileConfig>,
}

impl Default for RenderProfilesConfig {
    fn default() -> Self {
        Self {
            active: None,
            apply_to_canvas: true,
            apply_to_ui: true,
            items: Vec::new(),
        }
    }
}

fn default_render_profile_target_enabled() -> bool {
    true
}

/// A named set of exact RGB color mappings applied to rendered pixels.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderProfileConfig {
    /// Stable profile id used by config and runtime switching.
    pub id: String,

    /// Human-friendly display name.
    pub name: String,

    /// Exact RGB mappings for this profile. Pixel alpha is preserved.
    #[serde(default)]
    pub mappings: Vec<RenderColorMappingConfig>,
}

/// One exact source-to-target RGB color mapping.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenderColorMappingConfig {
    /// Source color as #RRGGBB, RRGGBB, or 0xRRGGBB.
    pub from: String,

    /// Target color as #RRGGBB, RRGGBB, or 0xRRGGBB.
    pub to: String,
}
