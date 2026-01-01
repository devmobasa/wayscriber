use super::Config;
use super::paths::primary_config_dir;
use crate::time_utils::{format_with_template, now_local};
use anyhow::{Context, Result, anyhow};
use log::{debug, info};
use std::fs;
use std::path::{Path, PathBuf};

/// Represents the source used to load configuration data.
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// Configuration file loaded from the Wayscriber config path.
    Primary,
    /// Defaults were used because no configuration file was found.
    Default,
}

/// Wrapper around [`Config`] that includes metadata about the load location.
#[derive(Debug)]
pub struct LoadedConfig {
    pub config: Config,
    pub source: ConfigSource,
}

impl Config {
    /// Returns the path to the configuration file.
    ///
    /// The config file is located at `~/.config/wayscriber/config.toml`.
    ///
    /// # Errors
    /// Returns an error if the config directory cannot be determined (e.g., HOME not set).
    pub fn get_config_path() -> Result<PathBuf> {
        Ok(primary_config_dir()?.join("config.toml"))
    }

    /// Determines the directory containing the active configuration file based on the source.
    pub fn config_directory_from_source(_source: &ConfigSource) -> Result<PathBuf> {
        let path = Self::get_config_path()?;
        path.parent()
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("Config path {} has no parent directory", path.display()))
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

        let (config_path, source) = if primary_path.exists() {
            (primary_path.clone(), ConfigSource::Primary)
        } else {
            info!("Config file not found, using defaults");
            debug!("Expected config at: {}", primary_path.display());
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
        let timestamp = format_with_template(now_local(), "%Y%m%d_%H%M%S");
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
            return Err(anyhow!(
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
