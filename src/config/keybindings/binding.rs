use std::fmt;

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

impl fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<&str> = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        parts.push(self.key.as_str());
        write!(f, "{}", parts.join("+"))
    }
}
