use crate::config::Config;
use crate::draw::{
    ArrowStyle, BlurStyle, Color, EraserKind, FontDescriptor, Frame, REGULAR_POLYGON_DEFAULT_SIDES,
};
use crate::input::{DrawingStyle, EraserMode, InputState, PerToolDrawingSettings, Tool};
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
///
/// Tool state only: what the user draws with (tool, colors, thicknesses,
/// fonts, arrow and polygon geometry, eraser and blur settings, the recent
/// palette, and the per-tool profile). Chrome preferences such as status-bar
/// visibility are not here on purpose — they are configured in `config.toml`
/// and toggled for the running process only, so persisting one would make an
/// explicitly this-run-only toggle outlive the run and outrank the configured
/// value on the next start. Sessions written before that split still carry a
/// `show_status_bar` key; it deserializes as an unknown field and is ignored.
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
    pub blur_style: BlurStyle,
    /// Recently applied colours, most-recent-first. Absent in sessions written
    /// before recents were persisted, which restore an empty list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_colors: Vec<Color>,
    #[serde(default)]
    pub marker_opacity: Option<f64>,
    /// Release-time stroke smoothing. Absent in sessions written before it
    /// existed, which restore the configured level instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pen_smoothing: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spotlight_magnification: Option<f64>,
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
    /// Style copied into the next arrow drawn. Absent in sessions written
    /// before arrow styles existed, which restore the configured default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrow_style: Option<ArrowStyle>,
    #[serde(default)]
    pub arrow_label_enabled: Option<bool>,
    #[serde(default = "default_polygon_sides_for_snapshot")]
    pub polygon_sides: u8,
    pub board_previous_color: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_settings: Option<PerToolDrawingSettings>,
}

impl ToolStateSnapshot {
    pub(crate) fn from_input_state(input: &InputState) -> Self {
        let active_tool = input.session_active_tool();
        let mut snapshot = Self::from((&input.style, active_tool, input.board_previous_color()));
        snapshot.tool_override = input.session_tool_override();
        snapshot
    }

    #[allow(dead_code)]
    pub(crate) fn from_config(config: &Config) -> Self {
        let style = DrawingStyle::from((&config.drawing, &config.arrow, &config.spotlight));
        Self::from((&style, Tool::Pen, None))
    }
}

impl From<(&DrawingStyle, Tool, Option<Color>)> for ToolStateSnapshot {
    fn from(
        (style, active_tool, board_previous_color): (&DrawingStyle, Tool, Option<Color>),
    ) -> Self {
        Self {
            current_color: style.color_for_tool(active_tool),
            current_thickness: style.thickness_for_tool(active_tool),
            eraser_size: style.eraser_size,
            eraser_kind: style.eraser_kind,
            eraser_mode: style.eraser_mode,
            blur_style: style.blur_style,
            recent_colors: style.recent_colors.clone(),
            marker_opacity: Some(style.marker_opacity),
            pen_smoothing: Some(style.pen_smoothing),
            spotlight_magnification: Some(style.spotlight_magnification),
            fill_enabled: Some(style.fill_enabled),
            tool_override: None,
            current_font_size: style.current_font_size,
            font_descriptor: Some(style.font_descriptor.clone()),
            text_background_enabled: style.text_background_enabled,
            arrow_length: style.arrow_length,
            arrow_angle: style.arrow_angle,
            arrow_head_at_end: Some(style.arrow_head_at_end),
            arrow_style: Some(style.arrow_style),
            arrow_label_enabled: Some(style.arrow_label_enabled),
            polygon_sides: style.polygon_sides,
            board_previous_color,
            tool_settings: Some(style.tool_settings.clone()),
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
