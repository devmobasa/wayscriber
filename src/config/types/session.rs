use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

    /// Enable autosaving session data while the overlay is running.
    #[serde(default = "default_autosave_enabled")]
    pub autosave_enabled: bool,

    /// Idle debounce before autosave (milliseconds).
    #[serde(default = "default_autosave_idle_ms")]
    pub autosave_idle_ms: u64,

    /// Maximum interval between autosaves while dirty (milliseconds).
    #[serde(default = "default_autosave_interval_ms")]
    pub autosave_interval_ms: u64,

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
            persist_transparent: true,
            persist_whiteboard: true,
            persist_blackboard: true,
            persist_history: default_persist_history(),
            restore_tool_state: default_restore_tool_state(),
            autosave_enabled: default_autosave_enabled(),
            autosave_idle_ms: default_autosave_idle_ms(),
            autosave_interval_ms: default_autosave_interval_ms(),
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

pub const DEFAULT_AUTOSAVE_ENABLED: bool = true;
pub const DEFAULT_AUTOSAVE_IDLE_MS: u64 = 5_000;
pub const DEFAULT_AUTOSAVE_INTERVAL_MS: u64 = 45_000;

fn default_autosave_enabled() -> bool {
    DEFAULT_AUTOSAVE_ENABLED
}

fn default_autosave_idle_ms() -> u64 {
    DEFAULT_AUTOSAVE_IDLE_MS
}

fn default_autosave_interval_ms() -> u64 {
    DEFAULT_AUTOSAVE_INTERVAL_MS
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
