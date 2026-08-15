use super::ColorSpec;
use super::action_meta::action_label;
use super::keybindings::{Action, KeybindingAuthorship, ShortcutTrigger};
use super::paths::primary_config_dir;
use super::types::{PRESET_SLOTS_MAX, ToolPresetConfig};
use super::validate::ConfigValidationReport;
use super::{Config, ConfigDocument};
use crate::draw::Color;
use crate::durable_io::{
    AtomicWriteOptions, DestinationExpectation, DurableIoError, FileIdentity, OverwriteMode,
    SymlinkPolicy,
};
use crate::time_utils::{format_with_template, now_local};
use anyhow::{Context, Result, anyhow, bail};
use log::{debug, info};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Config files larger than this are refused so a truncated or hostile
/// `config.toml` cannot be pulled entirely into memory.
pub(crate) const MAX_CONFIG_FILE_BYTES: u64 = 2 * 1024 * 1024;

pub(crate) fn ensure_config_file_size(len: u64, path: &Path) -> Result<()> {
    if len > MAX_CONFIG_FILE_BYTES {
        bail!(
            "Config file {} is {len} bytes; the maximum is {MAX_CONFIG_FILE_BYTES}",
            path.display()
        );
    }
    Ok(())
}

/// Represents the source used to load configuration data.
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// Configuration file loaded from the selected config path.
    Primary,
    /// Defaults were used because no configuration file was found.
    Default,
}

/// A top-level config entry the load could not understand and replaced with
/// its defaults for this session. The file keeps the authored value.
#[derive(Debug, Clone)]
pub struct ConfigSectionError {
    /// The top-level key, e.g. `ui` for `[ui]` or `config_revision`.
    pub section: String,
    /// The deserialization error, without spans (the value was re-checked from
    /// the parsed document, not the source text).
    pub error: String,
}

impl std::fmt::Display for ConfigSectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "[{}] {}", self.section, self.error)
    }
}

/// Wrapper around [`Config`] that includes metadata about the load location.
#[derive(Debug)]
pub struct LoadedConfig {
    pub config: Config,
    pub source: ConfigSource,
    /// What validation had to change in memory. Empty for an unvalidated load.
    pub validation: ConfigValidationReport,
    /// Top-level entries that failed to deserialize and are running on
    /// defaults for this session. The caller is expected to show these rather
    /// than let them disappear into the log: before this existed, one bad
    /// value silently cost the user every customization in the file.
    pub section_errors: Vec<ConfigSectionError>,
}

impl LoadedConfig {
    /// Whether this top-level entry failed to deserialize and is running on
    /// defaults. Consumers whose behavior must not silently degrade when
    /// their section is unreadable — destructive commands that would act on
    /// default paths, or policies that fail closed — check this instead of
    /// trusting the defaulted value.
    pub fn section_failed(&self, section: &str) -> bool {
        self.section_errors
            .iter()
            .any(|entry| entry.section == section)
    }
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
                section_errors: Vec::new(),
            });
        };

        let (config, section_errors) = Self::read_unvalidated_from(&config_path)?;

        info!("Loaded config from {}", config_path.display());

        Ok(LoadedConfig {
            config,
            source,
            validation: ConfigValidationReport::default(),
            section_errors,
        })
    }

    /// Reads a config file without validating it, recording which
    /// `[keybindings]` keys the file actually spells out.
    ///
    /// Serde cannot report that: it fills an omitted list with this build's
    /// default, and the result is indistinguishable from a list the user typed.
    /// The presence set is taken from the same text serde sees, so resolution
    /// can tell an authored shortcut from an offer (#293).
    fn read_unvalidated_from(config_path: &Path) -> Result<(Self, Vec<ConfigSectionError>)> {
        let metadata = fs::metadata(config_path)
            .with_context(|| format!("Failed to read config from {}", config_path.display()))?;
        ensure_config_file_size(metadata.len(), config_path)?;
        let config_str = fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read config from {}", config_path.display()))?;
        ensure_config_file_size(config_str.len() as u64, config_path)?;

        let (mut config, section_errors) = match toml::from_str::<Self>(&config_str) {
            Ok(config) => (config, Vec::new()),
            // A mapping error in one entry must not cost the session the whole
            // file: re-parse per top-level entry, keep everything that maps,
            // and report what had to fall back to defaults. A syntax error is
            // different — there is no parsed document to salvage from — and
            // still fails the load.
            Err(parse_err) => {
                let table = toml::from_str::<toml::Table>(&config_str).with_context(|| {
                    format!("Failed to parse config from {}", config_path.display())
                })?;
                Self::salvage_sections(table, &parse_err)?
            }
        };
        config.keybinding_authorship = if section_errors
            .iter()
            .any(|entry| entry.section == "keybindings")
        {
            // The section is running on shipped defaults, which the file
            // does not describe; presence in the source must not make
            // those defaults look authored.
            KeybindingAuthorship::default()
        } else {
            KeybindingAuthorship::from_toml_source(&config_str)
        };
        Ok((config, section_errors))
    }

    /// Rebuilds a config from a parsed document one top-level entry at a time,
    /// dropping only the entries that fail to map.
    ///
    /// Every section of [`Config`] is `#[serde(default)]`, so a table holding
    /// a single entry is a complete probe for that entry. `full_error` is the
    /// error from the whole-file parse, kept for the (theoretically
    /// impossible) case where every entry maps individually but the pruned
    /// document still fails.
    fn salvage_sections(
        table: toml::Table,
        full_error: &toml::de::Error,
    ) -> Result<(Self, Vec<ConfigSectionError>)> {
        let mut pruned = table.clone();
        let mut section_errors = Vec::new();
        for (key, value) in &table {
            let mut probe = toml::Table::new();
            probe.insert(key.clone(), value.clone());
            if let Err(err) = probe.try_into::<Self>() {
                section_errors.push(ConfigSectionError {
                    section: key.clone(),
                    error: err.message().to_string(),
                });
                pruned.remove(key);
            }
        }
        let config = pruned
            .try_into::<Self>()
            .with_context(|| format!("Failed to parse config: {}", full_error.message()))?;
        Ok((config, section_errors))
    }

    /// Test-only convenience for exercising the revision-guarded document
    /// update path against the active config path.
    ///
    /// Not a production path, and there is no production equivalent: what
    /// writes the file is the configurator's explicit Save, which edits a draft,
    /// and the narrow editors below, which each set one key by name — neither
    /// runs a caller's closure over the loaded config.
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

/// Whether a narrow edit had anything to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigEditWrite {
    /// The file now carries the edit, and its previous contents are in the
    /// backup beside it.
    Wrote,
    /// The file already resolved to the requested value, so there was no delta
    /// to write. Nothing was touched and no backup was made — the caller must
    /// say that rather than claim it just made the change durable.
    AlreadyCurrent,
}

/// What a successful narrow edit left behind.
#[derive(Debug)]
pub struct ConfigEditOutcome {
    /// The timestamped copy of the previous contents, or `None` when the write
    /// created the file — or when there was nothing to write.
    pub backup_path: Option<PathBuf>,
    /// Whether the file changed at all.
    pub write: ConfigEditWrite,
}

impl ConfigEditOutcome {
    /// A no-delta edit: the file already said this.
    fn already_current() -> Self {
        Self {
            backup_path: None,
            write: ConfigEditWrite::AlreadyCurrent,
        }
    }
}

/// The shortcut the edit asked for belongs to another action in the file as it
/// stands now.
///
/// The overlay checks for conflicts against the keymap this run loaded, which
/// can be older than the file: another window's edit, the configurator, or a
/// hand edit may have given the chord away since. Writing anyway would hand the
/// merge gate a list validation then drops against the newer claimant, so the
/// edit is refused before anything is written, and the caller names the owner.
#[derive(Debug)]
pub struct ShortcutClaimedOnDisk {
    pub binding: String,
    pub claimed_by: Action,
}

impl std::fmt::Display for ShortcutClaimedOnDisk {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} is already assigned to {} in config.toml",
            self.binding,
            action_label(self.claimed_by)
        )
    }
}

impl std::error::Error for ShortcutClaimedOnDisk {}

/// The write landed, but the bytes it wrote do not parse back to the requested
/// value.
///
/// The check is against the document parsed from the merge output — the text
/// that was just written — not a fresh read of the file, so it catches a value
/// the save's own validation declined on the way out rather than one a later
/// writer replaced.
///
/// Reported apart from a failed save because the difference is what the user
/// can act on: the file *did* change, so telling them it did not would send
/// them looking in the wrong place.
#[derive(Debug)]
pub struct ConfigEditNotReadBack {
    /// The gesture, as the caller names it in its own wording.
    pub what: String,
    /// The file that was written and did not read back with the value.
    pub path: PathBuf,
}

impl std::fmt::Display for ConfigEditNotReadBack {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} was written to {}, but the file does not read back with the requested value",
            self.what,
            self.path.display()
        )
    }
}

impl std::error::Error for ConfigEditNotReadBack {}

/// The palette no longer has the slot a recolor was accepted for.
///
/// Not a failure of the write: the configurator (or a hand edit) shortened
/// `drawing.quick_colors` while the picker was open, so there is nothing to
/// write to. The caller reports the situation rather than a save error.
#[derive(Debug)]
pub struct QuickColorSlotMissing {
    pub index: usize,
}

impl std::fmt::Display for QuickColorSlotMissing {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "quick color slot {} is no longer in config.toml",
            self.index + 1
        )
    }
}

impl std::error::Error for QuickColorSlotMissing {}

/// The substring a save uses to report that the file moved under a loaded
/// document.
///
/// Two places produce it: `ensure_source_unchanged`, where the document
/// compares what it loaded against what the path holds now, and
/// [`write_config_text_atomic`], where the rename finds a different file than
/// the one those comparisons were about. Both are already pinned by the
/// document suite; `a_stale_document_is_recognised_and_retried` below re-checks
/// the coupling against a real stale save rather than trusting it.
const STALE_SOURCE_MARKER: &str = "changed on disk";

pub(crate) fn is_stale_source_error(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.to_string().contains(STALE_SOURCE_MARKER))
}

/// The shared shape of every narrow config editor.
///
/// These are the explicit user edit actions the overlay is allowed to write for
/// — one per gesture — and each one rewrites its own key and nothing else,
/// after copying the previous contents to a timestamped `.bak`.
///
/// **Why only that key can move.** The base is [`ConfigDocument::config`], the
/// same validated configuration the merge gate is handed as `previous`. The one
/// value `apply` sets is therefore the *only* difference between `previous` and
/// `updated`, and the gate writes differences. `save_with_backup` does re-run
/// `validate_and_clamp` on `updated`, which is safe precisely because of that
/// construction: `document.config()` is already the output of the same pass
/// (`parse_typed_config` runs it at load), and every step of it is idempotent —
/// clamps re-clamp to the same number, resolution finds no conflicts left to
/// arbitrate, dropped text stays dropped — so re-running it moves nothing else.
/// Basing `updated` on [`ConfigDocument::authored_config`] instead would mean
/// building the write from values the user never saw the loader change.
///
/// Note what that argument rests on, because it is not observable from the
/// outside: because `save_with_backup` re-validates, the two bases *currently*
/// converge, and swapping them changes no file this suite can produce. The
/// choice of `config()` is what keeps the one-key property from depending on
/// that coincidence — it holds even if the save stops re-validating or a
/// non-idempotent validation step appears. Two things defend it in place of a
/// behavioural test: `document_config_is_a_fixed_point_of_validation` below,
/// which fails the moment validation stops being idempotent, and the
/// `authored_config()` check in `tools/check-config-writers.py`.
///
/// `verify` runs afterwards against the document the save parsed from the bytes
/// it wrote — the merge output, not a re-read of the file — so a value that
/// validation declined on the way out is reported instead of being assumed
/// durable. A later writer replacing the file is a different situation, and one
/// this check is deliberately not about: the edit did land.
///
/// An edit whose value the file already resolves to is not written at all. The
/// merge gate would produce the same bytes anyway, but "the same bytes" is not
/// the same claim: a caller that hears `Wrote` for a save that had nothing to
/// save tells the user it just pinned something down, and spends a backup on it.
fn edit_one_config_key(
    path: &Path,
    what: &str,
    apply: &dyn Fn(&mut Config) -> Result<()>,
    verify: &dyn Fn(&Config) -> bool,
) -> Result<ConfigEditOutcome> {
    let (document, parse_failure) = ConfigDocument::load_for_editing_from_path(path)?;
    // A repair draft is built from built-in defaults, so saving one rewrites
    // the whole file. That is a decision the configurator asks the user to make
    // explicitly, with the damage on screen; a single-key edit must never make
    // it for them.
    if let Some(failure) = parse_failure {
        bail!(
            "{what} not saved: {} could not be parsed ({failure}). Repair it in the configurator.",
            path.display()
        );
    }

    let mut updated = document.config().clone();
    apply(&mut updated)?;
    // The same pass `save_with_backup` runs on what it is handed, so the
    // comparison below sees exactly the values the merge gate would diff.
    updated.validate_and_clamp();

    if config_value_text(&updated)? == config_value_text(document.config())? {
        if !verify(document.config()) {
            bail!(
                "{what} not saved: {} does not read back with the requested value",
                path.display()
            );
        }
        debug!(
            "{what} already matches {}; nothing to write",
            path.display()
        );
        return Ok(ConfigEditOutcome::already_current());
    }

    let outcome = document.save_with_backup(updated)?;

    if !verify(outcome.document().config()) {
        return Err(anyhow!(ConfigEditNotReadBack {
            what: what.to_string(),
            path: path.to_path_buf(),
        }));
    }

    let backup_path = outcome.backup_path().map(Path::to_path_buf);
    info!("Wrote {what} to {}", path.display());
    Ok(ConfigEditOutcome {
        backup_path,
        write: ConfigEditWrite::Wrote,
    })
}

/// The configuration's values as the merge gate compares them.
///
/// `Config` has no `PartialEq`, and the gate does not need one: it diffs the
/// serialized documents. Answering "is there anything to write" the same way is
/// what keeps this decision and the gate's from ever disagreeing.
fn config_value_text(config: &Config) -> Result<String> {
    toml::to_string_pretty(config).context("Failed to serialize config for comparison")
}

/// Run a narrow edit, reloading once if another writer beat it to the file.
fn edit_one_config_key_with_retry(
    path: &Path,
    what: &str,
    apply: &dyn Fn(&mut Config) -> Result<()>,
    verify: &dyn Fn(&Config) -> bool,
) -> Result<ConfigEditOutcome> {
    match edit_one_config_key(path, what, apply, verify) {
        // Another writer landed between this document's load and its write.
        // Reloading picks their version up, and reapplying puts this edit on
        // top of it; a second collision is reported rather than looped over.
        Err(error) if is_stale_source_error(&error) => {
            info!("Config changed under the {what} write; reloading and reapplying");
            edit_one_config_key(path, what, apply, verify)
        }
        result => result,
    }
}

/// Write exactly one action's `[keybindings]` entry.
pub fn persist_keybinding_edit(action: Action, bindings: &[String]) -> Result<ConfigEditOutcome> {
    write_keybinding_edit(&Config::get_config_path()?, action, bindings)
}

/// Path-taking form, so the suite can drive real files without the process
/// environment.
///
/// Test-only, and gated rather than merely `pub(crate)`: production has no use
/// for a config path that did not come from the environment, and a build that
/// cannot name this cannot acquire one by accident. The name is still pinned in
/// `tools/check-config-writers.py`, which fails a production caller with a
/// message about the gesture rather than a resolution error.
#[cfg(test)]
pub(crate) fn persist_keybinding_edit_at(
    path: &Path,
    action: Action,
    bindings: &[String],
) -> Result<ConfigEditOutcome> {
    write_keybinding_edit(path, action, bindings)
}

fn write_keybinding_edit(
    path: &Path,
    action: Action,
    bindings: &[String],
) -> Result<ConfigEditOutcome> {
    edit_one_config_key_with_retry(
        path,
        "shortcut",
        &|config| {
            refuse_shortcut_claimed_on_disk(config, action, bindings)?;
            config
                .keybindings
                .set_bindings_for_action(action, bindings.to_vec())
                .map_err(|error| anyhow!(error))?;
            // This one list no longer comes from the document the authorship
            // was read from — the editor just typed it — so the omitted-default
            // pass must stop treating it as an offer this build made. Without
            // that, an action the file omits has its new shortcut filtered out
            // against whatever else claims the key, and the emptied list is
            // what the merge gate writes.
            //
            // One key, not the section: the other lists still are the file's.
            // Claiming otherwise would retire the pass for them too, which is
            // only harmless while the base stays a validated configuration —
            // the defaults that pass would filter are already gone from it, and
            // from `previous` with it. The narrow claim is the true one and
            // does not rest on that.
            config.mark_keybinding_explicit(action);
            Ok(())
        },
        &|config| config.keybindings.bindings_for_action(action) == Some(bindings),
    )
}

/// Refuses a chord the file gives to some other action.
///
/// The overlay ran the same check against the keymap this run loaded, which may
/// be older than the file. Re-running it on what was just read is what keeps a
/// stale accept from reaching the merge gate, where the requested list would be
/// dropped against the newer claimant and land as an empty key.
fn refuse_shortcut_claimed_on_disk(
    config: &Config,
    action: Action,
    bindings: &[String],
) -> Result<()> {
    // Claims other than this action's own: the list being replaced cannot
    // contest its replacement.
    let mut others = config.keybindings.clone();
    others
        .set_bindings_for_action(action, Vec::new())
        .map_err(|error| anyhow!(error))?;
    let claimed = others.claimed_keys();
    for text in bindings {
        let binding = ShortcutTrigger::parse(text).map_err(|error| anyhow!(error))?;
        if let Some(owner) = claimed.get(&binding) {
            return Err(anyhow!(ShortcutClaimedOnDisk {
                binding: text.clone(),
                claimed_by: *owner,
            }));
        }
    }
    Ok(())
}

/// Write exactly one `[presets.slot_N]` table, or remove it when clearing.
pub fn persist_preset_slot(
    slot: usize,
    preset: Option<&ToolPresetConfig>,
) -> Result<ConfigEditOutcome> {
    write_preset_slot(&Config::get_config_path()?, slot, preset)
}

/// Path-taking form; test-only for the reason given on
/// [`persist_keybinding_edit_at`].
#[cfg(test)]
pub(crate) fn persist_preset_slot_at(
    path: &Path,
    slot: usize,
    preset: Option<&ToolPresetConfig>,
) -> Result<ConfigEditOutcome> {
    write_preset_slot(path, slot, preset)
}

fn write_preset_slot(
    path: &Path,
    slot: usize,
    preset: Option<&ToolPresetConfig>,
) -> Result<ConfigEditOutcome> {
    // `set_slot` ignores an out-of-range slot silently, which would make the
    // write a no-op the verification below could not tell from success.
    if !(1..=PRESET_SLOTS_MAX).contains(&slot) {
        bail!("Preset slot {slot} is outside the {PRESET_SLOTS_MAX} configurable slots");
    }
    edit_one_config_key_with_retry(
        path,
        "preset",
        &|config| {
            config.presets.set_slot(slot, preset.cloned());
            Ok(())
        },
        &|config| config.presets.get_slot(slot) == preset,
    )
}

/// Write exactly one entry of `[[drawing.quick_colors]]`.
///
/// The palette is a single TOML value, so "one key" here is the array. The
/// write keeps it as short as the file already had it: recoloring a slot the
/// file spells out rewrites that entry and leaves the array's length, order,
/// and labels alone. Only recoloring a slot the file merely *implies* — one the
/// shipped defaults backfill — has to materialize the array up to that slot,
/// because there is no other way to express the color; the entries it gains
/// carry the values that were already in effect, so no slot changes meaning.
pub fn persist_quick_color(index: usize, color: Color) -> Result<ConfigEditOutcome> {
    write_quick_color(&Config::get_config_path()?, index, color)
}

/// Path-taking form; test-only for the reason given on
/// [`persist_keybinding_edit_at`].
#[cfg(test)]
pub(crate) fn persist_quick_color_at(
    path: &Path,
    index: usize,
    color: Color,
) -> Result<ConfigEditOutcome> {
    write_quick_color(path, index, color)
}

fn write_quick_color(path: &Path, index: usize, color: Color) -> Result<ConfigEditOutcome> {
    edit_one_config_key_with_retry(
        path,
        "quick color",
        &|config| {
            let palette = &mut config.drawing.quick_colors;
            let spec = ColorSpec::from(color);
            // The palette as the run sees it: what the file spells out, with
            // the shipped defaults standing in for the rest.
            let mut entries = palette.effective_entries();
            let Some(entry) = entries.get_mut(index) else {
                // Reported as a typed cause so the caller can say what happened
                // instead of blaming the save.
                return Err(anyhow!(QuickColorSlotMissing { index }));
            };
            if same_quick_color(&entry.color, color) {
                return Ok(());
            }
            entry.color = spec;
            // Write the array only as far as the edited slot. A TOML array is
            // positional, so a slot the file merely implies cannot be written
            // without the ones before it — but the ones after it can stay
            // implied, and staying implied is what keeps them tracking the
            // shipped palette. The entries this materializes carry the values
            // that were already in effect, so no slot changes meaning.
            let authored = palette.configured_entry_count().unwrap_or_default();
            entries.truncate(authored.max(index + 1));
            palette.set_entries(entries);
            Ok(())
        },
        &|config| {
            config
                .drawing
                .quick_colors
                .effective_entries()
                .get(index)
                .is_some_and(|entry| same_quick_color(&entry.color, color))
        },
    )
}

/// Whether a palette slot already paints this color.
///
/// Both sides go through the eight-bit form the file stores, so the question is
/// about the swatch rather than its spelling: `"#33F54F"` and the RGB array the
/// picker hands back are the same color, and rewriting one into the other is
/// not an edit anyone asked for. Quantizing the requested color the same way is
/// also what keeps a check *after* a write honest — a picked float only reaches
/// the file rounded.
fn same_quick_color(stored: &ColorSpec, color: Color) -> bool {
    ColorSpec::from(stored.to_color()) == ColorSpec::from(color)
}

pub(super) fn prepare_config_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create config directory")?;
    }
    Ok(())
}

/// How many same-second names one backup attempt will try before giving up.
/// Reached only if a burst of edits lands inside one second.
const BACKUP_NAME_ATTEMPTS: usize = 32;

/// Copies the current contents aside under a name no other backup holds.
///
/// The stamp is second-precision and edits arrive faster than that — a rebind
/// and the correction after it, a preset save and a recolor — so the name is
/// claimed with `create_new` and suffixed until it is free. Copying over an
/// existing `.bak` would destroy the previous edit's copy, which is exactly the
/// contents someone reaching for a backup wants back.
pub(super) fn create_config_backup(path: &Path) -> Result<PathBuf> {
    let timestamp = format_with_template(now_local(), "%Y%m%d_%H%M%S");
    let stem = match path.file_name().and_then(|name| name.to_str()) {
        Some(name) => format!("{name}.{timestamp}"),
        None => format!("config.toml.{timestamp}"),
    };
    let directory = path.parent().unwrap_or_else(|| Path::new("."));

    for attempt in 0..BACKUP_NAME_ATTEMPTS {
        let backup_path = directory.join(match attempt {
            0 => format!("{stem}.bak"),
            _ => format!("{stem}-{attempt}.bak"),
        });
        // Claiming the name with `O_EXCL` is what makes this collision-free:
        // testing for absence first leaves a window the next edit can land in.
        // `fs::copy` then fills the placeholder and restores the source's
        // permission bits onto it.
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup_path)
        {
            Ok(_) => {
                return match fs::copy(path, &backup_path) {
                    Ok(_) => Ok(backup_path),
                    Err(error) => {
                        // The reserved name holds an empty file until the copy
                        // lands; leaving it would pass for a backup of an empty
                        // config.
                        let _ = fs::remove_file(&backup_path);
                        Err(error).with_context(|| {
                            format!(
                                "Failed to create config backup from {} to {}",
                                path.display(),
                                backup_path.display()
                            )
                        })
                    }
                };
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to create config backup at {}",
                        backup_path.display()
                    )
                });
            }
        }
    }

    bail!("Config directory already holds every backup name for {stem}")
}

/// Renames the merged text onto a destination the caller already resolved,
/// reporting which file it left there.
///
/// `destination` is the end of the symlink chain the document recorded when it
/// read the file, and the same path the write lock was taken for and the byte
/// comparisons were made against. So the policy here refuses to follow a link
/// rather than following one: resolving a chain at this point would be a second
/// lookup of a path the caller has already resolved, and a link retargeted since
/// the last comparison would send these bytes to a file no lock covers and no
/// check ever read. A destination that has itself *become* a symlink since it was
/// resolved is the same situation from the other end, and is refused for the same
/// reason.
///
/// `expected` is the other half of the same idea, about the file rather than the
/// path. The advisory lock binds the writers that cooperate; anything else can
/// rename the checked file away and leave a replacement at the same name, and a
/// rename that only asks about the path would drop that replacement's contents
/// while reporting a clean save. An editor can also truncate and rewrite the
/// same file without changing its identity. Handing both the identity and exact
/// bytes the caller checked down to the rename makes the write conditional on
/// the complete revision still being there; the identity coming back names the
/// file this write created, which is what the caller's next comparison expects.
///
/// A destination that moved is reported in the wording
/// [`is_stale_source_error`] recognises, because the recovery is the editors'
/// ordinary one: load what the path holds now and reapply the edit onto it.
pub(super) fn write_config_text_atomic(
    destination: &Path,
    contents: &str,
    overwrite: OverwriteMode,
    expected: DestinationExpectation<'_>,
) -> Result<FileIdentity> {
    let mut options = AtomicWriteOptions::user_config_file();
    options.overwrite = overwrite;
    options.symlink = SymlinkPolicy::Reject;
    match crate::durable_io::write_atomic_reporting_identity(
        destination,
        contents.as_bytes(),
        options,
        Some(expected),
    ) {
        Ok(identity) => Ok(identity),
        Err(error) if matches!(error, DurableIoError::DestinationChanged { .. }) => Err(anyhow!(
            "Configuration changed on disk at {}: {error}. Reload before saving.",
            destination.display()
        )),
        Err(error) => Err(error)
            .with_context(|| format!("Failed to write config to {}", destination.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The load-bearing half of the narrow editors' one-key property.
    ///
    /// `edit_one_config_key` builds `updated` from `document.config()` and lets
    /// `save_with_backup` validate it again. That is only harmless while
    /// validating an already-validated configuration is a no-op: if some future
    /// pass clamped, resolved, or dropped something on the second run, every
    /// narrow write would carry that change into the file alongside the user's
    /// edit. This fixture is deliberately full of things the loader repairs or
    /// reports in memory — a contested shortcut pair, a binding that parses but
    /// names no deliverable key, an out-of-range number — so the second pass
    /// has something to get wrong.
    #[test]
    fn document_config_is_a_fixed_point_of_validation() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let path = temp.path().join("config.toml");
        fs::write(
            &path,
            "# comment\n\
             [keybindings]\n\
             undo = ['Ctrl+Alt+Shift+Q']\n\
             redo = ['Ctrl+Alt+Shift+Q']\n\
             clear_canvas = ['NotARealKey']\n\
             \n\
             [drawing]\n\
             default_thickness = 9999.0\n\
             \n\
             [ui]\n\
             setting_from_a_later_release = 7\n",
        )
        .expect("fixture");

        let document = ConfigDocument::load_from_path(&path).expect("load");
        let mut twice = document.config().clone();
        twice.validate_and_clamp();

        // `Config` has no `PartialEq`, and the merge gate does not need one: it
        // compares the serialized documents. Comparing the same way is what
        // makes this test speak about the write rather than about the type.
        let once = toml::to_string(document.config()).expect("serialize");
        let twice = toml::to_string(&twice).expect("serialize");
        let authored = toml::to_string(document.authored_config()).expect("serialize");

        assert_eq!(
            once, twice,
            "validating an already-validated config must change nothing, or every \
             narrow write would smuggle the difference into config.toml"
        );
        assert_ne!(
            once, authored,
            "the fixture must actually give validation something to repair"
        );
    }

    /// Two edits inside one second must leave two backups.
    ///
    /// The stamp resolves to seconds, and edits arrive faster than that — a
    /// rebind and the correction after it — so the second copy used to land on
    /// the first one's name and destroy the only record of what the file said
    /// before the pair started.
    #[test]
    fn two_backups_in_the_same_second_keep_distinct_names_and_contents() {
        let temp = crate::test_temp::tempdir().expect("tempdir");

        // Each attempt gets its own directory, so the `-1` suffix means "the
        // stamp collided" and nothing else. Two calls take microseconds; the
        // retry is here so a second boundary falling between them cannot make
        // the test flaky, not because it is expected to be needed.
        for attempt in 0..16 {
            let directory = temp.path().join(format!("attempt-{attempt}"));
            fs::create_dir_all(&directory).expect("attempt directory");
            let path = directory.join("config.toml");

            fs::write(&path, "first = 1\n").expect("first contents");
            let first = create_config_backup(&path).expect("the first backup");
            fs::write(&path, "second = 2\n").expect("second contents");
            let second = create_config_backup(&path).expect("the second backup");

            assert_ne!(
                first, second,
                "the second backup must not take the first one's name"
            );
            assert_eq!(
                fs::read_to_string(&first).expect("readable"),
                "first = 1\n",
                "the first backup must still hold what it copied"
            );
            assert_eq!(
                fs::read_to_string(&second).expect("readable"),
                "second = 2\n"
            );

            if second.to_string_lossy().ends_with("-1.bak") {
                return;
            }
        }
        panic!("the clock never put two backups inside the same second");
    }
}
