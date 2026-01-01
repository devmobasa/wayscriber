use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
