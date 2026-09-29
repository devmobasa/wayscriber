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

    let (mut config, section_errors, authorship) = match deserialize(table.clone()) {
        Ok(config) => {
            let authorship = KeybindingAuthorship::from_toml_table(&table);
            (config, Vec::new(), authorship)
        }
        Err(full_error) => {
            let (pruned, section_errors) = prune_unmappable(table);
            // A shortcut list that was dropped runs on its default, which the
            // file does not describe: presence is taken from what survived.
            let authorship = KeybindingAuthorship::from_toml_table(&pruned);
            let config = deserialize(pruned)
                .with_context(|| format!("Failed to parse config: {}", full_error.message()))?;
            (config, section_errors, authorship)
        }
    };

    config.keybinding_authorship = if section_errors
        .iter()
        .any(|entry| entry.section == "keybindings")
    {
        // The whole section is running on shipped defaults, which the file
        // does not describe; presence in the source must not make those
        // defaults look authored.
        KeybindingAuthorship::default()
    } else {
        authorship
    };
    Ok(SalvagedConfig {
        config,
        section_errors,
    })
}

/// Tables with a hand-written `Deserialize` whose keys depend on each other,
/// so dropping one would change what the others mean. Matched on the last
/// path segment: `drawing.drag_tools` and each preset slot's `drag_tools`
/// share the type.
const OPAQUE_TABLE_KEYS: &[&str] = &["drag_tools"];

/// Removes the entries that fail to map, as deep as they can be told apart.
///
/// A failing table is split: each entry in it is probed on its own and only
/// the ones that fail are dropped, so one mistyped value costs that value
/// rather than its whole section. An entry is dropped whole instead when
/// splitting it would change its meaning:
/// - a list, including an array of tables, whose elements are positional;
/// - a table whose empty form does not map, which has required keys, so an
///   entry probed alone fails for reasons of its own;
/// - a table in [`OPAQUE_TABLE_KEYS`].
///
/// Every probe deserializes a whole [`Config`], so this costs something, but
/// only a file that fails to map as a whole ever reaches it.
fn prune_unmappable(table: toml::Table) -> (toml::Table, Vec<ConfigSectionError>) {
    let mut section_errors = Vec::new();
    let pruned = prune_table(&mut Vec::new(), table, &mut section_errors);
    (pruned, section_errors)
}

fn prune_table(
    path: &mut Vec<String>,
    table: toml::Table,
    section_errors: &mut Vec<ConfigSectionError>,
) -> toml::Table {
    let mut pruned = toml::Table::new();
    for (key, value) in table {
        path.push(key);
        if let Some(value) = prune_entry(path, value, section_errors) {
            pruned.insert(path.last().expect("pushed above").clone(), value);
        }
        path.pop();
    }
    pruned
}

/// The entry at `path` with its failing parts removed, or `None` when it
/// fails and cannot be split.
fn prune_entry(
    path: &mut Vec<String>,
    value: toml::Value,
    section_errors: &mut Vec<ConfigSectionError>,
) -> Option<toml::Value> {
    let Err(error) = probe(path, value.clone()) else {
        return Some(value);
    };

    if let toml::Value::Table(table) = value
        && can_split(path)
    {
        let mut entry_errors = Vec::new();
        let pruned = toml::Value::Table(prune_table(path, table, &mut entry_errors));
        // What maps entry by entry can still fail together; then the table
        // is reported as the one unit it turned out to be.
        if probe(path, pruned.clone()).is_ok() {
            section_errors.extend(entry_errors);
            return Some(pruned);
        }
    }

    section_errors.push(ConfigSectionError {
        section: path.join("."),
        error: error.message().to_string(),
    });
    None
}

fn can_split(path: &[String]) -> bool {
    let opaque = path
        .last()
        .is_some_and(|key| OPAQUE_TABLE_KEYS.contains(&key.as_str()));
    !opaque && probe(path, toml::Value::Table(toml::Table::new())).is_ok()
}

/// Whether `value` maps when it is the only thing at `path`.
///
/// Only a table whose empty form maps is ever split (see [`can_split`]), so
/// every table above `path` fills in its other keys with defaults around the
/// probe.
fn probe(path: &[String], value: toml::Value) -> Result<(), toml::de::Error> {
    let mut nested = value;
    for key in path.iter().rev() {
        let mut table = toml::Table::new();
        table.insert(key.clone(), nested);
        nested = toml::Value::Table(table);
    }
    let toml::Value::Table(root) = nested else {
        unreachable!("a probe always names at least one key");
    };
    root.try_into::<Config>().map(drop)
}
