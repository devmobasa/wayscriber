//! Lossless configuration document ownership for editing clients.

mod lock;
mod merge;

pub use lock::ConfigWriteLockTimeout;

use super::io::{create_config_backup, prepare_config_parent, write_config_text_atomic};
use super::keybindings::KeybindingAuthorship;
use super::{Config, ConfigSource};
use crate::durable_io::{
    DestinationExpectation, FileIdentity, OverwriteMode, resolve_symlink_chain,
};
use anyhow::{Context, Result, anyhow, bail};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use toml_edit::DocumentMut;

use lock::{CONFIG_WRITE_LOCK_TIMEOUT, acquire_config_write_lock};
use merge::{
    conservative_repair_source_document, merge_config_document, repair_source_document,
    serialize_config_document,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigDiagnosticKind {
    UnknownSetting,
    /// A shortcut two actions both claim. Loading drops it from one of them
    /// for the session; the file is left exactly as authored.
    KeybindingConflict,
    /// A shortcut string the parser rejects. Loading drops it from the session
    /// keymap; the file is left exactly as authored.
    InvalidKeybinding,
    /// A shipped default an omitted action did not receive, because the file
    /// already binds that key to something else. Informational: nothing the
    /// user authored changed, and the file is left exactly as authored.
    DefaultShortcutSkipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDiagnostic {
    kind: ConfigDiagnosticKind,
    path: String,
    detail: Option<String>,
}

impl ConfigDiagnostic {
    pub fn kind(&self) -> ConfigDiagnosticKind {
        self.kind
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl fmt::Display for ConfigDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ConfigDiagnosticKind::UnknownSetting => {
                write!(formatter, "unrecognized setting `{}`", self.path)
            }
            ConfigDiagnosticKind::KeybindingConflict => match &self.detail {
                Some(detail) => write!(formatter, "{detail}"),
                None => write!(formatter, "conflicting keybinding in `{}`", self.path),
            },
            ConfigDiagnosticKind::InvalidKeybinding => match &self.detail {
                Some(detail) => write!(formatter, "{detail}"),
                None => write!(formatter, "invalid keybinding in `{}`", self.path),
            },
            ConfigDiagnosticKind::DefaultShortcutSkipped => match &self.detail {
                Some(detail) => write!(formatter, "{detail}"),
                None => write!(formatter, "default keybinding skipped for `{}`", self.path),
            },
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
enum SourceRevision {
    Missing {
        followed_links: Vec<(PathBuf, PathBuf)>,
        destination: PathBuf,
    },
    Present {
        bytes: Vec<u8>,
        /// Which file those bytes came out of.
        ///
        /// Part of the revision for the same reason the symlink chain is: a
        /// file renamed away and replaced under the same name is a different
        /// file, and a replacement that happens to hold identical text is the
        /// case bytes alone cannot tell apart. It is also what the write is
        /// made conditional on, so the rename lands on the file every check in
        /// between was about.
        ///
        /// Taken from the same `stat` that decided the path names a regular
        /// file, one syscall before the read. A file swapped in between the two
        /// leaves the pair describing two files, and the consequence is always
        /// the safe one: the identity no longer matches the destination, so
        /// every later comparison — and the rename itself — refuses the save
        /// rather than writing anywhere unexpected.
        identity: FileIdentity,
        followed_links: Vec<(PathBuf, PathBuf)>,
        destination: PathBuf,
    },
}

impl fmt::Debug for SourceRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing {
                followed_links,
                destination,
            } => formatter
                .debug_struct("Missing")
                .field("followed_links", followed_links)
                .field("destination", destination)
                .finish(),
            Self::Present {
                bytes,
                identity,
                followed_links,
                destination,
            } => formatter
                .debug_struct("Present")
                .field("byte_len", &bytes.len())
                .field("identity", identity)
                .field("followed_links", followed_links)
                .field("destination", destination)
                .finish(),
        }
    }
}

impl SourceRevision {
    fn read(path: &Path) -> Result<Self> {
        let (final_path, followed_links) = resolve_symlink_chain(path)
            .with_context(|| format!("Failed to resolve config source {}", path.display()))?;
        let destination = pin_destination(&final_path)?;
        let metadata = match fs::symlink_metadata(&destination) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(Self::Missing {
                    followed_links,
                    destination,
                });
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to inspect config source {}", destination.display())
                });
            }
        };

        if !metadata.is_file() {
            bail!(
                "Config source {} is not a regular file",
                destination.display()
            );
        }
        let bytes = fs::read(&destination)
            .with_context(|| format!("Failed to read config from {}", destination.display()))?;
        Ok(Self::Present {
            bytes,
            identity: FileIdentity::of(&metadata),
            followed_links,
            destination,
        })
    }

    fn bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Missing { .. } => None,
            Self::Present { bytes, .. } => Some(bytes),
        }
    }

    fn overwrite_mode(&self) -> OverwriteMode {
        match self {
            Self::Missing { .. } => OverwriteMode::CreateNew,
            Self::Present { .. } => OverwriteMode::Replace,
        }
    }

    /// What the rename must still find at the destination.
    ///
    /// The comparisons this document makes are syscalls of their own, and the
    /// rename is another; between them anything that is not one of the writers
    /// the lock binds can replace the file or rewrite it in place. Carrying the
    /// checked file's identity and exact bytes down to the write makes the
    /// rename conditional on the complete revision — and a load that found
    /// nothing expects to find nothing, because a file that appeared in the
    /// meantime is somebody's creation and overwriting it would discard it
    /// unread.
    fn expectation(&self) -> DestinationExpectation<'_> {
        match self {
            Self::Missing { .. } => DestinationExpectation::Absent,
            Self::Present {
                bytes, identity, ..
            } => DestinationExpectation::Present {
                identity: *identity,
                contents: bytes,
            },
        }
    }

    /// The symlink chain the load walked, link by link.
    ///
    /// Part of the revision, not a detail of it: a config path that resolves
    /// somewhere else than it did is a different file, whatever bytes happen to
    /// be in it.
    fn followed_links(&self) -> &[(PathBuf, PathBuf)] {
        match self {
            Self::Missing { followed_links, .. } | Self::Present { followed_links, .. } => {
                followed_links
            }
        }
    }

    /// The file that chain ends at, with every symlink on its path resolved —
    /// where the bytes were read from, and the only path a save may write.
    fn destination(&self) -> &Path {
        match self {
            Self::Missing { destination, .. } | Self::Present { destination, .. } => destination,
        }
    }

    /// The revision a save leaves behind: the text it wrote, in the file it
    /// created.
    ///
    /// `identity` comes from the write rather than from a fresh look at the
    /// path, because the two are not the same claim — a look afterwards would
    /// name whoever wrote last, and this revision is about what *this* save
    /// put there. Naming it exactly is what lets the next save through: it
    /// compares the destination against this identity, and a guess would refuse
    /// an ordinary second save as a change on disk.
    fn after_write(&self, bytes: &[u8], identity: FileIdentity) -> Self {
        let (followed_links, destination) = match self {
            Self::Missing {
                followed_links,
                destination,
            }
            | Self::Present {
                followed_links,
                destination,
                ..
            } => (followed_links.clone(), destination.clone()),
        };
        Self::Present {
            bytes: bytes.to_vec(),
            identity,
            followed_links,
            destination,
        }
    }
}

/// The destination with every symlink on its path resolved, not only the ones
/// on its last component.
///
/// [`resolve_symlink_chain`] answers what the final component points at, which
/// is a smaller question than which file the path names. A config path whose
/// *parent* is a link — `~/.config/wayscriber/active/config.toml`, with `active`
/// pointing at one profile directory — starts naming a different file the moment
/// that link is retargeted, and nothing about its final component moves when it
/// happens. Canonicalizing the directory is what turns the loaded path into a
/// file the save can hold on to: the lock, the comparisons, and the rename all
/// address the profile the document was read from, so a retarget between load
/// and save is a stale source rather than an edit written into somebody else's
/// profile.
///
/// The last component is deliberately left as the chain resolved it. Resolving
/// it again would undo that work, and it may name nothing at all — a dangling
/// link's target is created by the first save. The walk falls back to the
/// deepest ancestor that does exist for the same reason: a config directory the
/// save is about to create has to pin the same way before and after
/// `prepare_config_parent` makes it, or the first save into a fresh
/// `~/.config/wayscriber/` would report its own directory as a change on disk.
///
/// A relative path is anchored to the current directory before any of that.
/// Only a relative path can run the walk out — an absolute one always ends at
/// `/`, which exists — and what it would leave behind is a pin that still says
/// what the caller typed. That is not a stable answer: the save creates the
/// directories, the next derivation finds them and canonicalizes through them,
/// and the two disagree about which file the window was about, so the save
/// reports its own `mkdir` as somebody else's retarget. Resolving the anchor
/// first makes both derivations name the same absolute file, before and after
/// the directories exist.
fn pin_destination(final_path: &Path) -> Result<PathBuf> {
    let Some(file_name) = final_path.file_name() else {
        bail!(
            "Config source {} does not name a file",
            final_path.display()
        );
    };
    let anchored = (!final_path.is_absolute())
        .then(|| -> Result<PathBuf> {
            let current_directory = fs::canonicalize(".").with_context(|| {
                format!(
                    "Failed to resolve the current directory for the config source {}",
                    final_path.display()
                )
            })?;
            Ok(current_directory.join(final_path))
        })
        .transpose()?;
    let final_path = anchored.as_deref().unwrap_or(final_path);
    let mut trailing = vec![file_name.to_os_string()];
    let mut current = final_path.parent();
    while let Some(directory) = current {
        if directory.as_os_str().is_empty() {
            break;
        }
        match fs::canonicalize(directory) {
            Ok(mut pinned) => {
                pinned.extend(trailing.iter().rev());
                return Ok(pinned);
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                let Some(name) = directory.file_name() else {
                    break;
                };
                trailing.push(name.to_os_string());
                current = directory.parent();
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to resolve the config directory {}",
                        directory.display()
                    )
                });
            }
        }
    }
    // Nothing on the path exists to resolve against, which a `..` sitting above
    // a directory that is not there can still reach. The path is absolute by
    // now, so it stands as the chain left it and pins the same way at the next
    // comparison whatever the save creates in between.
    Ok(final_path.to_path_buf())
}

#[derive(Debug)]
pub struct ConfigDocument {
    config: Config,
    /// The same parse before validation touched it, so an editor can show what
    /// the file says rather than what this session resolved it to. A migration
    /// preview has to diff against the authored values: proposing a change to a
    /// binding that only exists because loading dropped a contested key would
    /// offer the user an edit their file never contained.
    authored_config: Config,
    document: DocumentMut,
    source_path: PathBuf,
    source: ConfigSource,
    revision: SourceRevision,
    diagnostics: Vec<ConfigDiagnostic>,
    repair_mode: bool,
}

impl ConfigDocument {
    pub fn load() -> Result<Self> {
        Self::load_from_path(Config::get_config_path()?)
    }

    pub fn load_from_path(path: impl Into<PathBuf>) -> Result<Self> {
        let source_path = path.into();
        let revision = SourceRevision::read(&source_path)?;
        Self::from_revision(source_path, revision)
    }

    /// Loads a document for an interactive editor, falling back to a repairable
    /// default draft when the file exists but its contents cannot be parsed.
    ///
    /// The returned warning contains the original parse failure. Saving the
    /// fallback document remains revision-guarded and creates a backup first.
    pub fn load_for_editing() -> Result<(Self, Option<String>)> {
        Self::load_for_editing_from_path(Config::get_config_path()?)
    }

    pub fn load_for_editing_from_path(path: impl Into<PathBuf>) -> Result<(Self, Option<String>)> {
        let source_path = path.into();
        let revision = SourceRevision::read(&source_path)?;
        match Self::from_revision(source_path.clone(), revision.clone()) {
            Ok(document) => Ok((document, None)),
            Err(error) if revision.bytes().is_some() => {
                let document = revision
                    .bytes()
                    .and_then(|bytes| std::str::from_utf8(bytes).ok())
                    .and_then(|input| input.parse::<DocumentMut>().ok())
                    .unwrap_or_default();
                Ok((
                    Self {
                        config: Config::default(),
                        authored_config: Config::default(),
                        document,
                        source_path,
                        source: ConfigSource::Primary,
                        revision,
                        diagnostics: Vec::new(),
                        repair_mode: true,
                    },
                    Some(format!("{error:#}")),
                ))
            }
            Err(error) => Err(error),
        }
    }

    fn from_revision(source_path: PathBuf, revision: SourceRevision) -> Result<Self> {
        match revision.bytes() {
            Some(bytes) => {
                let input = std::str::from_utf8(bytes).with_context(|| {
                    format!("Config at {} is not valid UTF-8", source_path.display())
                })?;
                let document = input.parse::<DocumentMut>().with_context(|| {
                    format!("Failed to parse config from {}", source_path.display())
                })?;
                let parsed = parse_typed_config(input).with_context(|| {
                    format!("Failed to parse config from {}", source_path.display())
                })?;
                Ok(Self {
                    config: parsed.config,
                    authored_config: parsed.authored,
                    document,
                    source_path,
                    source: ConfigSource::Primary,
                    revision,
                    diagnostics: parsed.diagnostics,
                    repair_mode: false,
                })
            }
            None => Ok(Self {
                config: Config::default(),
                authored_config: Config::default(),
                document: DocumentMut::new(),
                source_path,
                source: ConfigSource::Default,
                revision,
                diagnostics: Vec::new(),
                repair_mode: false,
            }),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The parsed configuration before validation, exactly as the source spells
    /// it out. [`Self::config`] is the same values after the running session's
    /// clamps and shortcut resolution.
    pub fn authored_config(&self) -> &Config {
        &self.authored_config
    }

    /// Which `[keybindings]` keys the source spells out. A migration preview
    /// needs this to tell a shortcut the user wrote from one serde filled in.
    pub fn keybinding_authorship(&self) -> &KeybindingAuthorship {
        &self.authored_config.keybinding_authorship
    }

    pub fn source(&self) -> &ConfigSource {
        &self.source
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn diagnostics(&self) -> &[ConfigDiagnostic] {
        &self.diagnostics
    }

    /// Writes an updated configuration, preserving the source document's
    /// comments, ordering, unknown settings, and compatible TOML formatting,
    /// after copying the previous contents to a timestamped `.bak`.
    ///
    /// The application's only durable `config.toml` write. Every writer goes
    /// through it: the configurator's explicit Save, which hands back a whole
    /// edited draft, and the overlay's narrow editors in `io.rs`, which each
    /// hand back the loaded config with one value changed. Nothing that merely
    /// runs — overlay startup, a toolbar toggle, a tray action, shutdown —
    /// reaches it, so the file stays exactly as authored unless the user
    /// deliberately edits it.
    ///
    /// The write window is held under an advisory lock on a sibling lock file
    /// (see `lock.rs`), so two of those writers in two processes cannot both
    /// pass the revision check and have the second rename discard the first.
    ///
    /// The lock binds the writers that take it, which is every writer this
    /// application has and no one else's. So the window does not rest on it
    /// alone: the identity and exact contents that were checked are carried down
    /// to the rename. An editor outside this application that replaces the file
    /// or rewrites it in place gets a stale-source error here rather than having
    /// its work quietly overwritten, and the editors reload and reapply onto
    /// what the path holds now.
    ///
    /// The window is about one file, named once. The lock, the byte
    /// comparisons, and the rename all address the destination this document
    /// loaded from — the end of the symlink chain it recorded, resolved through
    /// its directories as well as its final component — so a link retargeted
    /// while the window is open cannot move the write to a file nobody locked
    /// and nobody compared.
    pub fn save_with_backup(&self, mut config: Config) -> Result<ConfigDocumentSaveOutcome> {
        config.validate_and_clamp();
        self.merge_and_write(&self.config, &config, &mut || {})
    }

    /// [`Self::save_with_backup`], with the write window opened for the suite.
    ///
    /// `before_write` runs after the source bytes, the identity, and the symlink
    /// chain were last compared and before the merged text is renamed into
    /// place — the one stretch another writer can still arrive in unseen by any
    /// of those checks. Staging that arrival any other way would be a race
    /// rather than a test, and it is what the two properties above are stated
    /// against: a retarget of the path leaves the rename on the file the window
    /// was about, and a replacement of that file itself is refused by the
    /// expectation the rename carries.
    #[cfg(test)]
    pub(crate) fn save_with_backup_racing_the_write(
        &self,
        mut config: Config,
        before_write: &mut dyn FnMut(),
    ) -> Result<ConfigDocumentSaveOutcome> {
        config.validate_and_clamp();
        self.merge_and_write(&self.config, &config, before_write)
    }

    fn merge_and_write(
        &self,
        previous: &Config,
        updated: &Config,
        before_write: &mut dyn FnMut(),
    ) -> Result<ConfigDocumentSaveOutcome> {
        let repair_source = self
            .repair_mode
            .then(|| repair_source_document(&self.document, previous, updated))
            .transpose()?;
        let source = repair_source.as_ref().unwrap_or(&self.document);
        let mut merged = merge_config_document(source, previous, updated, self.repair_mode)?;
        let mut output = merged.to_string();
        let parsed = parse_typed_config(&output);
        let parsed = match parsed {
            Ok(parsed) => parsed,
            Err(_) if self.repair_mode => {
                let conservative =
                    conservative_repair_source_document(&self.document, previous, updated)?;
                merged = merge_config_document(&conservative, previous, updated, self.repair_mode)?;
                output = merged.to_string();
                parse_typed_config(&output)
                    .context("Repaired config failed its validation parse before save")?
            }
            Err(error) => {
                return Err(error).context("Merged config failed its validation parse before save");
            }
        };

        // The file this window is about, resolved once — through its parent
        // directories as much as through its own final component. Everything
        // below addresses it by this path: the lock, the two comparisons, and
        // the rename. Naming the config path again at any of those steps would
        // resolve it a second time, and a link retargeted since the last
        // comparison — the leaf, or a directory above it — would take that step
        // somewhere else: a lock on one file and a rename onto another, whose
        // bytes nothing checked.
        let destination = self.revision.destination();
        prepare_config_parent(destination)?;
        // Everything from here to the rename is one window. The comparison
        // below only means anything while no other writer can rename between it
        // and this write, and the two are separate syscalls in separate
        // processes; the lock is what makes them one step. The loser of the race
        // finds the file changed and reports it, which the editors' reload-and-
        // reapply retry recovers from — where without the lock both writers
        // would pass the comparison and the second rename would drop the first
        // edit with both reporting success.
        let _write_lock = acquire_config_write_lock(destination, CONFIG_WRITE_LOCK_TIMEOUT)?;
        self.ensure_source_unchanged()?;
        let backup_path = match self.revision {
            // Through the config path on purpose, unlike the rename below: the
            // copy belongs beside the path the user knows, not beside the file a
            // link happens to point at (pinned by
            // `save_with_backup_preserves_symlinked_config_target_and_backup_contents`).
            // It sits between the two comparisons, so a retarget arriving around
            // it is caught by the second one and nothing is written.
            SourceRevision::Present { .. } => Some(create_config_backup(&self.source_path)?),
            SourceRevision::Missing { .. } => None,
        };
        self.ensure_source_unchanged()?;
        before_write();
        // The comparison above and the rename below are still separate syscalls,
        // and the lock only binds the writers that take it. What closes the rest
        // of the distance is the expectation: the rename is conditional on the
        // destination still having the identity and exact bytes this document
        // read, so both replacement and in-place edits are refused rather than
        // silently overwritten. What that leaves is documented at
        // `finalize_temp_file`, where the check sits: one look at the
        // destination and one rename, adjacent, with no syscall able to close
        // the gap between them.
        let identity = write_config_text_atomic(
            destination,
            &output,
            self.revision.overwrite_mode(),
            self.revision.expectation(),
        )?;
        let revision = self.revision.after_write(output.as_bytes(), identity);

        Ok(ConfigDocumentSaveOutcome {
            document: Self {
                config: parsed.config,
                authored_config: parsed.authored,
                document: merged,
                source_path: self.source_path.clone(),
                source: ConfigSource::Primary,
                revision,
                diagnostics: parsed.diagnostics,
                repair_mode: false,
            },
            backup_path,
        })
    }

    /// Whether the file this document loaded is still the file it would write.
    ///
    /// Three things can end that. The bytes can change, which is the ordinary
    /// second writer. The file can be replaced, which the bytes need not show:
    /// a rename-away and a fresh file under the same name can hold identical
    /// text and still be a file this document never read, so the identity is
    /// compared as well as the contents. Or the config path can start resolving
    /// somewhere else, which is not a smaller version of either: a retargeted
    /// link means the bytes being compared are a different file's, and the
    /// document's edit was merged into text that path no longer holds. Each is
    /// reported on its own terms, with the same "changed on disk" wording the
    /// editors' reload-and-reapply retry recognises, because the recovery is the
    /// same — load what the path names now and reapply the edit onto it.
    ///
    /// The destination is re-derived here rather than re-read from the loaded
    /// revision, and it is derived the same way the load derived it: whole path,
    /// directories included. A link one level up — a profile directory swapped
    /// under a stable `config.toml` — moves the file the path names without
    /// touching the final component or the bytes at the end of it, so nothing
    /// else here would notice, and the save would land in the profile the user
    /// switched *to* while reporting that it wrote the one they switched from.
    fn ensure_source_unchanged(&self) -> Result<()> {
        let current = SourceRevision::read(&self.source_path)?;
        if current.destination() != self.revision.destination() {
            bail!(
                "Configuration changed on disk at {}: it now resolves to {} rather than {}. \
                 Reload before saving.",
                self.source_path.display(),
                current.destination().display(),
                self.revision.destination().display(),
            );
        }
        if current.followed_links() != self.revision.followed_links() {
            bail!(
                "Configuration changed on disk at {}: it reaches {} through different links \
                 than it did. Reload before saving.",
                self.source_path.display(),
                self.revision.destination().display(),
            );
        }
        if current != self.revision {
            bail!(
                "Configuration changed on disk at {}. Reload before saving.",
                self.source_path.display()
            );
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ConfigDocumentSaveOutcome {
    document: ConfigDocument,
    backup_path: Option<PathBuf>,
}

impl ConfigDocumentSaveOutcome {
    pub fn document(&self) -> &ConfigDocument {
        &self.document
    }

    pub fn backup_path(&self) -> Option<&Path> {
        self.backup_path.as_deref()
    }

    pub fn into_parts(self) -> (ConfigDocument, Option<PathBuf>) {
        (self.document, self.backup_path)
    }
}

struct ParsedConfig {
    config: Config,
    /// The same parse before `validate_and_clamp` ran.
    authored: Config,
    diagnostics: Vec<ConfigDiagnostic>,
}

fn parse_typed_config(input: &str) -> Result<ParsedConfig> {
    let mut ignored = BTreeSet::new();
    let deserializer = toml::Deserializer::parse(input).context("Failed to parse TOML")?;
    let mut config: Config = serde_ignored::deserialize(deserializer, |path| {
        let path = path.to_string();
        if !is_known_feature_gated_path(&path) {
            ignored.insert(path);
        }
    })
    .map_err(|error| anyhow!(error))?;
    // Serde reports an omitted `[keybindings]` field as this build's default,
    // so presence has to come from the source text for resolution to tell an
    // authored shortcut from an offer (#293).
    config.keybinding_authorship = KeybindingAuthorship::from_toml_source(input);
    let authored = config.clone();
    let validation = config.validate_and_clamp();
    collect_flattened_unknown_paths(input, &config, &mut ignored)?;
    let mut diagnostics: Vec<ConfigDiagnostic> = ignored
        .into_iter()
        .map(|path| ConfigDiagnostic {
            kind: ConfigDiagnosticKind::UnknownSetting,
            path,
            detail: None,
        })
        .collect();
    // Neither a dropped typo nor a resolved conflict ever reaches the file, so
    // the editor is the only place the user can find out that one of their
    // shortcuts is inert.
    diagnostics.extend(
        validation
            .invalid_keybindings
            .iter()
            .map(|invalid| ConfigDiagnostic {
                kind: ConfigDiagnosticKind::InvalidKeybinding,
                path: invalid.config_key().map_or_else(
                    || "keybindings".to_string(),
                    |key| format!("keybindings.{key}"),
                ),
                detail: Some(invalid.to_string()),
            }),
    );
    diagnostics.extend(
        validation
            .keybinding_conflicts
            .iter()
            .map(|resolution| ConfigDiagnostic {
                kind: ConfigDiagnosticKind::KeybindingConflict,
                path: resolution.dropped_config_key().map_or_else(
                    || "keybindings".to_string(),
                    |key| format!("keybindings.{key}"),
                ),
                detail: Some(resolution.to_string()),
            }),
    );
    // The key this one names is the one the file does not have; that absence is
    // exactly why the default was on offer, and it is where the user would add
    // the shortcut if they wanted it.
    diagnostics.extend(validation.skipped_default_shortcuts.iter().map(|skipped| {
        ConfigDiagnostic {
            kind: ConfigDiagnosticKind::DefaultShortcutSkipped,
            path: skipped.config_key().map_or_else(
                || "keybindings".to_string(),
                |key| format!("keybindings.{key}"),
            ),
            detail: Some(skipped.to_string()),
        }
    }));
    Ok(ParsedConfig {
        config,
        authored,
        diagnostics,
    })
}

fn collect_flattened_unknown_paths(
    input: &str,
    config: &Config,
    ignored: &mut BTreeSet<String>,
) -> Result<()> {
    let source = input
        .parse::<DocumentMut>()
        .context("Failed to inspect flattened config fields")?;
    let known = serialize_config_document(config)?;
    let Some(source_keybindings) = source
        .get("keybindings")
        .and_then(toml_edit::Item::as_table_like)
    else {
        return Ok(());
    };
    let Some(known_keybindings) = known
        .get("keybindings")
        .and_then(toml_edit::Item::as_table_like)
    else {
        return Ok(());
    };

    for (key, _) in source_keybindings.iter() {
        if !known_keybindings.contains_key(key) {
            ignored.insert(format!("keybindings.{key}"));
        }
    }
    Ok(())
}

fn is_known_feature_gated_path(_path: &str) -> bool {
    #[cfg(not(feature = "tablet-input"))]
    if _path == "tablet" || _path.starts_with("tablet.") {
        return true;
    }
    false
}
