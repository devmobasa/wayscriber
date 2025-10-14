//! Font descriptor for text rendering.

/// Font configuration for text rendering.
///
/// Describes which font to use, including family name, weight, and style.
/// This descriptor is passed through the rendering pipeline to ensure
/// consistent font usage across preview and finalized text.
#[derive(Debug, Clone)]
pub struct FontDescriptor {
    /// Font family name (e.g., "Sans", "Monospace", "JetBrains Mono")
    /// Reference installed system fonts by name
    pub family: String,

    /// Font weight (e.g., "normal", "bold", "light" or numeric 100-900)
    pub weight: String,

    /// Font style (e.g., "normal", "italic", "oblique")
    pub style: String,
}

impl FontDescriptor {
    /// Creates a new font descriptor with the specified parameters.
    pub fn new(family: String, weight: String, style: String) -> Self {
        Self {
            family,
            weight,
            style,
        }
    }

    /// Creates a default font descriptor matching the current hardcoded behavior.
    #[allow(dead_code)]
    pub fn default() -> Self {
        Self {
            family: "Sans".to_string(),
            weight: "bold".to_string(),
            style: "normal".to_string(),
        }
    }

    /// Converts this font descriptor to a Pango font description string.
    ///
    /// Format: "Family Style Weight Size"
    /// Example: "Sans Bold 32" or "Monospace Italic 24"
    pub fn to_pango_string(&self, size: f64) -> String {
        let mut parts = vec![self.family.clone()];

        // Add style if not normal
        if self.style.to_lowercase() != "normal" {
            parts.push(capitalize_first(&self.style));
        }

        // Add weight if not normal
        if self.weight.to_lowercase() != "normal" {
            parts.push(capitalize_first(&self.weight));
        }

        // Add size
        parts.push(format!("{}", size.round() as i32));

        parts.join(" ")
    }
}

/// Capitalizes the first letter of a string.
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pango_string_default() {
        let font = FontDescriptor::default();
        assert_eq!(font.to_pango_string(32.0), "Sans Bold 32");
    }

    #[test]
    fn test_pango_string_italic() {
        let font = FontDescriptor::new(
            "Monospace".to_string(),
            "normal".to_string(),
            "italic".to_string(),
        );
        assert_eq!(font.to_pango_string(24.0), "Monospace Italic 24");
    }

    #[test]
    fn test_pango_string_custom() {
        let font = FontDescriptor::new(
            "JetBrains Mono".to_string(),
            "light".to_string(),
            "normal".to_string(),
        );
        assert_eq!(font.to_pango_string(16.0), "JetBrains Mono Light 16");
    }
}
