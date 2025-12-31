use crate::paths::config_dir;
use anyhow::{Context, Result};
use std::path::PathBuf;

pub(crate) const PRIMARY_CONFIG_DIR: &str = "wayscriber";

pub(crate) fn config_home_dir() -> Result<PathBuf> {
    config_dir().context("Could not find config directory")
}

pub(crate) fn primary_config_dir() -> Result<PathBuf> {
    config_home_dir().map(|dir| dir.join(PRIMARY_CONFIG_DIR))
}
