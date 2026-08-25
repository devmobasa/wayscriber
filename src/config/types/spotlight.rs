use serde::{Deserialize, Serialize};

/// Spotlight tool settings.
///
/// A spotlight dims the whole overlay except the regions it covers, so attention
/// lands where the presenter points.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotlightConfig {
    /// How strongly the area outside every spotlight is dimmed
    /// (valid range: 0.1 - 0.95; higher is darker)
    #[serde(default = "default_spotlight_dim")]
    pub dim_opacity: f64,

    /// Fraction of each spotlight's radius spent fading out at the edge
    /// (valid range: 0.0 - 0.9). 0.0 gives a hard edge.
    #[serde(default = "default_spotlight_feather")]
    pub feather: f64,

    /// Magnification copied into newly drawn Spotlight shapes
    /// (valid range: 1.0 - 4.0). Existing shapes keep their own value.
    #[serde(default = "crate::draw::default_spotlight_magnification")]
    pub magnification: f64,
}

impl Default for SpotlightConfig {
    fn default() -> Self {
        Self {
            dim_opacity: default_spotlight_dim(),
            feather: default_spotlight_feather(),
            magnification: crate::draw::DEFAULT_SPOTLIGHT_MAGNIFICATION,
        }
    }
}

fn default_spotlight_dim() -> f64 {
    0.6
}

fn default_spotlight_feather() -> f64 {
    0.35
}
