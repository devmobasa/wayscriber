use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
