use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::keybindings::defaults::*;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PresetKeybindingsConfig {
    #[serde(default = "default_apply_preset_1")]
    pub apply_preset_1: Vec<String>,

    #[serde(default = "default_apply_preset_2")]
    pub apply_preset_2: Vec<String>,

    #[serde(default = "default_apply_preset_3")]
    pub apply_preset_3: Vec<String>,

    #[serde(default = "default_apply_preset_4")]
    pub apply_preset_4: Vec<String>,

    #[serde(default = "default_apply_preset_5")]
    pub apply_preset_5: Vec<String>,

    #[serde(default = "default_save_preset_1")]
    pub save_preset_1: Vec<String>,

    #[serde(default = "default_save_preset_2")]
    pub save_preset_2: Vec<String>,

    #[serde(default = "default_save_preset_3")]
    pub save_preset_3: Vec<String>,

    #[serde(default = "default_save_preset_4")]
    pub save_preset_4: Vec<String>,

    #[serde(default = "default_save_preset_5")]
    pub save_preset_5: Vec<String>,

    #[serde(default = "default_clear_preset_1")]
    pub clear_preset_1: Vec<String>,

    #[serde(default = "default_clear_preset_2")]
    pub clear_preset_2: Vec<String>,

    #[serde(default = "default_clear_preset_3")]
    pub clear_preset_3: Vec<String>,

    #[serde(default = "default_clear_preset_4")]
    pub clear_preset_4: Vec<String>,

    #[serde(default = "default_clear_preset_5")]
    pub clear_preset_5: Vec<String>,
}

impl Default for PresetKeybindingsConfig {
    fn default() -> Self {
        Self {
            apply_preset_1: default_apply_preset_1(),
            apply_preset_2: default_apply_preset_2(),
            apply_preset_3: default_apply_preset_3(),
            apply_preset_4: default_apply_preset_4(),
            apply_preset_5: default_apply_preset_5(),
            save_preset_1: default_save_preset_1(),
            save_preset_2: default_save_preset_2(),
            save_preset_3: default_save_preset_3(),
            save_preset_4: default_save_preset_4(),
            save_preset_5: default_save_preset_5(),
            clear_preset_1: default_clear_preset_1(),
            clear_preset_2: default_clear_preset_2(),
            clear_preset_3: default_clear_preset_3(),
            clear_preset_4: default_clear_preset_4(),
            clear_preset_5: default_clear_preset_5(),
        }
    }
}
