//! Configuration type definitions.

use super::enums::{ColorSpec, StatusPosition};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

    /// Default pen thickness in pixels (valid range: 1.0 - 20.0)
    #[serde(default = "default_thickness")]
    pub default_thickness: f64,

    /// Default font size for text mode in points (valid range: 8.0 - 72.0)
    #[serde(default = "default_font_size")]
    pub default_font_size: f64,

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
            default_font_size: default_font_size(),
            font_family: default_font_family(),
            font_weight: default_font_weight(),
            font_style: default_font_style(),
            text_background_enabled: default_text_background(),
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
}

impl Default for ArrowConfig {
    fn default() -> Self {
        Self {
            length: default_arrow_length(),
            angle_degrees: default_arrow_angle(),
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

    /// Status bar screen position (top-left, top-right, bottom-left, bottom-right)
    #[serde(default = "default_status_position")]
    pub status_bar_position: StatusPosition,

    /// Status bar styling options
    #[serde(default)]
    pub status_bar_style: StatusBarStyle,

    /// Help overlay styling options
    #[serde(default)]
    pub help_overlay_style: HelpOverlayStyle,

    /// Click highlight visual indicator settings
    #[serde(default)]
    pub click_highlight: ClickHighlightConfig,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_status_bar: default_show_status(),
            status_bar_position: default_status_position(),
            status_bar_style: StatusBarStyle::default(),
            help_overlay_style: HelpOverlayStyle::default(),
            click_highlight: ClickHighlightConfig::default(),
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

// =============================================================================
// Default value functions
// =============================================================================

fn default_color() -> ColorSpec {
    ColorSpec::Name("red".to_string())
}

fn default_thickness() -> f64 {
    3.0
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

fn default_arrow_length() -> f64 {
    20.0
}

fn default_arrow_angle() -> f64 {
    30.0
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

    /// Filename template with chrono format specifiers (e.g., "%Y-%m-%d_%H%M%S")
    #[serde(default = "default_capture_filename")]
    pub filename_template: String,

    /// Image format for saved screenshots (e.g., "png", "jpg")
    #[serde(default = "default_capture_format")]
    pub format: String,

    /// Automatically copy screenshots to clipboard
    #[serde(default = "default_capture_clipboard")]
    pub copy_to_clipboard: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            enabled: default_capture_enabled(),
            save_directory: default_capture_directory(),
            filename_template: default_capture_filename(),
            format: default_capture_format(),
            copy_to_clipboard: default_capture_clipboard(),
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
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            persist_transparent: false,
            persist_whiteboard: false,
            persist_blackboard: false,
            restore_tool_state: default_restore_tool_state(),
            storage: default_session_storage_mode(),
            custom_directory: None,
            max_shapes_per_frame: default_max_shapes_per_frame(),
            max_file_size_mb: default_max_file_size_mb(),
            compress: default_session_compression(),
            auto_compress_threshold_kb: default_auto_compress_threshold_kb(),
            backup_retention: default_backup_retention(),
            per_output: default_session_per_output(),
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
