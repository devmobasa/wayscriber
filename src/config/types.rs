//! Configuration type definitions.

use super::enums::{ColorSpec, StatusPosition};
use crate::draw::EraserKind;
use crate::input::{EraserMode, Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const PRESET_SLOTS_MIN: usize = 3;
pub const PRESET_SLOTS_MAX: usize = 5;

/// Drawing-related settings.
///
/// Controls the default appearance of drawing tools when the overlay first opens.
/// Users can change these values at runtime using keybindings.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DrawingConfig {
    /// Default pen color - either a named color (red, green, blue, yellow, orange, pink, white, black)
    /// or an RGB array like `[255, 0, 0]` for red
    #[serde(default = "default_color")]
    pub default_color: ColorSpec,

    /// Default pen thickness in pixels (valid range: 1.0 - 50.0)
    #[serde(default = "default_thickness")]
    pub default_thickness: f64,

    /// Default eraser size in pixels (valid range: 1.0 - 50.0)
    #[serde(default = "default_eraser_size")]
    pub default_eraser_size: f64,

    /// Default eraser behavior (brush or stroke)
    #[serde(default = "default_eraser_mode")]
    pub default_eraser_mode: EraserMode,

    /// Default marker opacity multiplier (0.05 - 0.9), applied to the current color alpha
    #[serde(default = "default_marker_opacity")]
    pub marker_opacity: f64,

    /// Whether shapes start filled when applicable
    #[serde(default = "default_fill_enabled")]
    pub default_fill_enabled: bool,

    /// Default font size for text mode in points (valid range: 8.0 - 72.0)
    #[serde(default = "default_font_size")]
    pub default_font_size: f64,

    /// Hit-test tolerance in pixels for selection (valid range: 1.0 - 20.0)
    #[serde(default = "default_hit_test_tolerance")]
    pub hit_test_tolerance: f64,

    /// Number of shapes processed linearly before enabling spatial index
    #[serde(default = "default_hit_test_threshold")]
    pub hit_test_linear_threshold: usize,

    /// Maximum undo actions retained (valid range: 10 - 1000)
    #[serde(default = "default_undo_stack_limit")]
    pub undo_stack_limit: usize,

    /// Font family name for text rendering (e.g., "Sans", "Monospace", "JetBrains Mono")
    /// Falls back to "Sans" if the specified font is not available
    /// Note: Install fonts system-wide and reference by family name
    #[serde(default = "default_font_family")]
    pub font_family: String,

    /// Font weight (e.g., "normal", "bold", "light", 400, 700)
    /// Can be a named weight or a numeric value (100-900)
    #[serde(default = "default_font_weight")]
    pub font_weight: String,

    /// Font style (e.g., "normal", "italic", "oblique")
    #[serde(default = "default_font_style")]
    pub font_style: String,

    /// Enable semi-transparent background box behind text for better contrast
    #[serde(default = "default_text_background")]
    pub text_background_enabled: bool,
}

impl Default for DrawingConfig {
    fn default() -> Self {
        Self {
            default_color: default_color(),
            default_thickness: default_thickness(),
            default_eraser_size: default_eraser_size(),
            default_eraser_mode: default_eraser_mode(),
            marker_opacity: default_marker_opacity(),
            default_fill_enabled: default_fill_enabled(),
            default_font_size: default_font_size(),
            hit_test_tolerance: default_hit_test_tolerance(),
            hit_test_linear_threshold: default_hit_test_threshold(),
            undo_stack_limit: default_undo_stack_limit(),
            font_family: default_font_family(),
            font_weight: default_font_weight(),
            font_style: default_font_style(),
            text_background_enabled: default_text_background(),
        }
    }
}

/// Tool preset configuration for quick slot switching.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolPresetConfig {
    /// Optional label for UI display.
    #[serde(default)]
    pub name: Option<String>,

    /// Tool to activate when applying the preset.
    pub tool: Tool,

    /// Drawing color to apply.
    pub color: ColorSpec,

    /// Tool size (thickness or eraser size depending on tool).
    pub size: f64,

    /// Optional eraser brush shape override.
    #[serde(default)]
    pub eraser_kind: Option<EraserKind>,

    /// Optional eraser mode override.
    #[serde(default)]
    pub eraser_mode: Option<EraserMode>,

    /// Optional marker opacity override.
    #[serde(default)]
    pub marker_opacity: Option<f64>,

    /// Optional fill state override.
    #[serde(default)]
    pub fill_enabled: Option<bool>,

    /// Optional font size override.
    #[serde(default)]
    pub font_size: Option<f64>,

    /// Optional text background override.
    #[serde(default)]
    pub text_background_enabled: Option<bool>,

    /// Optional arrow length override.
    #[serde(default)]
    pub arrow_length: Option<f64>,

    /// Optional arrow angle override.
    #[serde(default)]
    pub arrow_angle: Option<f64>,

    /// Optional arrow head placement override.
    #[serde(default)]
    pub arrow_head_at_end: Option<bool>,

    /// Optional status bar visibility override.
    #[serde(default)]
    pub show_status_bar: Option<bool>,
}

/// Preset slot configuration for quick tool switching.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PresetSlotsConfig {
    /// Number of visible preset slots (3-5).
    #[serde(default = "default_preset_slot_count")]
    pub slot_count: usize,

    /// Preset slot 1.
    #[serde(default)]
    pub slot_1: Option<ToolPresetConfig>,

    /// Preset slot 2.
    #[serde(default)]
    pub slot_2: Option<ToolPresetConfig>,

    /// Preset slot 3.
    #[serde(default)]
    pub slot_3: Option<ToolPresetConfig>,

    /// Preset slot 4.
    #[serde(default)]
    pub slot_4: Option<ToolPresetConfig>,

    /// Preset slot 5.
    #[serde(default)]
    pub slot_5: Option<ToolPresetConfig>,
}

impl PresetSlotsConfig {
    pub fn get_slot(&self, slot: usize) -> Option<&ToolPresetConfig> {
        match slot {
            1 => self.slot_1.as_ref(),
            2 => self.slot_2.as_ref(),
            3 => self.slot_3.as_ref(),
            4 => self.slot_4.as_ref(),
            5 => self.slot_5.as_ref(),
            _ => None,
        }
    }

    pub fn set_slot(&mut self, slot: usize, preset: Option<ToolPresetConfig>) {
        match slot {
            1 => self.slot_1 = preset,
            2 => self.slot_2 = preset,
            3 => self.slot_3 = preset,
            4 => self.slot_4 = preset,
            5 => self.slot_5 = preset,
            _ => {}
        }
    }
}

impl Default for PresetSlotsConfig {
    fn default() -> Self {
        Self {
            slot_count: default_preset_slot_count(),
            slot_1: None,
            slot_2: None,
            slot_3: None,
            slot_4: None,
            slot_5: None,
        }
    }
}

/// Arrow drawing settings.
///
/// Controls the appearance of arrowheads when using the arrow tool (Ctrl+Shift+Drag).
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ArrowConfig {
    /// Arrowhead length in pixels (valid range: 5.0 - 50.0)
    #[serde(default = "default_arrow_length")]
    pub length: f64,

    /// Arrowhead angle in degrees (valid range: 15.0 - 60.0)
    /// Smaller angles create narrower arrowheads, larger angles create wider ones
    #[serde(default = "default_arrow_angle")]
    pub angle_degrees: f64,

    /// Place the arrowhead at the end of the line instead of the start
    #[serde(default = "default_arrow_head_at_end")]
    pub head_at_end: bool,
}

impl Default for ArrowConfig {
    fn default() -> Self {
        Self {
            length: default_arrow_length(),
            angle_degrees: default_arrow_angle(),
            head_at_end: default_arrow_head_at_end(),
        }
    }
}

/// Undo/redo playback configuration.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HistoryConfig {
    /// Delay in milliseconds between steps when running "undo all by delay"
    #[serde(default = "default_undo_all_delay_ms")]
    pub undo_all_delay_ms: u64,

    /// Delay in milliseconds between steps when running "redo all by delay"
    #[serde(default = "default_redo_all_delay_ms")]
    pub redo_all_delay_ms: u64,

    /// Enable the custom undo/redo section in the toolbar
    #[serde(default = "default_custom_section_enabled")]
    pub custom_section_enabled: bool,

    /// Delay in milliseconds between steps for custom undo
    #[serde(default = "default_custom_undo_delay_ms")]
    pub custom_undo_delay_ms: u64,

    /// Delay in milliseconds between steps for custom redo
    #[serde(default = "default_custom_redo_delay_ms")]
    pub custom_redo_delay_ms: u64,

    /// Number of steps to play when running custom undo
    #[serde(default = "default_custom_undo_steps")]
    pub custom_undo_steps: usize,

    /// Number of steps to play when running custom redo
    #[serde(default = "default_custom_redo_steps")]
    pub custom_redo_steps: usize,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            undo_all_delay_ms: default_undo_all_delay_ms(),
            redo_all_delay_ms: default_redo_all_delay_ms(),
            custom_section_enabled: default_custom_section_enabled(),
            custom_undo_delay_ms: default_custom_undo_delay_ms(),
            custom_redo_delay_ms: default_custom_redo_delay_ms(),
            custom_undo_steps: default_custom_undo_steps(),
            custom_redo_steps: default_custom_redo_steps(),
        }
    }
}

/// Performance tuning options.
///
/// These settings control rendering performance and smoothness. Most users
/// won't need to change these from their defaults.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PerformanceConfig {
    /// Number of buffers for buffering (valid range: 2 - 4)
    /// - 2 = double buffering (lower memory, potential tearing)
    /// - 3 = triple buffering (balanced, recommended)
    /// - 4 = quad buffering (highest memory, smoothest)
    #[serde(default = "default_buffer_count")]
    pub buffer_count: u32,

    /// Enable vsync frame synchronization to prevent tearing
    /// Set to false for lower latency at the cost of potential screen tearing
    #[serde(default = "default_enable_vsync")]
    pub enable_vsync: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            buffer_count: default_buffer_count(),
            enable_vsync: default_enable_vsync(),
        }
    }
}

/// UI display preferences.
///
/// Controls the visibility and positioning of on-screen UI elements.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UiConfig {
    /// Show the status bar displaying current color, thickness, and tool
    #[serde(default = "default_show_status")]
    pub show_status_bar: bool,

    /// Show the frozen-mode badge when frozen is active
    #[serde(default = "default_show_frozen_badge")]
    pub show_frozen_badge: bool,

    /// Status bar screen position (top-left, top-right, bottom-left, bottom-right)
    #[serde(default = "default_status_position")]
    pub status_bar_position: StatusPosition,

    /// Status bar styling options
    #[serde(default)]
    pub status_bar_style: StatusBarStyle,

    /// Help overlay styling options
    #[serde(default)]
    pub help_overlay_style: HelpOverlayStyle,

    /// Preferred output name for the xdg-shell fallback overlay (GNOME).
    /// Falls back to last entered output or first available.
    #[serde(default)]
    pub preferred_output: Option<String>,

    /// Use fullscreen for the xdg-shell fallback (GNOME). Disable if fullscreen
    /// produces an opaque background; maximized is used when false.
    #[serde(default = "default_xdg_fullscreen")]
    pub xdg_fullscreen: bool,

    /// Click highlight visual indicator settings
    #[serde(default)]
    pub click_highlight: ClickHighlightConfig,

    /// Context menu preferences
    #[serde(default)]
    pub context_menu: ContextMenuUiConfig,

    /// Toolbar visibility and pinning options
    #[serde(default)]
    pub toolbar: ToolbarConfig,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_status_bar: default_show_status(),
            show_frozen_badge: default_show_frozen_badge(),
            status_bar_position: default_status_position(),
            status_bar_style: StatusBarStyle::default(),
            help_overlay_style: HelpOverlayStyle::default(),
            preferred_output: None,
            xdg_fullscreen: default_xdg_fullscreen(),
            click_highlight: ClickHighlightConfig::default(),
            context_menu: ContextMenuUiConfig::default(),
            toolbar: ToolbarConfig::default(),
        }
    }
}

/// Status bar styling configuration.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct StatusBarStyle {
    /// Font size for status bar text
    #[serde(default = "default_status_font_size")]
    pub font_size: f64,

    /// Padding around status bar text
    #[serde(default = "default_status_padding")]
    pub padding: f64,

    /// Background color [R, G, B, A] (0.0-1.0 range)
    #[serde(default = "default_status_bg_color")]
    pub bg_color: [f64; 4],

    /// Text color [R, G, B, A] (0.0-1.0 range)
    #[serde(default = "default_status_text_color")]
    pub text_color: [f64; 4],

    /// Color indicator dot radius
    #[serde(default = "default_status_dot_radius")]
    pub dot_radius: f64,
}

impl Default for StatusBarStyle {
    fn default() -> Self {
        Self {
            font_size: default_status_font_size(),
            padding: default_status_padding(),
            bg_color: default_status_bg_color(),
            text_color: default_status_text_color(),
            dot_radius: default_status_dot_radius(),
        }
    }
}

/// Click highlight configuration for mouse press indicator.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClickHighlightConfig {
    /// Whether the highlight effect starts enabled
    #[serde(default = "default_click_highlight_enabled")]
    pub enabled: bool,

    /// Radius of the highlight circle in pixels
    #[serde(default = "default_click_highlight_radius")]
    pub radius: f64,

    /// Outline thickness in pixels
    #[serde(default = "default_click_highlight_outline")]
    pub outline_thickness: f64,

    /// Lifetime of the highlight in milliseconds
    #[serde(default = "default_click_highlight_duration_ms")]
    pub duration_ms: u64,

    /// Fill color RGBA (0.0-1.0)
    #[serde(default = "default_click_highlight_fill_color")]
    pub fill_color: [f64; 4],

    /// Outline color RGBA (0.0-1.0)
    #[serde(default = "default_click_highlight_outline_color")]
    pub outline_color: [f64; 4],

    /// Derive highlight color from current pen color
    #[serde(default = "default_click_highlight_use_pen_color")]
    pub use_pen_color: bool,
}

/// Context menu visibility configuration.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ContextMenuUiConfig {
    #[serde(default = "default_context_menu_enabled")]
    pub enabled: bool,
}

impl Default for ContextMenuUiConfig {
    fn default() -> Self {
        Self {
            enabled: default_context_menu_enabled(),
        }
    }
}

impl Default for ClickHighlightConfig {
    fn default() -> Self {
        Self {
            enabled: default_click_highlight_enabled(),
            radius: default_click_highlight_radius(),
            outline_thickness: default_click_highlight_outline(),
            duration_ms: default_click_highlight_duration_ms(),
            fill_color: default_click_highlight_fill_color(),
            outline_color: default_click_highlight_outline_color(),
            use_pen_color: default_click_highlight_use_pen_color(),
        }
    }
}

/// Help overlay styling configuration.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HelpOverlayStyle {
    /// Font size for help overlay text
    #[serde(default = "default_help_font_size")]
    pub font_size: f64,

    /// Line height for help text
    #[serde(default = "default_help_line_height")]
    pub line_height: f64,

    /// Padding around help box
    #[serde(default = "default_help_padding")]
    pub padding: f64,

    /// Background color [R, G, B, A] (0.0-1.0 range)
    #[serde(default = "default_help_bg_color")]
    pub bg_color: [f64; 4],

    /// Border color [R, G, B, A] (0.0-1.0 range)
    #[serde(default = "default_help_border_color")]
    pub border_color: [f64; 4],

    /// Border line width
    #[serde(default = "default_help_border_width")]
    pub border_width: f64,

    /// Text color [R, G, B, A] (0.0-1.0 range)
    #[serde(default = "default_help_text_color")]
    pub text_color: [f64; 4],
}

impl Default for HelpOverlayStyle {
    fn default() -> Self {
        Self {
            font_size: default_help_font_size(),
            line_height: default_help_line_height(),
            padding: default_help_padding(),
            bg_color: default_help_bg_color(),
            border_color: default_help_border_color(),
            border_width: default_help_border_width(),
            text_color: default_help_text_color(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tablet Input (feature-gated)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(tablet)]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TabletInputConfig {
    /// Enable tablet/stylus events at runtime (feature must be compiled in).
    #[serde(default = "default_tablet_enabled")]
    pub enabled: bool,

    /// Enable pressure-to-thickness mapping.
    #[serde(default = "default_tablet_pressure_enabled")]
    pub pressure_enabled: bool,

    /// Minimum thickness when pressure is near 0.
    #[serde(default = "default_tablet_min_thickness")]
    pub min_thickness: f64,

    /// Maximum thickness when pressure is 1.0.
    #[serde(default = "default_tablet_max_thickness")]
    pub max_thickness: f64,
}

#[cfg(tablet)]
impl Default for TabletInputConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            pressure_enabled: true,
            min_thickness: 1.0,
            max_thickness: 8.0,
        }
    }
}

#[cfg(tablet)]
fn default_tablet_enabled() -> bool {
    false
}
#[cfg(tablet)]
fn default_tablet_pressure_enabled() -> bool {
    true
}
#[cfg(tablet)]
fn default_tablet_min_thickness() -> f64 {
    1.0
}
#[cfg(tablet)]
fn default_tablet_max_thickness() -> f64 {
    8.0
}

// =============================================================================
// Default value functions
// =============================================================================

fn default_color() -> ColorSpec {
    ColorSpec::Name("red".to_string())
}

fn default_thickness() -> f64 {
    3.0
}

fn default_eraser_size() -> f64 {
    12.0
}

fn default_eraser_mode() -> EraserMode {
    EraserMode::Brush
}

fn default_marker_opacity() -> f64 {
    0.32
}

fn default_fill_enabled() -> bool {
    false
}

fn default_font_size() -> f64 {
    32.0
}

fn default_font_family() -> String {
    "Sans".to_string()
}

fn default_font_weight() -> String {
    "bold".to_string()
}

fn default_font_style() -> String {
    "normal".to_string()
}

fn default_text_background() -> bool {
    false
}

fn default_hit_test_tolerance() -> f64 {
    6.0
}

fn default_hit_test_threshold() -> usize {
    400
}

fn default_undo_stack_limit() -> usize {
    100
}

fn default_preset_slot_count() -> usize {
    PRESET_SLOTS_MAX
}

fn default_arrow_length() -> f64 {
    20.0
}

fn default_arrow_angle() -> f64 {
    30.0
}

fn default_arrow_head_at_end() -> bool {
    false
}

fn default_undo_all_delay_ms() -> u64 {
    1000
}

fn default_redo_all_delay_ms() -> u64 {
    1000
}

fn default_custom_section_enabled() -> bool {
    false
}

fn default_custom_undo_delay_ms() -> u64 {
    1000
}

fn default_custom_redo_delay_ms() -> u64 {
    1000
}

fn default_custom_undo_steps() -> usize {
    5
}

fn default_custom_redo_steps() -> usize {
    5
}

fn default_buffer_count() -> u32 {
    3
}

fn default_enable_vsync() -> bool {
    true
}

fn default_show_status() -> bool {
    true
}

fn default_show_frozen_badge() -> bool {
    false
}

fn default_xdg_fullscreen() -> bool {
    false
}

fn default_status_position() -> StatusPosition {
    StatusPosition::BottomLeft
}

// Status bar style defaults
fn default_status_font_size() -> f64 {
    21.0 // 50% larger than previous 14.0
}

fn default_status_padding() -> f64 {
    15.0 // 50% larger than previous 10.0
}

fn default_status_bg_color() -> [f64; 4] {
    [0.0, 0.0, 0.0, 0.85] // More opaque (was 0.7) for better visibility
}

fn default_status_text_color() -> [f64; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn default_status_dot_radius() -> f64 {
    6.0 // 50% larger than previous 4.0
}

// Help overlay style defaults
fn default_help_font_size() -> f64 {
    18.0
}

fn default_help_line_height() -> f64 {
    28.0
}

fn default_help_padding() -> f64 {
    32.0
}

fn default_help_bg_color() -> [f64; 4] {
    [0.09, 0.1, 0.13, 0.92]
}

fn default_help_border_color() -> [f64; 4] {
    [0.33, 0.39, 0.52, 0.88]
}

fn default_help_border_width() -> f64 {
    2.0
}

fn default_help_text_color() -> [f64; 4] {
    [0.95, 0.96, 0.98, 1.0]
}

// Click highlight defaults
fn default_click_highlight_enabled() -> bool {
    false
}

fn default_click_highlight_radius() -> f64 {
    24.0
}

fn default_click_highlight_outline() -> f64 {
    4.0
}

fn default_click_highlight_duration_ms() -> u64 {
    750
}

fn default_click_highlight_fill_color() -> [f64; 4] {
    [1.0, 0.8, 0.0, 0.35]
}

fn default_click_highlight_outline_color() -> [f64; 4] {
    [1.0, 0.6, 0.0, 0.9]
}

fn default_click_highlight_use_pen_color() -> bool {
    true
}

fn default_context_menu_enabled() -> bool {
    true
}

/// Board mode configuration for whiteboard/blackboard features.
///
/// Controls the appearance and behavior of board modes, including background colors,
/// default pen colors, and whether to auto-adjust colors when entering board modes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BoardConfig {
    /// Enable board mode features (whiteboard/blackboard)
    #[serde(default = "default_board_enabled")]
    pub enabled: bool,

    /// Default mode on startup (transparent, whiteboard, or blackboard)
    #[serde(default = "default_board_mode")]
    pub default_mode: String,

    /// Whiteboard background color [R, G, B] (0.0-1.0 range)
    #[serde(default = "default_whiteboard_color")]
    pub whiteboard_color: [f64; 3],

    /// Blackboard background color [R, G, B] (0.0-1.0 range)
    #[serde(default = "default_blackboard_color")]
    pub blackboard_color: [f64; 3],

    /// Default pen color for whiteboard mode [R, G, B] (0.0-1.0 range)
    #[serde(default = "default_whiteboard_pen_color")]
    pub whiteboard_pen_color: [f64; 3],

    /// Default pen color for blackboard mode [R, G, B] (0.0-1.0 range)
    #[serde(default = "default_blackboard_pen_color")]
    pub blackboard_pen_color: [f64; 3],

    /// Automatically adjust pen color when entering board modes
    #[serde(default = "default_board_auto_adjust")]
    pub auto_adjust_pen: bool,
}

impl Default for BoardConfig {
    fn default() -> Self {
        Self {
            enabled: default_board_enabled(),
            default_mode: default_board_mode(),
            whiteboard_color: default_whiteboard_color(),
            blackboard_color: default_blackboard_color(),
            whiteboard_pen_color: default_whiteboard_pen_color(),
            blackboard_pen_color: default_blackboard_pen_color(),
            auto_adjust_pen: default_board_auto_adjust(),
        }
    }
}

// Board config defaults
fn default_board_enabled() -> bool {
    true
}

fn default_board_mode() -> String {
    "transparent".to_string()
}

fn default_whiteboard_color() -> [f64; 3] {
    [0.992, 0.992, 0.992] // Off-white #FDFDFD
}

fn default_blackboard_color() -> [f64; 3] {
    [0.067, 0.067, 0.067] // Near-black #111111
}

fn default_whiteboard_pen_color() -> [f64; 3] {
    [0.0, 0.0, 0.0] // Black
}

fn default_blackboard_pen_color() -> [f64; 3] {
    [1.0, 1.0, 1.0] // White
}

fn default_board_auto_adjust() -> bool {
    true
}

/// Screenshot capture configuration.
///
/// Controls the behavior of screenshot capture features including file saving,
/// clipboard integration, and capture shortcuts.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CaptureConfig {
    /// Enable screenshot capture functionality
    #[serde(default = "default_capture_enabled")]
    pub enabled: bool,

    /// Directory to save screenshots to (supports ~ expansion)
    #[serde(default = "default_capture_directory")]
    pub save_directory: String,

    /// Filename template (strftime-like subset: %Y, %m, %d, %H, %M, %S)
    #[serde(default = "default_capture_filename")]
    pub filename_template: String,

    /// Image format for saved screenshots (e.g., "png", "jpg")
    #[serde(default = "default_capture_format")]
    pub format: String,

    /// Automatically copy screenshots to clipboard
    #[serde(default = "default_capture_clipboard")]
    pub copy_to_clipboard: bool,

    /// Exit the overlay after any capture completes (forces exit for all capture types).
    /// When false, clipboard-only captures still auto-exit by default.
    #[serde(default = "default_capture_exit_after")]
    pub exit_after_capture: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            enabled: default_capture_enabled(),
            save_directory: default_capture_directory(),
            filename_template: default_capture_filename(),
            format: default_capture_format(),
            copy_to_clipboard: default_capture_clipboard(),
            exit_after_capture: default_capture_exit_after(),
        }
    }
}

// Capture config defaults
fn default_capture_enabled() -> bool {
    true
}

fn default_capture_directory() -> String {
    "~/Pictures/Wayscriber".to_string()
}

fn default_capture_filename() -> String {
    "screenshot_%Y-%m-%d_%H%M%S".to_string()
}

fn default_capture_format() -> String {
    "png".to_string()
}

fn default_capture_clipboard() -> bool {
    true
}

fn default_capture_exit_after() -> bool {
    false
}

/// Session persistence configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionConfig {
    /// Persist drawings from transparent mode between sessions.
    #[serde(default)]
    pub persist_transparent: bool,

    /// Persist drawings from whiteboard mode between sessions.
    #[serde(default)]
    pub persist_whiteboard: bool,

    /// Persist drawings from blackboard mode between sessions.
    #[serde(default)]
    pub persist_blackboard: bool,

    /// Persist undo/redo history between sessions.
    #[serde(default = "default_persist_history")]
    pub persist_history: bool,

    /// Restore tool state (color, thickness, font size, etc.) on next launch.
    #[serde(default = "default_restore_tool_state")]
    pub restore_tool_state: bool,

    /// Storage location for session files.
    #[serde(default = "default_session_storage_mode")]
    pub storage: SessionStorageMode,

    /// Custom directory used when `storage = "custom"`.
    #[serde(default)]
    pub custom_directory: Option<String>,

    /// Maximum shapes retained per frame during load/save.
    #[serde(default = "default_max_shapes_per_frame")]
    pub max_shapes_per_frame: usize,

    /// Maximum session file size (in megabytes).
    #[serde(default = "default_max_file_size_mb")]
    pub max_file_size_mb: u64,

    /// Compression mode for session files.
    #[serde(default = "default_session_compression")]
    pub compress: SessionCompression,

    /// Threshold (in kilobytes) beyond which automatic compression engages.
    #[serde(default = "default_auto_compress_threshold_kb")]
    pub auto_compress_threshold_kb: u64,

    /// Number of rotated backups to retain (0 disables backups).
    #[serde(default = "default_backup_retention")]
    pub backup_retention: usize,

    /// Separate persistence per output instead of per display.
    #[serde(default = "default_session_per_output")]
    pub per_output: bool,

    /// Maximum undo history depth persisted on disk (None = follow runtime limit).
    #[serde(default)]
    pub max_persisted_undo_depth: Option<usize>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            persist_transparent: false,
            persist_whiteboard: false,
            persist_blackboard: false,
            persist_history: default_persist_history(),
            restore_tool_state: default_restore_tool_state(),
            storage: default_session_storage_mode(),
            custom_directory: None,
            max_shapes_per_frame: default_max_shapes_per_frame(),
            max_file_size_mb: default_max_file_size_mb(),
            compress: default_session_compression(),
            auto_compress_threshold_kb: default_auto_compress_threshold_kb(),
            backup_retention: default_backup_retention(),
            per_output: default_session_per_output(),
            max_persisted_undo_depth: None,
        }
    }
}

/// Session storage location options.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SessionStorageMode {
    Auto,
    Config,
    Custom,
}

/// Session compression preferences.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SessionCompression {
    Auto,
    On,
    Off,
}

fn default_restore_tool_state() -> bool {
    true
}

fn default_session_storage_mode() -> SessionStorageMode {
    SessionStorageMode::Auto
}

fn default_max_shapes_per_frame() -> usize {
    10_000
}

fn default_max_file_size_mb() -> u64 {
    10
}

fn default_session_compression() -> SessionCompression {
    SessionCompression::Auto
}

fn default_auto_compress_threshold_kb() -> u64 {
    100
}

fn default_backup_retention() -> usize {
    1
}

fn default_session_per_output() -> bool {
    true
}

fn default_persist_history() -> bool {
    true
}

/// Toolbar layout complexity presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ToolbarLayoutMode {
    Simple,
    Regular,
    Advanced,
}

impl Default for ToolbarLayoutMode {
    fn default() -> Self {
        Self::Regular
    }
}

impl ToolbarLayoutMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Simple => "Simple",
            Self::Regular => "Regular",
            Self::Advanced => "Advanced",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Simple => Self::Regular,
            Self::Regular => Self::Advanced,
            Self::Advanced => Self::Simple,
        }
    }

    pub fn section_defaults(self) -> ToolbarSectionDefaults {
        match self {
            Self::Simple => ToolbarSectionDefaults {
                show_actions_section: true,
                show_actions_advanced: false,
                show_presets: false,
                show_step_section: false,
                show_text_controls: false,
                show_settings_section: false,
            },
            Self::Regular => ToolbarSectionDefaults {
                show_actions_section: true,
                show_actions_advanced: false,
                show_presets: true,
                show_step_section: false,
                show_text_controls: false,
                show_settings_section: true,
            },
            Self::Advanced => ToolbarSectionDefaults {
                show_actions_section: true,
                show_actions_advanced: true,
                show_presets: true,
                show_step_section: true,
                show_text_controls: true,
                show_settings_section: true,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ToolbarSectionDefaults {
    pub show_actions_section: bool,
    pub show_actions_advanced: bool,
    pub show_presets: bool,
    pub show_step_section: bool,
    pub show_text_controls: bool,
    pub show_settings_section: bool,
}

/// Toolbar visibility and pinning configuration.
///
/// Controls which toolbar panels are visible on startup and whether they
/// remain pinned (saved to config) when closed.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolbarConfig {
    /// Toolbar layout preset (simple, regular, advanced)
    #[serde(default = "default_toolbar_layout_mode")]
    pub layout_mode: ToolbarLayoutMode,

    /// Show the top toolbar (tool selection) on startup
    #[serde(default = "default_toolbar_top_pinned")]
    pub top_pinned: bool,

    /// Show the side toolbar (colors, settings) on startup
    #[serde(default = "default_toolbar_side_pinned")]
    pub side_pinned: bool,

    /// Use icons instead of text labels in toolbars
    #[serde(default = "default_toolbar_use_icons")]
    pub use_icons: bool,

    /// Show extended color palette
    #[serde(default = "default_show_more_colors")]
    pub show_more_colors: bool,

    /// Show the Actions section (undo all, redo all, etc.)
    #[serde(default = "default_show_actions_section")]
    pub show_actions_section: bool,

    /// Show advanced actions (undo all, zoom, freeze, etc.)
    #[serde(default = "default_show_actions_advanced")]
    pub show_actions_advanced: bool,

    /// Show the presets section in the side toolbar
    #[serde(default = "default_show_presets")]
    pub show_presets: bool,

    /// Show the Step Undo/Redo section
    #[serde(default = "default_show_step_section")]
    pub show_step_section: bool,

    /// Keep text controls visible even when text is not active
    #[serde(default = "default_show_text_controls")]
    pub show_text_controls: bool,

    /// Show the Settings section (config shortcuts, layout controls)
    #[serde(default = "default_show_settings_section")]
    pub show_settings_section: bool,

    /// Show delay sliders in Step Undo/Redo section
    #[serde(default = "default_show_delay_sliders")]
    pub show_delay_sliders: bool,

    /// Show the marker opacity slider section in the side toolbar
    #[serde(default = "default_show_marker_opacity_section")]
    pub show_marker_opacity_section: bool,

    /// Show preset action toast notifications
    #[serde(default = "default_show_preset_toasts")]
    pub show_preset_toasts: bool,

    /// Saved horizontal offset for the top toolbar (layer-shell/inline)
    #[serde(default)]
    pub top_offset: f64,

    /// Saved vertical offset for the top toolbar (layer-shell/inline)
    #[serde(default)]
    pub top_offset_y: f64,

    /// Saved vertical offset for the side toolbar (layer-shell/inline)
    #[serde(default)]
    pub side_offset: f64,

    /// Saved horizontal offset for the side toolbar (layer-shell/inline)
    #[serde(default)]
    pub side_offset_x: f64,

    /// Force inline toolbars even when layer-shell is available (debug/compatibility).
    #[serde(default)]
    pub force_inline: bool,
}

impl Default for ToolbarConfig {
    fn default() -> Self {
        Self {
            layout_mode: default_toolbar_layout_mode(),
            top_pinned: default_toolbar_top_pinned(),
            side_pinned: default_toolbar_side_pinned(),
            use_icons: default_toolbar_use_icons(),
            show_more_colors: default_show_more_colors(),
            show_actions_section: default_show_actions_section(),
            show_actions_advanced: default_show_actions_advanced(),
            show_presets: default_show_presets(),
            show_step_section: default_show_step_section(),
            show_text_controls: default_show_text_controls(),
            show_settings_section: default_show_settings_section(),
            show_delay_sliders: default_show_delay_sliders(),
            show_marker_opacity_section: default_show_marker_opacity_section(),
            show_preset_toasts: default_show_preset_toasts(),
            top_offset: 0.0,
            top_offset_y: 0.0,
            side_offset: 0.0,
            side_offset_x: 0.0,
            force_inline: false,
        }
    }
}

fn default_toolbar_top_pinned() -> bool {
    true
}

fn default_toolbar_side_pinned() -> bool {
    true
}

fn default_toolbar_use_icons() -> bool {
    true
}

fn default_toolbar_layout_mode() -> ToolbarLayoutMode {
    ToolbarLayoutMode::Regular
}

fn default_show_more_colors() -> bool {
    false
}

fn default_show_actions_section() -> bool {
    true
}

fn default_show_actions_advanced() -> bool {
    false
}

fn default_show_presets() -> bool {
    true
}

fn default_show_step_section() -> bool {
    false
}

fn default_show_text_controls() -> bool {
    false
}

fn default_show_settings_section() -> bool {
    true
}

fn default_show_delay_sliders() -> bool {
    false
}

fn default_show_marker_opacity_section() -> bool {
    false
}

fn default_show_preset_toasts() -> bool {
    true
}
