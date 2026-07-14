use serde::{Deserialize, Serialize};

/// System tray preferences.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayConfig {
    /// Select whether the tray uses a theme-adaptive symbolic icon or colored pixmaps.
    #[serde(default)]
    pub icon_style: TrayIconStyle,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            icon_style: TrayIconStyle::Auto,
        }
    }
}

/// Rendering strategy for the main system tray icon.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrayIconStyle {
    /// Use a symbolic icon where supported and colored pixmaps as a compatibility fallback.
    #[default]
    Auto,
    /// Always request the theme-adaptive symbolic icon.
    Symbolic,
    /// Always publish the colored fallback pixmaps.
    Colored,
}
