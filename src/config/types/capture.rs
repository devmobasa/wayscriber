use serde::{Deserialize, Serialize};

use crate::config::enums::RegionPicker;

/// Region screenshot picker configuration.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionCaptureConfig {
    /// Region selection frontend.
    #[serde(default)]
    pub picker: RegionPicker,

    /// Show pointer coordinates and the selected export size.
    #[serde(default = "default_region_show_size_readout")]
    pub show_size_readout: bool,

    /// Show a magnified pixel loupe while selecting a region.
    ///
    /// The setting is persisted for the picker; the loupe renderer may be
    /// unavailable in builds that do not yet consume it.
    #[serde(default = "default_region_show_loupe")]
    pub show_loupe: bool,

    /// Show the short picker hotkey legend until the first drag.
    #[serde(default = "default_region_show_legend")]
    pub show_legend: bool,
}

impl Default for RegionCaptureConfig {
    fn default() -> Self {
        Self {
            picker: RegionPicker::default(),
            show_size_readout: default_region_show_size_readout(),
            show_loupe: default_region_show_loupe(),
            show_legend: default_region_show_legend(),
        }
    }
}

/// Screenshot capture configuration.
///
/// Controls the behavior of screenshot capture features including file saving,
/// clipboard integration, and capture shortcuts.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Composite the active board's committed drawings into full-screen and
    /// region screenshot exports.
    #[serde(default = "default_capture_include_drawings")]
    pub include_drawings: bool,

    /// Exit the overlay after any capture completes (forces exit for all capture types).
    /// When false, clipboard-only captures still auto-exit by default.
    #[serde(default = "default_capture_exit_after")]
    pub exit_after_capture: bool,

    /// Tesseract languages used by `Copy text from screen`, in Tesseract's
    /// plus-separated form (for example `eng` or `eng+deu`). The matching
    /// Tesseract language packages must be installed.
    #[serde(default = "default_capture_ocr_languages")]
    pub ocr_languages: String,

    /// Region screenshot picker behavior and chrome.
    #[serde(default)]
    pub region: RegionCaptureConfig,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            enabled: default_capture_enabled(),
            save_directory: default_capture_directory(),
            filename_template: default_capture_filename(),
            format: default_capture_format(),
            copy_to_clipboard: default_capture_clipboard(),
            include_drawings: default_capture_include_drawings(),
            exit_after_capture: default_capture_exit_after(),
            ocr_languages: default_capture_ocr_languages(),
            region: RegionCaptureConfig::default(),
        }
    }
}

impl CaptureConfig {
    /// The configured OCR languages, or the default when the authored value is
    /// unusable. Recognition never passes an unvalidated string to the engine.
    pub fn resolved_ocr_languages(&self) -> String {
        match validate_ocr_languages(&self.ocr_languages) {
            Ok(languages) => languages,
            Err(reason) => {
                log::warn!(
                    "Ignoring capture.ocr_languages {:?}: {reason}; using {}",
                    self.ocr_languages,
                    DEFAULT_OCR_LANGUAGES
                );
                DEFAULT_OCR_LANGUAGES.to_string()
            }
        }
    }
}

pub const DEFAULT_OCR_LANGUAGES: &str = "eng";

/// Normalize a Tesseract language argument, rejecting anything that could
/// change how the engine is invoked.
///
/// Accepts the plus-separated form Tesseract itself uses (`eng`, `eng+deu`).
/// Tokens are restricted to the character set real language codes use, so a
/// value can never carry a path, an option, or shell syntax into the argument
/// vector.
pub fn validate_ocr_languages(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("value is empty".to_string());
    }
    let tokens: Vec<&str> = trimmed.split('+').collect();
    for token in &tokens {
        if token.is_empty() {
            return Err("value has an empty language between '+' separators".to_string());
        }
        if token.starts_with('-') {
            return Err(format!("language {token:?} looks like a command option"));
        }
        if !token
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        {
            return Err(format!(
                "language {token:?} may only contain letters, digits, '_' and '-'"
            ));
        }
    }
    Ok(tokens.join("+"))
}

/// Filename templates must expand to a file name inside the save directory.
pub fn validate_filename_template(template: &str) -> Result<(), String> {
    let trimmed = template.trim();
    if trimmed.is_empty() {
        return Err("value is empty".to_string());
    }
    if !crate::paths::is_single_path_component(trimmed) {
        return Err("must be a single file name, not a path".to_string());
    }
    Ok(())
}

/// Screenshot save format. PDF exports use a separate extension at save time.
pub fn validate_capture_format(format: &str) -> Result<String, String> {
    let normalized = format.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "png" | "jpg" | "jpeg" => Ok(normalized),
        _ => Err("must be png, jpg, or jpeg".to_string()),
    }
}

fn default_capture_ocr_languages() -> String {
    DEFAULT_OCR_LANGUAGES.to_string()
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

fn default_capture_include_drawings() -> bool {
    true
}

fn default_capture_exit_after() -> bool {
    false
}

fn default_region_show_size_readout() -> bool {
    true
}

fn default_region_show_loupe() -> bool {
    false
}

fn default_region_show_legend() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ocr_language_is_english() {
        assert_eq!(CaptureConfig::default().ocr_languages, "eng");
        assert_eq!(CaptureConfig::default().resolved_ocr_languages(), "eng");
    }

    #[test]
    fn region_picker_defaults_match_the_documented_native_experience() {
        let region = RegionCaptureConfig::default();

        assert_eq!(region.picker, RegionPicker::Native);
        assert!(region.show_size_readout);
        assert!(!region.show_loupe);
        assert!(region.show_legend);
    }

    #[test]
    fn plus_separated_languages_round_trip_after_trimming() {
        assert_eq!(validate_ocr_languages("  eng+deu \n").unwrap(), "eng+deu");
        assert_eq!(validate_ocr_languages("chi_sim").unwrap(), "chi_sim");
        assert_eq!(validate_ocr_languages("eng").unwrap(), "eng");
    }

    #[test]
    fn unsafe_or_empty_values_are_rejected() {
        for value in [
            "",
            "   ",
            "eng+",
            "+eng",
            "eng deu",
            "../tessdata/eng",
            "/usr/share/eng",
            "eng;rm -rf /",
            "eng$(id)",
            "--psm",
            "eng\0",
        ] {
            assert!(
                validate_ocr_languages(value).is_err(),
                "{value:?} must be rejected"
            );
        }
    }

    #[test]
    fn an_invalid_authored_value_falls_back_to_the_default() {
        let config = CaptureConfig {
            ocr_languages: "../evil".to_string(),
            ..CaptureConfig::default()
        };

        assert_eq!(config.resolved_ocr_languages(), DEFAULT_OCR_LANGUAGES);
    }

    #[test]
    fn filename_template_must_be_a_single_component() {
        validate_filename_template("screenshot_%Y-%m-%d_%H%M%S").unwrap();
        for template in ["", "  ", "../evil", "foo/bar", "/tmp/x"] {
            assert!(
                validate_filename_template(template).is_err(),
                "{template:?} must be rejected"
            );
        }
    }

    #[test]
    fn capture_format_is_allowlisted() {
        assert_eq!(validate_capture_format(" PNG ").unwrap(), "png");
        assert_eq!(validate_capture_format("jpeg").unwrap(), "jpeg");
        assert!(validate_capture_format("pdf").is_err());
        assert!(validate_capture_format("png/../../x").is_err());
        assert!(validate_capture_format("exe").is_err());
    }
}
