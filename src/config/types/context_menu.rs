use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Context menu visibility configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
