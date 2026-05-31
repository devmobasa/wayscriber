use serde::{Deserialize, Serialize};

use crate::config::keybindings::defaults::*;

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureKeybindingsConfig {
    #[serde(default = "default_capture_full_screen")]
    pub capture_full_screen: Vec<String>,

    #[serde(default = "default_capture_active_window")]
    pub capture_active_window: Vec<String>,

    #[serde(default = "default_capture_selection")]
    pub capture_selection: Vec<String>,

    #[serde(default = "default_capture_clipboard_full")]
    pub capture_clipboard_full: Vec<String>,

    #[serde(default = "default_capture_file_full")]
    pub capture_file_full: Vec<String>,

    #[serde(default = "default_capture_clipboard_selection")]
    pub capture_clipboard_selection: Vec<String>,

    #[serde(default = "default_capture_file_selection")]
    pub capture_file_selection: Vec<String>,

    #[serde(default = "default_capture_clipboard_region")]
    pub capture_clipboard_region: Vec<String>,

    #[serde(default = "default_capture_file_region")]
    pub capture_file_region: Vec<String>,

    #[serde(default = "default_export_canvas_file")]
    pub export_canvas_file: Vec<String>,

    #[serde(default = "default_export_canvas_clipboard")]
    pub export_canvas_clipboard: Vec<String>,

    #[serde(default = "default_export_canvas_clipboard_and_file")]
    pub export_canvas_clipboard_and_file: Vec<String>,

    #[serde(default = "default_export_board_pdf_file")]
    pub export_board_pdf_file: Vec<String>,

    #[serde(default = "default_open_capture_folder")]
    pub open_capture_folder: Vec<String>,
}

impl Default for CaptureKeybindingsConfig {
    fn default() -> Self {
        Self {
            capture_full_screen: default_capture_full_screen(),
            capture_active_window: default_capture_active_window(),
            capture_selection: default_capture_selection(),
            capture_clipboard_full: default_capture_clipboard_full(),
            capture_file_full: default_capture_file_full(),
            capture_clipboard_selection: default_capture_clipboard_selection(),
            capture_file_selection: default_capture_file_selection(),
            capture_clipboard_region: default_capture_clipboard_region(),
            capture_file_region: default_capture_file_region(),
            export_canvas_file: default_export_canvas_file(),
            export_canvas_clipboard: default_export_canvas_clipboard(),
            export_canvas_clipboard_and_file: default_export_canvas_clipboard_and_file(),
            export_board_pdf_file: default_export_board_pdf_file(),
            open_capture_folder: default_open_capture_folder(),
        }
    }
}
