use serde::{Deserialize, Serialize};

/// Performance tuning options.
///
/// These settings control rendering performance and smoothness. Defaults favor
/// low drawing latency over strict display-synchronized rendering.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Number of buffers for buffering (valid range: 2 - 4)
    /// - 2 = double buffering (lower memory, potential tearing)
    /// - 3 = triple buffering (balanced, recommended)
    /// - 4 = quad buffering (highest memory, smoothest)
    #[serde(default = "default_buffer_count")]
    pub buffer_count: u32,

    /// Enable vsync frame synchronization to prevent tearing.
    /// The default is false for lower drawing latency.
    #[serde(default = "default_enable_vsync")]
    pub enable_vsync: bool,

    /// Maximum frame rate when vsync is disabled (0 = unlimited).
    /// Prevents CPU spinning at very high FPS. 120 FPS keeps input latency low
    /// while avoiding uncapped redraw loops.
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
    false
}

fn default_max_fps_no_vsync() -> u32 {
    120
}

fn default_ui_animation_fps() -> u32 {
    30
}
