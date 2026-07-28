use super::Config;
use super::keybindings::KeybindingAuthorship;
use super::paths::primary_config_dir;
use super::validate::ConfigValidationReport;
use crate::durable_io::{AtomicWriteOptions, OverwriteMode};
use crate::time_utils::{format_with_template, now_local};
use anyhow::{Context, Result, anyhow};
use log::{debug, info};
use std::fs;
use std::path::{Path, PathBuf};

/// Represents the source used to load configuration data.
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// Configuration file loaded from the selected config path.
    Primary,
    /// Defaults were used because no configuration file was found.
    Default,
}

/// Wrapper around [`Config`] that includes metadata about the load location.
#[derive(Debug)]
pub struct LoadedConfig {
    pub config: Config,
    pub source: ConfigSource,
    /// What validation had to change in memory. Empty for an unvalidated load.
    pub validation: ConfigValidationReport,
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
        let mut loaded = Self::load_unvalidated()?;

        // Validate and clamp values to acceptable ranges.
        loaded.validation = loaded.config.validate_and_clamp();

        debug!("Config: {:?}", loaded.config);

        Ok(loaded)
    }

    /// Reads and deserializes the active config without validation.
    ///
    /// Binary-only repair workflows use this to fix an invalid subsection
    /// before normal validation. Ordinary consumers must use [`Self::load`].
    pub(crate) fn load_unvalidated() -> Result<LoadedConfig> {
        let primary_path = primary_config_dir()?.join("config.toml");

        let (config_path, source) = if primary_path.exists() {
            (primary_path.clone(), ConfigSource::Primary)
        } else {
            info!("Config file not found, using defaults");
            debug!("Expected config at: {}", primary_path.display());
            return Ok(LoadedConfig {
                config: Config::default(),
                source: ConfigSource::Default,
                validation: ConfigValidationReport::default(),
            });
        };

        let config = Self::read_unvalidated_from(&config_path)?;

        info!("Loaded config from {}", config_path.display());

        Ok(LoadedConfig {
            config,
            source,
            validation: ConfigValidationReport::default(),
        })
    }

    /// Reads a config file without validating it, recording which
    /// `[keybindings]` keys the file actually spells out.
    ///
    /// Serde cannot report that: it fills an omitted list with this build's
    /// default, and the result is indistinguishable from a list the user typed.
    /// The presence set is taken from the same text serde sees, so resolution
    /// can tell an authored shortcut from an offer (#293).
    fn read_unvalidated_from(config_path: &Path) -> Result<Self> {
        let config_str = fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read config from {}", config_path.display()))?;

        let mut config: Self = toml::from_str(&config_str)
            .with_context(|| format!("Failed to parse config from {}", config_path.display()))?;
        config.keybinding_authorship = KeybindingAuthorship::from_toml_source(&config_str);
        Ok(config)
    }

    /// Test-only convenience for exercising the revision-guarded document
    /// update path against the active config path.
    ///
    /// Not a production path, and there is no production equivalent: the
    /// configurator's explicit Save is the only code that writes the file, and
    /// it edits a draft rather than a closure over the loaded config.
    #[cfg(test)]
    pub(crate) fn update_file(update: impl FnOnce(&mut Self)) -> Result<()> {
        let config_path = Self::get_config_path()?;
        let document = super::ConfigDocument::load_from_path(&config_path)?;
        let mut config = document.config().clone();
        update(&mut config);
        document.save_with_backup(config)?;
        info!("Updated config at {}", config_path.display());
        Ok(())
    }
}

pub(super) fn prepare_config_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create config directory")?;
    }
    Ok(())
}

pub(super) fn create_config_backup(path: &Path) -> Result<PathBuf> {
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

pub(super) fn write_config_text_atomic(
    path: &Path,
    contents: &str,
    overwrite: OverwriteMode,
) -> Result<()> {
    let mut options = AtomicWriteOptions::user_config_file();
    options.overwrite = overwrite;
    crate::durable_io::write_text_atomic(path, contents, options)
        .with_context(|| format!("Failed to write config to {}", path.display()))
}
