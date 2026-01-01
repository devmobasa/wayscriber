use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Undo/redo playback configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
