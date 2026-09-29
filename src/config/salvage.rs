//! Partial recovery for a config whose TOML parses but whose values do not
//! all map onto [`Config`].
//!
//! The overlay's loader and the editors' document loader both deserialize
//! through here, so they agree on what a file means: the same values, and the
//! same entries running on defaults. When they disagreed, the overlay ran a
//! file with one section on defaults while the configurator offered to
//! replace all of it.

use anyhow::{Context, Result};

use super::Config;
use super::io::ConfigSectionError;
use super::keybindings::KeybindingAuthorship;

/// A configuration read from source text, and what had to fall back.
pub(super) struct SalvagedConfig {
    pub(super) config: Config,
    /// Entries that failed to deserialize and hold their defaults instead.
    /// Empty when the whole file mapped.
    pub(super) section_errors: Vec<ConfigSectionError>,
}

/// Deserializes `input`, keeping every entry that maps when the whole does
/// not.
///
/// `deserialize` turns a TOML table into a [`Config`]. It runs on the whole
/// table first and, only when that fails, once more on the table with the
/// failing entries removed, so a caller that records side information (the
/// editor collects ignored keys) must reset it at the start of each call.
///
/// A syntax error is not salvageable — there is no parsed document to salvage
/// from — and fails the call.
pub(super) fn deserialize_salvaging(
    input: &str,
    mut deserialize: impl FnMut(toml::Table) -> Result<Config, toml::de::Error>,
) -> Result<SalvagedConfig> {
    let table = input
        .parse::<toml::Table>()
        .context("Failed to parse TOML")?;
    let authorship = KeybindingAuthorship::from_toml_table(&table);

    let (mut config, section_errors) = match deserialize(table.clone()) {
        Ok(config) => (config, Vec::new()),
        Err(full_error) => {
            let (pruned, section_errors) = prune_unmappable(table);
            let config = deserialize(pruned)
                .with_context(|| format!("Failed to parse config: {}", full_error.message()))?;
            (config, section_errors)
        }
    };

    config.keybinding_authorship = if section_errors
        .iter()
        .any(|entry| entry.section == "keybindings")
    {
        // The section is running on shipped defaults, which the file does not
        // describe; presence in the source must not make those defaults look
        // authored.
        KeybindingAuthorship::default()
    } else {
        authorship
    };
    Ok(SalvagedConfig {
        config,
        section_errors,
    })
}

/// Removes the top-level entries that fail to map on their own.
///
/// Every section of [`Config`] is `#[serde(default)]`, so a table holding a
/// single entry is a complete probe for that entry.
fn prune_unmappable(table: toml::Table) -> (toml::Table, Vec<ConfigSectionError>) {
    let mut pruned = table.clone();
    let mut section_errors = Vec::new();
    for (key, value) in table {
        let mut probe = toml::Table::new();
        probe.insert(key.clone(), value);
        if let Err(err) = probe.try_into::<Config>() {
            section_errors.push(ConfigSectionError {
                section: key.clone(),
                error: err.message().to_string(),
            });
            pruned.remove(&key);
        }
    }
    (pruned, section_errors)
}
