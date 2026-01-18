use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::keybindings::defaults::*;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UiKeybindingsConfig {
    #[serde(default = "default_toggle_help")]
    pub toggle_help: Vec<String>,

    #[serde(default = "default_toggle_quick_help")]
    pub toggle_quick_help: Vec<String>,

    #[serde(default = "default_toggle_status_bar")]
    pub toggle_status_bar: Vec<String>,

    #[serde(default = "default_toggle_click_highlight")]
    pub toggle_click_highlight: Vec<String>,

    #[serde(default = "default_toggle_toolbar")]
    pub toggle_toolbar: Vec<String>,

    #[serde(default = "default_toggle_presenter_mode")]
    pub toggle_presenter_mode: Vec<String>,

    #[serde(default = "default_toggle_fill")]
    pub toggle_fill: Vec<String>,

    #[serde(default = "default_toggle_selection_properties")]
    pub toggle_selection_properties: Vec<String>,

    #[serde(default = "default_open_context_menu")]
    pub open_context_menu: Vec<String>,

    #[serde(default = "default_open_configurator")]
    pub open_configurator: Vec<String>,

    #[serde(default = "default_toggle_command_palette")]
    pub toggle_command_palette: Vec<String>,
}

impl Default for UiKeybindingsConfig {
    fn default() -> Self {
        Self {
            toggle_help: default_toggle_help(),
            toggle_quick_help: default_toggle_quick_help(),
            toggle_status_bar: default_toggle_status_bar(),
            toggle_click_highlight: default_toggle_click_highlight(),
            toggle_toolbar: default_toggle_toolbar(),
            toggle_presenter_mode: default_toggle_presenter_mode(),
            toggle_fill: default_toggle_fill(),
            toggle_selection_properties: default_toggle_selection_properties(),
            open_context_menu: default_open_context_menu(),
            open_configurator: default_open_configurator(),
            toggle_command_palette: default_toggle_command_palette(),
        }
    }
}
