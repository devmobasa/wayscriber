use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::keybindings::defaults::*;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BoardKeybindingsConfig {
    #[serde(default = "default_toggle_whiteboard")]
    pub toggle_whiteboard: Vec<String>,

    #[serde(default = "default_toggle_blackboard")]
    pub toggle_blackboard: Vec<String>,

    #[serde(default = "default_return_to_transparent")]
    pub return_to_transparent: Vec<String>,

    #[serde(default = "default_page_prev")]
    pub page_prev: Vec<String>,

    #[serde(default = "default_page_next")]
    pub page_next: Vec<String>,

    #[serde(default = "default_page_new")]
    pub page_new: Vec<String>,

    #[serde(default = "default_page_duplicate")]
    pub page_duplicate: Vec<String>,

    #[serde(default = "default_page_delete")]
    pub page_delete: Vec<String>,
}

impl Default for BoardKeybindingsConfig {
    fn default() -> Self {
        Self {
            toggle_whiteboard: default_toggle_whiteboard(),
            toggle_blackboard: default_toggle_blackboard(),
            return_to_transparent: default_return_to_transparent(),
            page_prev: default_page_prev(),
            page_next: default_page_next(),
            page_new: default_page_new(),
            page_duplicate: default_page_duplicate(),
            page_delete: default_page_delete(),
        }
    }
}
