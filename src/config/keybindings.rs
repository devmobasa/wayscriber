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

    // Screenshot capture actions
    CaptureFullScreen,
    CaptureActiveWindow,
    CaptureSelection,
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
    /// Modifiers can appear in any order: "Shift+Ctrl+W", "Alt+Shift+Ctrl+W", etc.
    /// Supports spaces around '+' (e.g., "Ctrl + Shift + W")
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if s.is_empty() {
            return Err("Empty keybinding string".to_string());
        }

        // Normalize by removing spaces around '+'
        let s_normalized = s.replace(" + ", "+").replace("+ ", "+").replace(" +", "+");

        // Split on '+' to get all parts
        let parts: Vec<&str> = s_normalized.split('+').collect();

        if parts.is_empty() {
            return Err("Empty keybinding string".to_string());
        }

        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;
        let mut key_parts = Vec::new();

        // Process each part, checking if it's a modifier or the actual key
        for part in parts {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => ctrl = true,
                "shift" => shift = true,
                "alt" => alt = true,
                _ => {
                    // Not a modifier, so it's part of the key
                    key_parts.push(part);
                }
            }
        }

        // Reconstruct the key from remaining parts (handles cases like "+" being the key)
        if key_parts.is_empty() {
            return Err(format!("No key specified in: {}", s));
        }

        // Join with '+' to handle the case where the key itself is '+'
        // (e.g., "Ctrl+Shift++" becomes ["Ctrl", "Shift", "", ""] with last two being the '+' key)
        let key = key_parts.join("+");

        if key.is_empty() {
            // This happens for "Ctrl+Shift++" where we have empty strings after the modifiers
            // The key is actually '+'
            Ok(Self {
                key: "+".to_string(),
                ctrl,
                shift,
                alt,
            })
        } else {
            Ok(Self {
                key,
                ctrl,
                shift,
                alt,
            })
        }
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

    #[serde(default = "default_capture_full_screen")]
    pub capture_full_screen: Vec<String>,

    #[serde(default = "default_capture_active_window")]
    pub capture_active_window: Vec<String>,

    #[serde(default = "default_capture_selection")]
    pub capture_selection: Vec<String>,
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
            capture_full_screen: default_capture_full_screen(),
            capture_active_window: default_capture_active_window(),
            capture_selection: default_capture_selection(),
        }
    }
}

impl KeybindingsConfig {
    /// Build a lookup map from keybindings to actions for efficient matching.
    /// Returns an error if any keybinding string is invalid or if duplicates are detected.
    pub fn build_action_map(&self) -> Result<HashMap<KeyBinding, Action>, String> {
        let mut map = HashMap::new();

        // Helper closure to insert and check for duplicates
        let mut insert_binding = |binding_str: &str, action: Action| -> Result<(), String> {
            let binding = KeyBinding::parse(binding_str)?;
            if let Some(existing_action) = map.insert(binding.clone(), action) {
                return Err(format!(
                    "Duplicate keybinding '{}' assigned to both {:?} and {:?}",
                    binding_str, existing_action, action
                ));
            }
            Ok(())
        };

        for binding_str in &self.exit {
            insert_binding(binding_str, Action::Exit)?;
        }

        for binding_str in &self.enter_text_mode {
            insert_binding(binding_str, Action::EnterTextMode)?;
        }

        for binding_str in &self.clear_canvas {
            insert_binding(binding_str, Action::ClearCanvas)?;
        }

        for binding_str in &self.undo {
            insert_binding(binding_str, Action::Undo)?;
        }

        for binding_str in &self.increase_thickness {
            insert_binding(binding_str, Action::IncreaseThickness)?;
        }

        for binding_str in &self.decrease_thickness {
            insert_binding(binding_str, Action::DecreaseThickness)?;
        }

        for binding_str in &self.increase_font_size {
            insert_binding(binding_str, Action::IncreaseFontSize)?;
        }

        for binding_str in &self.decrease_font_size {
            insert_binding(binding_str, Action::DecreaseFontSize)?;
        }

        for binding_str in &self.toggle_whiteboard {
            insert_binding(binding_str, Action::ToggleWhiteboard)?;
        }

        for binding_str in &self.toggle_blackboard {
            insert_binding(binding_str, Action::ToggleBlackboard)?;
        }

        for binding_str in &self.return_to_transparent {
            insert_binding(binding_str, Action::ReturnToTransparent)?;
        }

        for binding_str in &self.toggle_help {
            insert_binding(binding_str, Action::ToggleHelp)?;
        }

        for binding_str in &self.set_color_red {
            insert_binding(binding_str, Action::SetColorRed)?;
        }

        for binding_str in &self.set_color_green {
            insert_binding(binding_str, Action::SetColorGreen)?;
        }

        for binding_str in &self.set_color_blue {
            insert_binding(binding_str, Action::SetColorBlue)?;
        }

        for binding_str in &self.set_color_yellow {
            insert_binding(binding_str, Action::SetColorYellow)?;
        }

        for binding_str in &self.set_color_orange {
            insert_binding(binding_str, Action::SetColorOrange)?;
        }

        for binding_str in &self.set_color_pink {
            insert_binding(binding_str, Action::SetColorPink)?;
        }

        for binding_str in &self.set_color_white {
            insert_binding(binding_str, Action::SetColorWhite)?;
        }

        for binding_str in &self.set_color_black {
            insert_binding(binding_str, Action::SetColorBlack)?;
        }

        for binding_str in &self.capture_full_screen {
            insert_binding(binding_str, Action::CaptureFullScreen)?;
        }

        for binding_str in &self.capture_active_window {
            insert_binding(binding_str, Action::CaptureActiveWindow)?;
        }

        for binding_str in &self.capture_selection {
            insert_binding(binding_str, Action::CaptureSelection)?;
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

fn default_capture_full_screen() -> Vec<String> {
    vec!["Ctrl+Shift+P".to_string()]
}

fn default_capture_active_window() -> Vec<String> {
    vec!["Ctrl+Shift+O".to_string()]
}

fn default_capture_selection() -> Vec<String> {
    vec!["Ctrl+Shift+I".to_string()]
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
    fn test_parse_modifier_order_independence() {
        // Test that modifiers can appear in any order
        let binding1 = KeyBinding::parse("Ctrl+Shift+W").unwrap();
        let binding2 = KeyBinding::parse("Shift+Ctrl+W").unwrap();

        assert_eq!(binding1.key, "W");
        assert_eq!(binding2.key, "W");
        assert_eq!(binding1.ctrl, binding2.ctrl);
        assert_eq!(binding1.shift, binding2.shift);
        assert_eq!(binding1.alt, binding2.alt);
        assert!(binding1.ctrl);
        assert!(binding1.shift);

        // Test three modifiers in different orders
        let binding3 = KeyBinding::parse("Ctrl+Alt+Shift+W").unwrap();
        let binding4 = KeyBinding::parse("Shift+Alt+Ctrl+W").unwrap();
        let binding5 = KeyBinding::parse("Alt+Shift+Ctrl+W").unwrap();

        assert_eq!(binding3.key, "W");
        assert_eq!(binding4.key, "W");
        assert_eq!(binding5.key, "W");
        assert!(binding3.ctrl && binding3.shift && binding3.alt);
        assert!(binding4.ctrl && binding4.shift && binding4.alt);
        assert!(binding5.ctrl && binding5.shift && binding5.alt);
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

    #[test]
    fn test_duplicate_keybinding_detection() {
        // Create a config with duplicate keybindings
        let config = KeybindingsConfig {
            exit: vec!["Ctrl+Z".to_string()],
            undo: vec!["Ctrl+Z".to_string()],
            ..Default::default()
        };

        // This should fail with a duplicate error
        let result = config.build_action_map();
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("Duplicate keybinding"));
        assert!(err_msg.contains("Ctrl+Z"));
    }

    #[test]
    fn test_duplicate_with_different_modifier_order() {
        // Even with different modifier orders, these are the same keybinding
        let config = KeybindingsConfig {
            exit: vec!["Ctrl+Shift+W".to_string()],
            toggle_whiteboard: vec!["Shift+Ctrl+W".to_string()],
            ..Default::default()
        };

        // This should fail because they normalize to the same binding
        let result = config.build_action_map();
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("Duplicate keybinding"));
    }
}
