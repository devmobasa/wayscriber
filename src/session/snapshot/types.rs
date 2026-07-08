use crate::config::Config;
use crate::draw::{
    Color, EraserKind, FontDescriptor, Frame, REGULAR_POLYGON_DEFAULT_SIDES, clamp_regular_sides,
};
use crate::input::{EraserMode, InputState, PerToolDrawingSettings, Tool};
use serde::{Deserialize, Serialize};

pub(super) const CURRENT_VERSION: u32 = 6;

/// Captured state suitable for serialisation or restoration.
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub active_board_id: String,
    pub boards: Vec<BoardSnapshot>,
    pub tool_state: Option<ToolStateSnapshot>,
}

#[derive(Debug, Clone)]
pub struct BoardSnapshot {
    pub id: String,
    pub pages: BoardPagesSnapshot,
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
    pub(crate) fn has_board_data(&self) -> bool {
        self.boards
            .iter()
            .any(|board| board.pages.has_persistable_data())
    }

    pub(super) fn is_empty(&self) -> bool {
        !self.has_board_data()
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_descriptor: Option<FontDescriptor>,
    pub text_background_enabled: bool,
    pub arrow_length: f64,
    pub arrow_angle: f64,
    #[serde(default)]
    pub arrow_head_at_end: Option<bool>,
    #[serde(default)]
    pub arrow_label_enabled: Option<bool>,
    #[serde(default = "default_polygon_sides_for_snapshot")]
    pub polygon_sides: u8,
    pub board_previous_color: Option<Color>,
    pub show_status_bar: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_settings: Option<PerToolDrawingSettings>,
}

impl ToolStateSnapshot {
    pub(crate) fn from_input_state(input: &InputState) -> Self {
        let active_tool = input.session_active_tool();
        Self {
            current_color: input.color_for_tool(active_tool),
            current_thickness: input.thickness_for_tool(active_tool),
            eraser_size: input.eraser_size,
            eraser_kind: input.eraser_kind,
            eraser_mode: input.eraser_mode,
            marker_opacity: Some(input.marker_opacity),
            fill_enabled: Some(input.fill_enabled),
            tool_override: input.session_tool_override(),
            current_font_size: input.current_font_size,
            font_descriptor: Some(input.font_descriptor.clone()),
            text_background_enabled: input.text_background_enabled,
            arrow_length: input.arrow_length,
            arrow_angle: input.arrow_angle,
            arrow_head_at_end: Some(input.arrow_head_at_end),
            arrow_label_enabled: Some(input.arrow_label_enabled),
            polygon_sides: input.polygon_sides,
            board_previous_color: input.board_previous_color,
            show_status_bar: input.session_show_status_bar(),
            tool_settings: Some(input.tool_settings.clone()),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn from_config(config: &Config) -> Self {
        let color = config.drawing.default_color.to_color();
        let thickness = config.drawing.default_thickness;
        let font_descriptor = FontDescriptor::new(
            config.drawing.font_family.clone(),
            config.drawing.font_weight.clone(),
            config.drawing.font_style.clone(),
        );
        let mut tool_settings = PerToolDrawingSettings::new(color, thickness);
        tool_settings.step_marker.thickness =
            crate::input::state::default_step_marker_size(config.drawing.default_font_size);

        Self {
            current_color: color,
            current_thickness: thickness,
            eraser_size: config.drawing.default_eraser_size,
            eraser_kind: EraserKind::Circle,
            eraser_mode: config.drawing.default_eraser_mode,
            marker_opacity: Some(config.drawing.marker_opacity),
            fill_enabled: Some(config.drawing.default_fill_enabled),
            tool_override: None,
            current_font_size: config.drawing.default_font_size,
            font_descriptor: Some(font_descriptor),
            text_background_enabled: config.drawing.text_background_enabled,
            arrow_length: config.arrow.length,
            arrow_angle: config.arrow.angle_degrees,
            arrow_head_at_end: Some(config.arrow.head_at_end),
            arrow_label_enabled: Some(false),
            polygon_sides: clamp_regular_sides(config.drawing.polygon_sides),
            board_previous_color: None,
            show_status_bar: config.ui.show_status_bar,
            tool_settings: Some(tool_settings),
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

fn default_polygon_sides_for_snapshot() -> u8 {
    REGULAR_POLYGON_DEFAULT_SIDES
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct SessionFile {
    #[serde(default = "default_file_version")]
    pub version: u32,
    pub last_modified: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_board_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boards: Vec<BoardFile>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct BoardFile {
    pub id: String,
    pub pages: Vec<Frame>,
    pub active_page: usize,
}

fn default_file_version() -> u32 {
    1
}
