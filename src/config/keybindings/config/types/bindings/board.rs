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

    #[serde(default = "default_board_1")]
    pub board_1: Vec<String>,

    #[serde(default = "default_board_2")]
    pub board_2: Vec<String>,

    #[serde(default = "default_board_3")]
    pub board_3: Vec<String>,

    #[serde(default = "default_board_4")]
    pub board_4: Vec<String>,

    #[serde(default = "default_board_5")]
    pub board_5: Vec<String>,

    #[serde(default = "default_board_6")]
    pub board_6: Vec<String>,

    #[serde(default = "default_board_7")]
    pub board_7: Vec<String>,

    #[serde(default = "default_board_8")]
    pub board_8: Vec<String>,

    #[serde(default = "default_board_9")]
    pub board_9: Vec<String>,

    #[serde(default = "default_board_next")]
    pub board_next: Vec<String>,

    #[serde(default = "default_board_prev")]
    pub board_prev: Vec<String>,

    #[serde(default = "default_board_new")]
    pub board_new: Vec<String>,

    #[serde(default = "default_board_duplicate")]
    pub board_duplicate: Vec<String>,

    #[serde(default = "default_board_delete")]
    pub board_delete: Vec<String>,

    #[serde(default = "default_board_picker")]
    pub board_picker: Vec<String>,
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
            board_1: default_board_1(),
            board_2: default_board_2(),
            board_3: default_board_3(),
            board_4: default_board_4(),
            board_5: default_board_5(),
            board_6: default_board_6(),
            board_7: default_board_7(),
            board_8: default_board_8(),
            board_9: default_board_9(),
            board_next: default_board_next(),
            board_prev: default_board_prev(),
            board_new: default_board_new(),
            board_duplicate: default_board_duplicate(),
            board_delete: default_board_delete(),
            board_picker: default_board_picker(),
        }
    }
}
