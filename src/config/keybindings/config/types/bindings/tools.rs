use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::keybindings::defaults::*;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolKeybindingsConfig {
    #[serde(default = "default_increase_thickness")]
    pub increase_thickness: Vec<String>,

    #[serde(default = "default_decrease_thickness")]
    pub decrease_thickness: Vec<String>,

    #[serde(default = "default_increase_marker_opacity")]
    pub increase_marker_opacity: Vec<String>,

    #[serde(default = "default_decrease_marker_opacity")]
    pub decrease_marker_opacity: Vec<String>,

    #[serde(default = "default_select_selection_tool")]
    pub select_selection_tool: Vec<String>,

    #[serde(default = "default_select_marker_tool")]
    pub select_marker_tool: Vec<String>,

    #[serde(default = "default_select_eraser_tool")]
    pub select_eraser_tool: Vec<String>,

    #[serde(default = "default_toggle_eraser_mode")]
    pub toggle_eraser_mode: Vec<String>,

    #[serde(default = "default_select_pen_tool")]
    pub select_pen_tool: Vec<String>,

    #[serde(default = "default_select_line_tool")]
    pub select_line_tool: Vec<String>,

    #[serde(default = "default_select_rect_tool")]
    pub select_rect_tool: Vec<String>,

    #[serde(default = "default_select_ellipse_tool")]
    pub select_ellipse_tool: Vec<String>,

    #[serde(default = "default_select_arrow_tool")]
    pub select_arrow_tool: Vec<String>,

    #[serde(default = "default_select_highlight_tool")]
    pub select_highlight_tool: Vec<String>,

    #[serde(default = "default_toggle_highlight_tool")]
    pub toggle_highlight_tool: Vec<String>,

    #[serde(default = "default_increase_font_size")]
    pub increase_font_size: Vec<String>,

    #[serde(default = "default_decrease_font_size")]
    pub decrease_font_size: Vec<String>,

    #[serde(default = "default_reset_arrow_labels")]
    pub reset_arrow_labels: Vec<String>,
}

impl Default for ToolKeybindingsConfig {
    fn default() -> Self {
        Self {
            increase_thickness: default_increase_thickness(),
            decrease_thickness: default_decrease_thickness(),
            increase_marker_opacity: default_increase_marker_opacity(),
            decrease_marker_opacity: default_decrease_marker_opacity(),
            select_selection_tool: default_select_selection_tool(),
            select_marker_tool: default_select_marker_tool(),
            select_eraser_tool: default_select_eraser_tool(),
            toggle_eraser_mode: default_toggle_eraser_mode(),
            select_pen_tool: default_select_pen_tool(),
            select_line_tool: default_select_line_tool(),
            select_rect_tool: default_select_rect_tool(),
            select_ellipse_tool: default_select_ellipse_tool(),
            select_arrow_tool: default_select_arrow_tool(),
            select_highlight_tool: default_select_highlight_tool(),
            toggle_highlight_tool: default_toggle_highlight_tool(),
            increase_font_size: default_increase_font_size(),
            decrease_font_size: default_decrease_font_size(),
            reset_arrow_labels: default_reset_arrow_labels(),
        }
    }
}
