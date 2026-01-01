use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::keybindings::defaults::*;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoreKeybindingsConfig {
    #[serde(default = "default_exit")]
    pub exit: Vec<String>,

    #[serde(default = "default_enter_text_mode")]
    pub enter_text_mode: Vec<String>,

    #[serde(default = "default_enter_sticky_note_mode")]
    pub enter_sticky_note_mode: Vec<String>,

    #[serde(default = "default_clear_canvas")]
    pub clear_canvas: Vec<String>,

    #[serde(default = "default_undo")]
    pub undo: Vec<String>,

    #[serde(default = "default_redo")]
    pub redo: Vec<String>,

    #[serde(default)]
    pub undo_all: Vec<String>,

    #[serde(default)]
    pub redo_all: Vec<String>,

    #[serde(default)]
    pub undo_all_delayed: Vec<String>,

    #[serde(default)]
    pub redo_all_delayed: Vec<String>,
}

impl Default for CoreKeybindingsConfig {
    fn default() -> Self {
        Self {
            exit: default_exit(),
            enter_text_mode: default_enter_text_mode(),
            enter_sticky_note_mode: default_enter_sticky_note_mode(),
            clear_canvas: default_clear_canvas(),
            undo: default_undo(),
            redo: default_redo(),
            undo_all: Vec::new(),
            redo_all: Vec::new(),
            undo_all_delayed: Vec::new(),
            redo_all_delayed: Vec::new(),
        }
    }
}
