use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::keybindings::defaults::*;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ColorKeybindingsConfig {
    #[serde(default = "default_set_color_red")]
    pub set_color_red: Vec<String>,

    #[serde(default = "default_set_color_green")]
    pub set_color_green: Vec<String>,

    #[serde(default = "default_set_color_blue")]
    pub set_color_blue: Vec<String>,

    #[serde(default = "default_set_color_yellow")]
    pub set_color_yellow: Vec<String>,

    #[serde(default = "default_set_color_orange")]
    pub set_color_orange: Vec<String>,

    #[serde(default = "default_set_color_pink")]
    pub set_color_pink: Vec<String>,

    #[serde(default = "default_set_color_white")]
    pub set_color_white: Vec<String>,

    #[serde(default = "default_set_color_black")]
    pub set_color_black: Vec<String>,
}

impl Default for ColorKeybindingsConfig {
    fn default() -> Self {
        Self {
            set_color_red: default_set_color_red(),
            set_color_green: default_set_color_green(),
            set_color_blue: default_set_color_blue(),
            set_color_yellow: default_set_color_yellow(),
            set_color_orange: default_set_color_orange(),
            set_color_pink: default_set_color_pink(),
            set_color_white: default_set_color_white(),
            set_color_black: default_set_color_black(),
        }
    }
}
