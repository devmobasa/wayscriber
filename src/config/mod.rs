//! Configuration file support for wayscriber.
//!
//! This module handles loading and validating user settings from the configuration file
//! located at `~/.config/wayscriber/config.toml`. Settings include drawing defaults,
//! arrow appearance, performance tuning, and UI preferences.
//!
//! If no config file exists, sensible defaults are used automatically.

pub mod enums;
pub mod keybindings;
pub mod migration;
pub mod types;

// Re-export commonly used types at module level
pub use enums::StatusPosition;
pub use keybindings::{Action, KeyBinding, KeybindingsConfig};
pub use migration::{MigrationActions, MigrationReport, migrate_config};
pub use types::{
    ArrowConfig, BoardConfig, CaptureConfig, DrawingConfig, HelpOverlayStyle, PerformanceConfig,
    StatusBarStyle, UiConfig,
};

// Re-export for public API (unused internally but part of public interface)
#[allow(unused_imports)]
pub use enums::ColorSpec;

use crate::legacy;
use anyhow::{Context, Result};
use chrono::Local;
use log::{debug, info, warn};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

const PRIMARY_CONFIG_DIR: &str = "wayscriber";
const LEGACY_CONFIG_DIR: &str = "hyprmarker";

static USING_LEGACY_CONFIG: AtomicBool = AtomicBool::new(false);

/// Represents the source used to load configuration data.
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// Configuration file loaded from the current Wayscriber path.
    Primary,
    /// Configuration file loaded from the legacy hyprmarker path.
    Legacy(PathBuf),
    /// Defaults were used because no configuration file was found.
    Default,
}

/// Wrapper around [`Config`] that includes metadata about the load location.
#[derive(Debug)]
pub struct LoadedConfig {
    pub config: Config,
    pub source: ConfigSource,
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::ColorSpec;
    use std::fs;
    use std::path::Path;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn with_temp_config_home<F, T>(f: F) -> T
    where
        F: FnOnce(&Path) -> T,
    {
        let _guard = ENV_MUTEX.lock().unwrap();
        let temp = TempDir::new().expect("tempdir");
        let original = std::env::var_os("XDG_CONFIG_HOME");
        // SAFETY: tests run single-threaded via the mutex above; restoring the previous
        // value prevents leaking the override to other tests.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", temp.path()); }
        let result = f(temp.path());
        match original {
            Some(value) => unsafe { std::env::set_var("XDG_CONFIG_HOME", value) },
            None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
        }
        result
    }

    #[test]
    fn load_prefers_primary_directory() {
        with_temp_config_home(|config_root| {
            let primary_dir = config_root.join(PRIMARY_CONFIG_DIR);
            fs::create_dir_all(&primary_dir).unwrap();
            fs::write(
                primary_dir.join("config.toml"),
                "[drawing]\ndefault_color = 'red'\n",
            )
            .unwrap();

            let loaded = Config::load().expect("load succeeds");
            assert!(matches!(loaded.source, ConfigSource::Primary));
        });
    }

    #[test]
    fn load_falls_back_to_legacy_directory() {
        with_temp_config_home(|config_root| {
            let legacy_dir = config_root.join(LEGACY_CONFIG_DIR);
            fs::create_dir_all(&legacy_dir).unwrap();
            fs::write(
                legacy_dir.join("config.toml"),
                "[drawing]\ndefault_color = 'blue'\n",
            )
            .unwrap();

            let loaded = Config::load().expect("load succeeds");
            assert!(matches!(loaded.source, ConfigSource::Legacy(_)));
            assert!(matches!(
                loaded.config.drawing.default_color,
                ColorSpec::Name(ref color) if color == "blue"
            ));
        });
    }
}

pub(super) fn config_home_dir() -> Result<PathBuf> {
    dirs::config_dir().context("Could not find config directory")
}

pub(super) fn primary_config_dir() -> Result<PathBuf> {
    config_home_dir().map(|dir| dir.join(PRIMARY_CONFIG_DIR))
}

pub(super) fn legacy_config_dir() -> Result<PathBuf> {
    config_home_dir().map(|dir| dir.join(LEGACY_CONFIG_DIR))
}

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
///
/// [keybindings]
/// exit = ["Escape", "Ctrl+Q"]
/// undo = ["Ctrl+Z"]
/// ```
#[derive(Debug, Serialize, Deserialize, Default, JsonSchema)]
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

    /// Keybinding customization
    #[serde(default)]
    pub keybindings: KeybindingsConfig,

    /// Screenshot capture settings
    #[serde(default)]
    pub capture: CaptureConfig,
}

impl Config {
    /// Generates a JSON Schema describing the full configuration surface.
    #[allow(dead_code)]
    pub fn json_schema() -> Value {
        serde_json::to_value(schema_for!(Config))
            .expect("serializing configuration schema should succeed")
    }

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
    pub fn validate_and_clamp(&mut self) {
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

        // Validate keybindings (try to build action map to catch parse errors)
        if let Err(e) = self.keybindings.build_action_map() {
            log::warn!("Invalid keybinding configuration: {}. Using defaults.", e);
            self.keybindings = KeybindingsConfig::default();
        }
    }

    /// Returns the path to the configuration file.
    ///
    /// The config file is located at `~/.config/wayscriber/config.toml`.
    ///
    /// # Errors
    /// Returns an error if the config directory cannot be determined (e.g., HOME not set).
    pub fn get_config_path() -> Result<PathBuf> {
        Ok(primary_config_dir()?.join("config.toml"))
    }

    /// Loads configuration from file, or returns defaults if not found.
    ///
    /// Attempts to read and parse the config file at `~/.config/wayscriber/config.toml`.
    /// If the file doesn't exist, returns a Config with default values. All loaded values
    /// are validated and clamped to acceptable ranges.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The config directory path cannot be determined
    /// - The file exists but cannot be read
    /// - The file exists but contains invalid TOML syntax
    pub fn load() -> Result<LoadedConfig> {
        let primary_path = primary_config_dir()?.join("config.toml");
        let legacy_path = legacy_config_dir()?.join("config.toml");

        let (config_path, source) = if primary_path.exists() {
            (primary_path.clone(), ConfigSource::Primary)
        } else if legacy_path.exists() {
            (
                legacy_path.clone(),
                ConfigSource::Legacy(legacy_path.clone()),
            )
        } else {
            USING_LEGACY_CONFIG.store(false, Ordering::Relaxed);
            info!("Config file not found, using defaults");
            debug!("Expected config at: {}", primary_path.display());
            debug!("Checked legacy config at: {}", legacy_path.display());
            return Ok(LoadedConfig {
                config: Config::default(),
                source: ConfigSource::Default,
            });
        };

        let config_str = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config from {}", config_path.display()))?;

        let mut config: Config = toml::from_str(&config_str)
            .with_context(|| format!("Failed to parse config from {}", config_path.display()))?;

        // Validate and clamp values to acceptable ranges
        config.validate_and_clamp();

        match &source {
            ConfigSource::Legacy(path) => {
                USING_LEGACY_CONFIG.store(true, Ordering::Relaxed);
                if !legacy::warnings_suppressed() {
                    warn!(
                        "Loading configuration from legacy hyprmarker path: {}",
                        path.display()
                    );
                    warn!(
                        "Run `wayscriber --migrate-config` to copy settings to ~/.config/wayscriber/."
                    );
                }
            }
            ConfigSource::Primary => {
                USING_LEGACY_CONFIG.store(false, Ordering::Relaxed);
            }
            ConfigSource::Default => {
                USING_LEGACY_CONFIG.store(false, Ordering::Relaxed);
            }
        }

        info!("Loaded config from {}", config_path.display());
        debug!("Config: {:?}", config);

        Ok(LoadedConfig { config, source })
    }

    fn write_config(&self, create_backup: bool) -> Result<Option<PathBuf>> {
        let config_path = Self::get_config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let backup_path = if create_backup && config_path.exists() {
            Some(Self::create_backup(&config_path)?)
        } else {
            None
        };

        let config_str = toml::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&config_path, config_str)
            .with_context(|| format!("Failed to write config to {}", config_path.display()))?;

        if let Some(path) = &backup_path {
            info!(
                "Saved config to {} (backup at {})",
                config_path.display(),
                path.display()
            );
        } else {
            info!("Saved config to {}", config_path.display());
        }

        Ok(backup_path)
    }

    /// Saves the current configuration to disk without creating a backup.
    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        self.write_config(false)?;
        Ok(())
    }

    /// Saves the current configuration and creates a timestamped `.bak` copy when overwriting
    /// an existing file. Returns the backup path if one was created.
    #[allow(dead_code)]
    pub fn save_with_backup(&self) -> Result<Option<PathBuf>> {
        self.write_config(true)
    }

    fn create_backup(path: &Path) -> Result<PathBuf> {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let filename = match path.file_name().and_then(|name| name.to_str()) {
            Some(name) => format!("{name}.{}.bak", timestamp),
            None => format!("config.toml.{}.bak", timestamp),
        };

        let backup_path = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(filename);

        fs::copy(path, &backup_path).with_context(|| {
            format!(
                "Failed to create config backup from {} to {}",
                path.display(),
                backup_path.display()
            )
        })?;

        Ok(backup_path)
    }

    /// Creates a default configuration file with documentation comments.
    ///
    /// Writes the example config from `config.example.toml` to the user's config directory.
    /// This method is kept for future use (e.g., `wayscriber --init-config`).
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
