use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Performance tuning options.
///
/// These settings control rendering performance and smoothness. Most users
/// won't need to change these from their defaults.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

    /// Maximum frame rate when vsync is disabled (0 = unlimited)
    /// Prevents CPU spinning at very high FPS. Set to match your monitor's
    /// refresh rate (e.g., 60, 120, 144, 240) or 0 for no limit.
    #[serde(default = "default_max_fps_no_vsync")]
    pub max_fps_no_vsync: u32,

    /// Target UI animation frame rate (0 = unlimited)
    /// Higher values make UI effects smoother at the cost of more redraws.
    #[serde(default = "default_ui_animation_fps")]
    pub ui_animation_fps: u32,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            buffer_count: default_buffer_count(),
            enable_vsync: default_enable_vsync(),
            max_fps_no_vsync: default_max_fps_no_vsync(),
            ui_animation_fps: default_ui_animation_fps(),
        }
    }
}

fn default_buffer_count() -> u32 {
    3
}

fn default_enable_vsync() -> bool {
    true
}

fn default_max_fps_no_vsync() -> u32 {
    60
}

fn default_ui_animation_fps() -> u32 {
    30
}
