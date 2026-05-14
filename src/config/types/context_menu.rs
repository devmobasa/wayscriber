use serde::{Deserialize, Serialize};

/// Context menu visibility configuration.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMenuUiConfig {
    #[serde(default = "default_context_menu_enabled")]
    pub enabled: bool,
}

impl Default for ContextMenuUiConfig {
    fn default() -> Self {
        Self {
            enabled: default_context_menu_enabled(),
        }
    }
}

fn default_context_menu_enabled() -> bool {
    true
}
