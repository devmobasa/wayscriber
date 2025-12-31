use crate::draw::{Color, EraserKind, Frame};
use crate::input::{EraserMode, InputState, Tool, board_mode::BoardMode};
use serde::{Deserialize, Serialize};

pub(super) const CURRENT_VERSION: u32 = 4;

/// Captured state suitable for serialisation or restoration.
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub active_mode: BoardMode,
    pub transparent: Option<BoardPagesSnapshot>,
    pub whiteboard: Option<BoardPagesSnapshot>,
    pub blackboard: Option<BoardPagesSnapshot>,
    pub tool_state: Option<ToolStateSnapshot>,
}

#[derive(Debug, Clone)]
pub struct BoardPagesSnapshot {
    pub pages: Vec<Frame>,
    pub active: usize,
}

impl BoardPagesSnapshot {
    pub(super) fn has_persistable_data(&self) -> bool {
        if self.pages.len() > 1 || self.active > 0 {
            return true;
        }
        self.pages.iter().any(|page| page.has_persistable_data())
    }
}

impl SessionSnapshot {
    pub(super) fn is_empty(&self) -> bool {
        let empty_pages = |pages: &Option<BoardPagesSnapshot>| {
            pages
                .as_ref()
                .is_none_or(|data| !data.has_persistable_data())
        };
        empty_pages(&self.transparent)
            && empty_pages(&self.whiteboard)
            && empty_pages(&self.blackboard)
    }
}

/// Subset of [`InputState`] we persist to disk to restore tool context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStateSnapshot {
    pub current_color: Color,
    pub current_thickness: f64,
    #[serde(default = "default_eraser_size_for_snapshot")]
    pub eraser_size: f64,
    #[serde(default = "default_eraser_kind_for_snapshot")]
    pub eraser_kind: EraserKind,
    #[serde(default = "default_eraser_mode_for_snapshot")]
    pub eraser_mode: EraserMode,
    #[serde(default)]
    pub marker_opacity: Option<f64>,
    #[serde(default)]
    pub fill_enabled: Option<bool>,
    #[serde(default)]
    pub tool_override: Option<Tool>,
    pub current_font_size: f64,
    pub text_background_enabled: bool,
    pub arrow_length: f64,
    pub arrow_angle: f64,
    #[serde(default)]
    pub arrow_head_at_end: Option<bool>,
    pub board_previous_color: Option<Color>,
    pub show_status_bar: bool,
}

impl ToolStateSnapshot {
    pub(super) fn from_input_state(input: &InputState) -> Self {
        Self {
            current_color: input.current_color,
            current_thickness: input.current_thickness,
            eraser_size: input.eraser_size,
            eraser_kind: input.eraser_kind,
            eraser_mode: input.eraser_mode,
            marker_opacity: Some(input.marker_opacity),
            fill_enabled: Some(input.fill_enabled),
            tool_override: input.tool_override(),
            current_font_size: input.current_font_size,
            text_background_enabled: input.text_background_enabled,
            arrow_length: input.arrow_length,
            arrow_angle: input.arrow_angle,
            arrow_head_at_end: Some(input.arrow_head_at_end),
            board_previous_color: input.board_previous_color,
            show_status_bar: input.show_status_bar,
        }
    }
}

fn default_eraser_size_for_snapshot() -> f64 {
    12.0
}

fn default_eraser_kind_for_snapshot() -> EraserKind {
    EraserKind::Circle
}

fn default_eraser_mode_for_snapshot() -> EraserMode {
    EraserMode::Brush
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct SessionFile {
    #[serde(default = "default_file_version")]
    pub version: u32,
    pub last_modified: String,
    pub active_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparent: Option<Frame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whiteboard: Option<Frame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blackboard: Option<Frame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparent_pages: Option<Vec<Frame>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whiteboard_pages: Option<Vec<Frame>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blackboard_pages: Option<Vec<Frame>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparent_active_page: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whiteboard_active_page: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blackboard_active_page: Option<usize>,
    #[serde(default)]
    pub tool_state: Option<ToolStateSnapshot>,
}

fn default_file_version() -> u32 {
    1
}
