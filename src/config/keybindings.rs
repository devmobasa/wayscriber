//! Keybinding configuration types and parsing.
//!
//! This module defines the configurable keybinding system that allows users
//! to customize keyboard shortcuts for all actions in the application.

mod binding;
mod config;
mod defaults;

pub use crate::domain::Action;
pub use binding::KeyBinding;
pub use config::KeybindingsConfig;

#[cfg(test)]
mod tests;
