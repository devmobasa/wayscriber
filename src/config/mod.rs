//! Configuration file support for hyprmarker.
//!
//! This module handles loading and validating user settings from the configuration file
//! located at `~/.config/hyprmarker/config.toml`. Settings include drawing defaults,
//! arrow appearance, performance tuning, and UI preferences.
//!
//! If no config file exists, sensible defaults are used automatically.

pub mod enums;
pub mod types;

// Re-export commonly used types at module level
pub use enums::StatusPosition;
pub use types::{
    ArrowConfig, BoardConfig, DrawingConfig, HelpOverlayStyle, PerformanceConfig, StatusBarStyle,
    UiConfig,
};

// Re-export for public API (unused internally but part of public interface)
#[allow(unused_imports)]
pub use enums::ColorSpec;

use anyhow::{Context, Result};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Main configuration structure containing all user settings.
///
/// This is the root configuration type that gets deserialized from the TOML file.
/// All fields have sensible defaults and will use those if not specified in the config file.
///
/// # Example TOML
/// ```toml
/// [drawing]
/// default_color = "red"
/// default_thickness = 3.0
/// default_font_size = 32.0
///
/// [arrow]
/// length = 20.0
/// angle_degrees = 30.0
///
/// [performance]
/// buffer_count = 3
/// enable_vsync = true
///
/// [ui]
/// show_status_bar = true
/// status_bar_position = "bottom-left"
/// ```
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    /// Drawing tool defaults (color, thickness, font size)
    #[serde(default)]
    pub drawing: DrawingConfig,

    /// Arrow appearance settings
    #[serde(default)]
    pub arrow: ArrowConfig,

    /// Performance tuning options
    #[serde(default)]
    pub performance: PerformanceConfig,

    /// UI display preferences
    #[serde(default)]
    pub ui: UiConfig,

    /// Board mode settings (whiteboard/blackboard)
    #[serde(default)]
    pub board: BoardConfig,
}

impl Config {
    /// Validates and clamps all configuration values to acceptable ranges.
    ///
    /// This method ensures that user-provided config values won't cause undefined behavior
    /// or rendering issues. Invalid values are clamped to the nearest valid value and a
    /// warning is logged.
    ///
    /// Validated ranges:
    /// - `default_thickness`: 1.0 - 20.0
    /// - `default_font_size`: 8.0 - 72.0
    /// - `arrow.length`: 5.0 - 50.0
    /// - `arrow.angle_degrees`: 15.0 - 60.0
    /// - `buffer_count`: 2 - 4
    fn validate_and_clamp(&mut self) {
        // Thickness: 1.0 - 20.0
        if !(1.0..=20.0).contains(&self.drawing.default_thickness) {
            log::warn!(
                "Invalid default_thickness {:.1}, clamping to 1.0-20.0 range",
                self.drawing.default_thickness
            );
            self.drawing.default_thickness = self.drawing.default_thickness.clamp(1.0, 20.0);
        }

        // Font size: 8.0 - 72.0
        if !(8.0..=72.0).contains(&self.drawing.default_font_size) {
            log::warn!(
                "Invalid default_font_size {:.1}, clamping to 8.0-72.0 range",
                self.drawing.default_font_size
            );
            self.drawing.default_font_size = self.drawing.default_font_size.clamp(8.0, 72.0);
        }

        // Arrow length: 5.0 - 50.0
        if !(5.0..=50.0).contains(&self.arrow.length) {
            log::warn!(
                "Invalid arrow length {:.1}, clamping to 5.0-50.0 range",
                self.arrow.length
            );
            self.arrow.length = self.arrow.length.clamp(5.0, 50.0);
        }

        // Arrow angle: 15.0 - 60.0 degrees
        if !(15.0..=60.0).contains(&self.arrow.angle_degrees) {
            log::warn!(
                "Invalid arrow angle {:.1}°, clamping to 15.0-60.0° range",
                self.arrow.angle_degrees
            );
            self.arrow.angle_degrees = self.arrow.angle_degrees.clamp(15.0, 60.0);
        }

        // Buffer count: 2 - 4
        if !(2..=4).contains(&self.performance.buffer_count) {
            log::warn!(
                "Invalid buffer_count {}, clamping to 2-4 range",
                self.performance.buffer_count
            );
            self.performance.buffer_count = self.performance.buffer_count.clamp(2, 4);
        }

        // Validate font weight is reasonable
        let valid_weight = matches!(
            self.drawing.font_weight.to_lowercase().as_str(),
            "normal" | "bold" | "light" | "ultralight" | "heavy" | "ultrabold"
        ) || self
            .drawing
            .font_weight
            .parse::<u32>()
            .is_ok_and(|w| (100..=900).contains(&w));

        if !valid_weight {
            log::warn!(
                "Invalid font_weight '{}', falling back to 'bold'",
                self.drawing.font_weight
            );
            self.drawing.font_weight = "bold".to_string();
        }

        // Validate font style
        if !matches!(
            self.drawing.font_style.to_lowercase().as_str(),
            "normal" | "italic" | "oblique"
        ) {
            log::warn!(
                "Invalid font_style '{}', falling back to 'normal'",
                self.drawing.font_style
            );
            self.drawing.font_style = "normal".to_string();
        }

        // Validate board mode default
        if !matches!(
            self.board.default_mode.to_lowercase().as_str(),
            "transparent" | "whiteboard" | "blackboard"
        ) {
            log::warn!(
                "Invalid board default_mode '{}', falling back to 'transparent'",
                self.board.default_mode
            );
            self.board.default_mode = "transparent".to_string();
        }

        // Validate board color RGB values (0.0-1.0)
        for i in 0..3 {
            if !(0.0..=1.0).contains(&self.board.whiteboard_color[i]) {
                log::warn!(
                    "Invalid whiteboard_color[{}] = {:.3}, clamping to 0.0-1.0",
                    i,
                    self.board.whiteboard_color[i]
                );
                self.board.whiteboard_color[i] = self.board.whiteboard_color[i].clamp(0.0, 1.0);
            }
            if !(0.0..=1.0).contains(&self.board.blackboard_color[i]) {
                log::warn!(
                    "Invalid blackboard_color[{}] = {:.3}, clamping to 0.0-1.0",
                    i,
                    self.board.blackboard_color[i]
                );
                self.board.blackboard_color[i] = self.board.blackboard_color[i].clamp(0.0, 1.0);
            }
            if !(0.0..=1.0).contains(&self.board.whiteboard_pen_color[i]) {
                log::warn!(
                    "Invalid whiteboard_pen_color[{}] = {:.3}, clamping to 0.0-1.0",
                    i,
                    self.board.whiteboard_pen_color[i]
                );
                self.board.whiteboard_pen_color[i] =
                    self.board.whiteboard_pen_color[i].clamp(0.0, 1.0);
            }
            if !(0.0..=1.0).contains(&self.board.blackboard_pen_color[i]) {
                log::warn!(
                    "Invalid blackboard_pen_color[{}] = {:.3}, clamping to 0.0-1.0",
                    i,
                    self.board.blackboard_pen_color[i]
                );
                self.board.blackboard_pen_color[i] =
                    self.board.blackboard_pen_color[i].clamp(0.0, 1.0);
            }
        }
    }

    /// Returns the path to the configuration file.
    ///
    /// The config file is located at `~/.config/hyprmarker/config.toml`.
    ///
    /// # Errors
    /// Returns an error if the config directory cannot be determined (e.g., HOME not set).
    pub fn get_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("hyprmarker");

        Ok(config_dir.join("config.toml"))
    }

    /// Loads configuration from file, or returns defaults if not found.
    ///
    /// Attempts to read and parse the config file at `~/.config/hyprmarker/config.toml`.
    /// If the file doesn't exist, returns a Config with default values. All loaded values
    /// are validated and clamped to acceptable ranges.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The config directory path cannot be determined
    /// - The file exists but cannot be read
    /// - The file exists but contains invalid TOML syntax
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path()?;

        if !config_path.exists() {
            info!("Config file not found, using defaults");
            debug!("Expected config at: {}", config_path.display());
            return Ok(Self::default());
        }

        let config_str = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config from {}", config_path.display()))?;

        let mut config: Config = toml::from_str(&config_str)
            .with_context(|| format!("Failed to parse config from {}", config_path.display()))?;

        // Validate and clamp values to acceptable ranges
        config.validate_and_clamp();

        info!("Loaded config from {}", config_path.display());
        debug!("Config: {:?}", config);

        Ok(config)
    }

    /// Saves the current configuration to file.
    ///
    /// Serializes the config to TOML format and writes it to `~/.config/hyprmarker/config.toml`.
    /// Creates the parent directory if it doesn't exist. This method is kept for future use
    /// (e.g., runtime config editing).
    ///
    /// # Errors
    /// Returns an error if:
    /// - The config directory cannot be created
    /// - The config cannot be serialized to TOML
    /// - The file cannot be written
    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path()?;

        // Create directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let config_str = toml::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&config_path, config_str)
            .with_context(|| format!("Failed to write config to {}", config_path.display()))?;

        info!("Saved config to {}", config_path.display());
        Ok(())
    }

    /// Creates a default configuration file with documentation comments.
    ///
    /// Writes the example config from `config.example.toml` to the user's config directory.
    /// This method is kept for future use (e.g., `hyprmarker --init-config`).
    ///
    /// # Errors
    /// Returns an error if:
    /// - A config file already exists at the target path
    /// - The config directory cannot be created
    /// - The file cannot be written
    #[allow(dead_code)]
    pub fn create_default_file() -> Result<()> {
        let config_path = Self::get_config_path()?;

        if config_path.exists() {
            return Err(anyhow::anyhow!(
                "Config file already exists at {}",
                config_path.display()
            ));
        }

        // Create directory
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let default_config = include_str!("../../config.example.toml");
        fs::write(&config_path, default_config)?;

        info!("Created default config at {}", config_path.display());
        Ok(())
    }
}
