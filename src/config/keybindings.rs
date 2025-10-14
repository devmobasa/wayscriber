//! Keybinding configuration types and parsing.
//!
//! This module defines the configurable keybinding system that allows users
//! to customize keyboard shortcuts for all actions in the application.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// All possible actions that can be bound to keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    // Exit and cancellation
    Exit,

    // Drawing actions
    EnterTextMode,
    ClearCanvas,
    Undo,

    // Thickness controls
    IncreaseThickness,
    DecreaseThickness,
    IncreaseFontSize,
    DecreaseFontSize,

    // Board mode toggles
    ToggleWhiteboard,
    ToggleBlackboard,
    ReturnToTransparent,

    // UI toggles
    ToggleHelp,

    // Color selections (using char to represent the color)
    SetColorRed,
    SetColorGreen,
    SetColorBlue,
    SetColorYellow,
    SetColorOrange,
    SetColorPink,
    SetColorWhite,
    SetColorBlack,
}

/// A single keybinding: a key character with optional modifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl KeyBinding {
    /// Parse a keybinding string like "Ctrl+Shift+W" or "Escape".
    /// Special handling for keys that contain '+' or '-' as the actual key.
    /// Supports spaces around '+' (e.g., "Ctrl + Shift + W")
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if s.is_empty() {
            return Err("Empty keybinding string".to_string());
        }

        // Normalize by removing spaces around '+'
        let s_normalized = s.replace(" + ", "+").replace("+ ", "+").replace(" +", "+");
        let s_lower = s_normalized.to_lowercase();

        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;

        // Check for modifiers at the start
        let mut remaining: &str = &s_normalized;

        // Try to extract modifiers from the beginning
        if s_lower.starts_with("ctrl+") || s_lower.starts_with("control+") {
            ctrl = true;
            let prefix_len = if s_lower.starts_with("ctrl+") { 5 } else { 8 };
            remaining = &remaining[prefix_len..];
        }

        let remaining_lower = remaining.to_lowercase();
        if remaining_lower.starts_with("shift+") {
            shift = true;
            remaining = &remaining[6..];
        }

        let remaining_lower = remaining.to_lowercase();
        if remaining_lower.starts_with("alt+") {
            alt = true;
            remaining = &remaining[4..];
        }

        // Whatever remains is the key
        if remaining.is_empty() {
            return Err(format!("No key specified in: {}", s));
        }

        Ok(Self {
            key: remaining.to_string(),
            ctrl,
            shift,
            alt,
        })
    }

    /// Check if this keybinding matches the current input state.
    pub fn matches(&self, key: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
        self.key.eq_ignore_ascii_case(key)
            && self.ctrl == ctrl
            && self.shift == shift
            && self.alt == alt
    }
}

/// Configuration for all keybindings.
///
/// Each action can have multiple keybindings. Users specify them in config.toml as:
/// ```toml
/// [keybindings]
/// exit = ["Escape", "Ctrl+Q"]
/// undo = ["Ctrl+Z"]
/// clear_canvas = ["E"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindingsConfig {
    #[serde(default = "default_exit")]
    pub exit: Vec<String>,

    #[serde(default = "default_enter_text_mode")]
    pub enter_text_mode: Vec<String>,

    #[serde(default = "default_clear_canvas")]
    pub clear_canvas: Vec<String>,

    #[serde(default = "default_undo")]
    pub undo: Vec<String>,

    #[serde(default = "default_increase_thickness")]
    pub increase_thickness: Vec<String>,

    #[serde(default = "default_decrease_thickness")]
    pub decrease_thickness: Vec<String>,

    #[serde(default = "default_increase_font_size")]
    pub increase_font_size: Vec<String>,

    #[serde(default = "default_decrease_font_size")]
    pub decrease_font_size: Vec<String>,

    #[serde(default = "default_toggle_whiteboard")]
    pub toggle_whiteboard: Vec<String>,

    #[serde(default = "default_toggle_blackboard")]
    pub toggle_blackboard: Vec<String>,

    #[serde(default = "default_return_to_transparent")]
    pub return_to_transparent: Vec<String>,

    #[serde(default = "default_toggle_help")]
    pub toggle_help: Vec<String>,

    #[serde(default = "default_set_color_red")]
    pub set_color_red: Vec<String>,

    #[serde(default = "default_set_color_green")]
    pub set_color_green: Vec<String>,

    #[serde(default = "default_set_color_blue")]
    pub set_color_blue: Vec<String>,

    #[serde(default = "default_set_color_yellow")]
    pub set_color_yellow: Vec<String>,

    #[serde(default = "default_set_color_orange")]
    pub set_color_orange: Vec<String>,

    #[serde(default = "default_set_color_pink")]
    pub set_color_pink: Vec<String>,

    #[serde(default = "default_set_color_white")]
    pub set_color_white: Vec<String>,

    #[serde(default = "default_set_color_black")]
    pub set_color_black: Vec<String>,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            exit: default_exit(),
            enter_text_mode: default_enter_text_mode(),
            clear_canvas: default_clear_canvas(),
            undo: default_undo(),
            increase_thickness: default_increase_thickness(),
            decrease_thickness: default_decrease_thickness(),
            increase_font_size: default_increase_font_size(),
            decrease_font_size: default_decrease_font_size(),
            toggle_whiteboard: default_toggle_whiteboard(),
            toggle_blackboard: default_toggle_blackboard(),
            return_to_transparent: default_return_to_transparent(),
            toggle_help: default_toggle_help(),
            set_color_red: default_set_color_red(),
            set_color_green: default_set_color_green(),
            set_color_blue: default_set_color_blue(),
            set_color_yellow: default_set_color_yellow(),
            set_color_orange: default_set_color_orange(),
            set_color_pink: default_set_color_pink(),
            set_color_white: default_set_color_white(),
            set_color_black: default_set_color_black(),
        }
    }
}

impl KeybindingsConfig {
    /// Build a lookup map from keybindings to actions for efficient matching.
    /// Returns an error if any keybinding string is invalid.
    pub fn build_action_map(&self) -> Result<HashMap<KeyBinding, Action>, String> {
        let mut map = HashMap::new();

        for binding_str in &self.exit {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::Exit);
        }

        for binding_str in &self.enter_text_mode {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::EnterTextMode);
        }

        for binding_str in &self.clear_canvas {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::ClearCanvas);
        }

        for binding_str in &self.undo {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::Undo);
        }

        for binding_str in &self.increase_thickness {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::IncreaseThickness);
        }

        for binding_str in &self.decrease_thickness {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::DecreaseThickness);
        }

        for binding_str in &self.increase_font_size {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::IncreaseFontSize);
        }

        for binding_str in &self.decrease_font_size {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::DecreaseFontSize);
        }

        for binding_str in &self.toggle_whiteboard {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::ToggleWhiteboard);
        }

        for binding_str in &self.toggle_blackboard {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::ToggleBlackboard);
        }

        for binding_str in &self.return_to_transparent {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::ReturnToTransparent);
        }

        for binding_str in &self.toggle_help {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::ToggleHelp);
        }

        for binding_str in &self.set_color_red {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::SetColorRed);
        }

        for binding_str in &self.set_color_green {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::SetColorGreen);
        }

        for binding_str in &self.set_color_blue {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::SetColorBlue);
        }

        for binding_str in &self.set_color_yellow {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::SetColorYellow);
        }

        for binding_str in &self.set_color_orange {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::SetColorOrange);
        }

        for binding_str in &self.set_color_pink {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::SetColorPink);
        }

        for binding_str in &self.set_color_white {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::SetColorWhite);
        }

        for binding_str in &self.set_color_black {
            let binding = KeyBinding::parse(binding_str)?;
            map.insert(binding, Action::SetColorBlack);
        }

        Ok(map)
    }
}

// =============================================================================
// Default keybinding functions (matching current hardcoded behavior)
// =============================================================================

fn default_exit() -> Vec<String> {
    vec!["Escape".to_string(), "Ctrl+Q".to_string()]
}

fn default_enter_text_mode() -> Vec<String> {
    vec!["T".to_string()]
}

fn default_clear_canvas() -> Vec<String> {
    vec!["E".to_string()]
}

fn default_undo() -> Vec<String> {
    vec!["Ctrl+Z".to_string()]
}

fn default_increase_thickness() -> Vec<String> {
    vec!["+".to_string(), "=".to_string()]
}

fn default_decrease_thickness() -> Vec<String> {
    vec!["-".to_string(), "_".to_string()]
}

fn default_increase_font_size() -> Vec<String> {
    vec!["Ctrl+Shift++".to_string(), "Ctrl+Shift+=".to_string()]
}

fn default_decrease_font_size() -> Vec<String> {
    vec!["Ctrl+Shift+-".to_string(), "Ctrl+Shift+_".to_string()]
}

fn default_toggle_whiteboard() -> Vec<String> {
    vec!["Ctrl+W".to_string()]
}

fn default_toggle_blackboard() -> Vec<String> {
    vec!["Ctrl+B".to_string()]
}

fn default_return_to_transparent() -> Vec<String> {
    vec!["Ctrl+Shift+T".to_string()]
}

fn default_toggle_help() -> Vec<String> {
    vec!["F10".to_string()]
}

fn default_set_color_red() -> Vec<String> {
    vec!["R".to_string()]
}

fn default_set_color_green() -> Vec<String> {
    vec!["G".to_string()]
}

fn default_set_color_blue() -> Vec<String> {
    vec!["B".to_string()]
}

fn default_set_color_yellow() -> Vec<String> {
    vec!["Y".to_string()]
}

fn default_set_color_orange() -> Vec<String> {
    vec!["O".to_string()]
}

fn default_set_color_pink() -> Vec<String> {
    vec!["P".to_string()]
}

fn default_set_color_white() -> Vec<String> {
    vec!["W".to_string()]
}

fn default_set_color_black() -> Vec<String> {
    vec!["K".to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_key() {
        let binding = KeyBinding::parse("Escape").unwrap();
        assert_eq!(binding.key, "Escape");
        assert!(!binding.ctrl);
        assert!(!binding.shift);
        assert!(!binding.alt);
    }

    #[test]
    fn test_parse_ctrl_key() {
        let binding = KeyBinding::parse("Ctrl+Z").unwrap();
        assert_eq!(binding.key, "Z");
        assert!(binding.ctrl);
        assert!(!binding.shift);
        assert!(!binding.alt);
    }

    #[test]
    fn test_parse_ctrl_shift_key() {
        let binding = KeyBinding::parse("Ctrl+Shift+W").unwrap();
        assert_eq!(binding.key, "W");
        assert!(binding.ctrl);
        assert!(binding.shift);
        assert!(!binding.alt);
    }

    #[test]
    fn test_parse_all_modifiers() {
        let binding = KeyBinding::parse("Ctrl+Shift+Alt+A").unwrap();
        assert_eq!(binding.key, "A");
        assert!(binding.ctrl);
        assert!(binding.shift);
        assert!(binding.alt);
    }

    #[test]
    fn test_parse_case_insensitive() {
        let binding = KeyBinding::parse("ctrl+shift+w").unwrap();
        assert_eq!(binding.key, "w");
        assert!(binding.ctrl);
        assert!(binding.shift);
    }

    #[test]
    fn test_parse_with_spaces() {
        let binding = KeyBinding::parse("Ctrl + Shift + W").unwrap();
        assert_eq!(binding.key, "W");
        assert!(binding.ctrl);
        assert!(binding.shift);
    }

    #[test]
    fn test_matches() {
        let binding = KeyBinding::parse("Ctrl+Shift+W").unwrap();
        assert!(binding.matches("W", true, true, false));
        assert!(binding.matches("w", true, true, false)); // Case insensitive
        assert!(!binding.matches("W", false, true, false)); // Missing ctrl
        assert!(!binding.matches("W", true, false, false)); // Missing shift
        assert!(!binding.matches("A", true, true, false)); // Wrong key
    }

    #[test]
    fn test_build_action_map() {
        let config = KeybindingsConfig::default();
        let map = config.build_action_map().unwrap();

        // Check that some default bindings are present
        let escape = KeyBinding::parse("Escape").unwrap();
        assert_eq!(map.get(&escape), Some(&Action::Exit));

        let ctrl_z = KeyBinding::parse("Ctrl+Z").unwrap();
        assert_eq!(map.get(&ctrl_z), Some(&Action::Undo));
    }
}
