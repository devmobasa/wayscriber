//! Keybinding configuration types and parsing.
//!
//! This module defines the configurable keybinding system that allows users
//! to customize keyboard shortcuts for all actions in the application.

mod authorship;
mod binding;
mod config;
mod defaults;

pub use crate::domain::Action;
pub use authorship::KeybindingAuthorship;
pub use binding::{KeyBinding, NAMED_KEYS, is_deliverable_key_name, suggest_key_name};
pub use config::{KeybindingConflict, KeybindingsConfig};

/// The compiled-in default keymap, built once per process.
///
/// Constructing a `KeybindingsConfig` allocates a `Vec<String>` per action —
/// hundreds of strings — and callers of the "is this action rebindable" check
/// include the command palette's per-frame tooltip path.
pub fn default_keybindings() -> &'static KeybindingsConfig {
    static DEFAULTS: std::sync::OnceLock<KeybindingsConfig> = std::sync::OnceLock::new();
    DEFAULTS.get_or_init(KeybindingsConfig::default)
}

#[cfg(test)]
mod tests;
