use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::keybindings::defaults::*;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ZoomKeybindingsConfig {
    #[serde(default = "default_toggle_frozen_mode")]
    pub toggle_frozen_mode: Vec<String>,

    #[serde(default = "default_zoom_in")]
    pub zoom_in: Vec<String>,

    #[serde(default = "default_zoom_out")]
    pub zoom_out: Vec<String>,

    #[serde(default = "default_reset_zoom")]
    pub reset_zoom: Vec<String>,

    #[serde(default = "default_toggle_zoom_lock")]
    pub toggle_zoom_lock: Vec<String>,

    #[serde(default = "default_refresh_zoom_capture")]
    pub refresh_zoom_capture: Vec<String>,
}

impl Default for ZoomKeybindingsConfig {
    fn default() -> Self {
        Self {
            toggle_frozen_mode: default_toggle_frozen_mode(),
            zoom_in: default_zoom_in(),
            zoom_out: default_zoom_out(),
            reset_zoom: default_reset_zoom(),
            toggle_zoom_lock: default_toggle_zoom_lock(),
            refresh_zoom_capture: default_refresh_zoom_capture(),
        }
    }
}
