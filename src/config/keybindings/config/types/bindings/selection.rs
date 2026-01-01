use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::keybindings::defaults::*;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SelectionKeybindingsConfig {
    #[serde(default = "default_duplicate_selection")]
    pub duplicate_selection: Vec<String>,

    #[serde(default = "default_copy_selection")]
    pub copy_selection: Vec<String>,

    #[serde(default = "default_paste_selection")]
    pub paste_selection: Vec<String>,

    #[serde(default = "default_select_all")]
    pub select_all: Vec<String>,

    #[serde(default = "default_move_selection_to_front")]
    pub move_selection_to_front: Vec<String>,

    #[serde(default = "default_move_selection_to_back")]
    pub move_selection_to_back: Vec<String>,

    #[serde(default = "default_nudge_selection_up")]
    pub nudge_selection_up: Vec<String>,

    #[serde(default = "default_nudge_selection_down")]
    pub nudge_selection_down: Vec<String>,

    #[serde(default = "default_nudge_selection_left")]
    pub nudge_selection_left: Vec<String>,

    #[serde(default = "default_nudge_selection_right")]
    pub nudge_selection_right: Vec<String>,

    #[serde(default = "default_nudge_selection_up_large")]
    pub nudge_selection_up_large: Vec<String>,

    #[serde(default = "default_nudge_selection_down_large")]
    pub nudge_selection_down_large: Vec<String>,

    #[serde(default = "default_move_selection_to_start")]
    pub move_selection_to_start: Vec<String>,

    #[serde(default = "default_move_selection_to_end")]
    pub move_selection_to_end: Vec<String>,

    #[serde(default = "default_move_selection_to_top")]
    pub move_selection_to_top: Vec<String>,

    #[serde(default = "default_move_selection_to_bottom")]
    pub move_selection_to_bottom: Vec<String>,

    #[serde(default = "default_delete_selection")]
    pub delete_selection: Vec<String>,
}

impl Default for SelectionKeybindingsConfig {
    fn default() -> Self {
        Self {
            duplicate_selection: default_duplicate_selection(),
            copy_selection: default_copy_selection(),
            paste_selection: default_paste_selection(),
            select_all: default_select_all(),
            move_selection_to_front: default_move_selection_to_front(),
            move_selection_to_back: default_move_selection_to_back(),
            nudge_selection_up: default_nudge_selection_up(),
            nudge_selection_down: default_nudge_selection_down(),
            nudge_selection_left: default_nudge_selection_left(),
            nudge_selection_right: default_nudge_selection_right(),
            nudge_selection_up_large: default_nudge_selection_up_large(),
            nudge_selection_down_large: default_nudge_selection_down_large(),
            move_selection_to_start: default_move_selection_to_start(),
            move_selection_to_end: default_move_selection_to_end(),
            move_selection_to_top: default_move_selection_to_top(),
            move_selection_to_bottom: default_move_selection_to_bottom(),
            delete_selection: default_delete_selection(),
        }
    }
}
