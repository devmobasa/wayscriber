//! Lossless configuration document ownership for editing clients.

mod merge;

use super::io::{create_config_backup, prepare_config_parent, write_config_text_atomic};
use super::{Config, ConfigSource};
use crate::durable_io::{OverwriteMode, resolve_symlink_chain};
use anyhow::{Context, Result, anyhow, bail};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use toml_edit::{DocumentMut, TableLike};

use merge::{
    conservative_repair_source_document, merge_config_document, repair_source_document,
    serialize_config_document,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigDiagnosticKind {
    UnknownSetting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDiagnostic {
    kind: ConfigDiagnosticKind,
    path: String,
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
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
enum SourceRevision {
    Missing {
        followed_links: Arc<[(PathBuf, PathBuf)]>,
    },
    Present {
        bytes: Arc<[u8]>,
        followed_links: Arc<[(PathBuf, PathBuf)]>,
    },
}

impl fmt::Debug for SourceRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { followed_links } => formatter
                .debug_struct("Missing")
                .field("followed_links", followed_links)
                .finish(),
            Self::Present {
                bytes,
                followed_links,
            } => formatter
                .debug_struct("Present")
                .field("byte_len", &bytes.len())
                .field("followed_links", followed_links)
                .finish(),
        }
    }
}

impl SourceRevision {
    fn read(path: &Path) -> Result<Self> {
        let (final_path, followed_links) = resolve_symlink_chain(path)
            .with_context(|| format!("Failed to resolve config source {}", path.display()))?;
        let followed_links: Arc<[(PathBuf, PathBuf)]> = followed_links.into();
        let metadata = match fs::symlink_metadata(&final_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(Self::Missing { followed_links });
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to inspect config source {}", final_path.display())
                });
            }
        };

        if !metadata.is_file() {
            bail!(
                "Config source {} is not a regular file",
                final_path.display()
            );
        }
        let bytes = fs::read(&final_path)
            .with_context(|| format!("Failed to read config from {}", final_path.display()))?;
        Ok(Self::Present {
            bytes: Arc::from(bytes.into_boxed_slice()),
            followed_links,
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

    fn destination_path<'a>(&'a self, source_path: &'a Path) -> &'a Path {
        match self {
            Self::Missing { followed_links } | Self::Present { followed_links, .. } => {
                followed_links
                    .last()
                    .map_or(source_path, |(_, target)| target.as_path())
            }
        }
    }

    fn after_write(&self, bytes: &[u8]) -> Self {
        let followed_links = match self {
            Self::Missing { followed_links } | Self::Present { followed_links, .. } => {
                Arc::clone(followed_links)
            }
        };
        Self::Present {
            bytes: Arc::from(bytes),
            followed_links,
        }
    }
}

#[derive(Debug)]
pub struct ConfigDocument {
    config: Config,
    document: DocumentMut,
    known_document: DocumentMut,
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
                let config = Config::default();
                let known_document = serialize_config_document(&config)?;
                Ok((
                    Self {
                        config,
                        document,
                        known_document,
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
                    document,
                    known_document: parsed.known_document,
                    source_path,
                    source: ConfigSource::Primary,
                    revision,
                    diagnostics: parsed.diagnostics,
                    repair_mode: false,
                })
            }
            None => {
                let config = Config::default();
                let document = DocumentMut::new();
                let known_document = serialize_config_document(&config)?;
                Ok(Self {
                    config,
                    document,
                    known_document,
                    source_path,
                    source: ConfigSource::Default,
                    revision,
                    diagnostics: Vec::new(),
                    repair_mode: false,
                })
            }
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
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

    pub fn save_with_backup(&self, mut config: Config) -> Result<ConfigDocumentSaveOutcome> {
        config.validate_and_clamp();
        let repair_source = self
            .repair_mode
            .then(|| repair_source_document(&self.document, &self.config, &config))
            .transpose()?;
        let source = repair_source.as_ref().unwrap_or(&self.document);
        let mut merged =
            merge_config_document(source, &self.config, &config, &self.known_document)?;
        let mut output = merged.to_string();
        let parsed = parse_typed_config(&output);
        let parsed = match parsed {
            Ok(parsed) => parsed,
            Err(_) if self.repair_mode => {
                let conservative =
                    conservative_repair_source_document(&self.document, &self.config, &config)?;
                merged = merge_config_document(
                    &conservative,
                    &self.config,
                    &config,
                    &self.known_document,
                )?;
                output = merged.to_string();
                parse_typed_config(&output)
                    .context("Repaired config failed its validation parse before save")?
            }
            Err(error) => {
                return Err(error).context("Merged config failed its validation parse before save");
            }
        };

        self.ensure_source_unchanged()?;
        prepare_config_parent(self.revision.destination_path(&self.source_path))?;
        let backup_path = if matches!(self.revision, SourceRevision::Present { .. }) {
            Some(create_config_backup(&self.source_path)?)
        } else {
            None
        };
        self.ensure_source_unchanged()?;
        write_config_text_atomic(&self.source_path, &output, self.revision.overwrite_mode())?;
        let revision = self.revision.after_write(output.as_bytes());

        Ok(ConfigDocumentSaveOutcome {
            document: Self {
                config: parsed.config,
                document: merged,
                known_document: parsed.known_document,
                source_path: self.source_path.clone(),
                source: ConfigSource::Primary,
                revision,
                diagnostics: parsed.diagnostics,
                repair_mode: false,
            },
            backup_path,
        })
    }

    fn ensure_source_unchanged(&self) -> Result<()> {
        let current = SourceRevision::read(&self.source_path)?;
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
    known_document: DocumentMut,
    diagnostics: Vec<ConfigDiagnostic>,
}

fn parse_typed_config(input: &str) -> Result<ParsedConfig> {
    let mut ignored = BTreeSet::new();
    let mut editor_input = input
        .parse::<DocumentMut>()
        .context("Failed to parse TOML")?;
    strip_unknown_strict_export_fields(&mut editor_input, &mut ignored);
    let editor_input = editor_input.to_string();
    let deserializer = toml::Deserializer::parse(&editor_input).context("Failed to parse TOML")?;
    let mut config: Config = serde_ignored::deserialize(deserializer, |path| {
        let path = path.to_string();
        if !is_known_feature_gated_path(&path) {
            ignored.insert(path);
        }
    })
    .map_err(|error| anyhow!(error))?;
    let known_document = serialize_config_document(&config)?;
    config.validate_and_clamp();
    collect_flattened_unknown_paths(input, &config, &mut ignored)?;
    let diagnostics = ignored
        .into_iter()
        .map(|path| ConfigDiagnostic {
            kind: ConfigDiagnosticKind::UnknownSetting,
            path,
        })
        .collect();
    Ok(ParsedConfig {
        config,
        known_document,
        diagnostics,
    })
}

fn strip_unknown_strict_export_fields(document: &mut DocumentMut, ignored: &mut BTreeSet<String>) {
    strip_unknown_fields_at_path(document, &["export"], &["pdf"], ignored);
    strip_unknown_fields_at_path(
        document,
        &["export", "pdf"],
        &[
            "filename_template",
            "all_boards_filename_template",
            "page_size",
            "orientation",
            "fit",
            "transparent_background",
            "custom_width",
            "custom_height",
            "content_source_padding",
            "labels",
        ],
        ignored,
    );
    strip_unknown_fields_at_path(
        document,
        &["export", "pdf", "labels"],
        &[
            "enabled",
            "position",
            "content",
            "template",
            "font_family",
            "font_size",
            "margin",
            "padding_x",
            "padding_y",
            "text_color",
            "background_enabled",
            "background_color",
        ],
        ignored,
    );
}

fn strip_unknown_fields_at_path(
    document: &mut DocumentMut,
    path: &[&str],
    known_fields: &[&str],
    ignored: &mut BTreeSet<String>,
) {
    let Some(table) = table_like_at_path_mut(document.as_table_mut(), path) else {
        return;
    };
    let unknown = table
        .iter()
        .map(|(key, _)| key.to_string())
        .filter(|key| !known_fields.contains(&key.as_str()))
        .collect::<Vec<_>>();
    let prefix = path.join(".");
    for key in unknown {
        table.remove(&key);
        ignored.insert(format!("{prefix}.{key}"));
    }
}

fn table_like_at_path_mut<'a>(
    table: &'a mut dyn TableLike,
    path: &[&str],
) -> Option<&'a mut dyn TableLike> {
    let Some((head, tail)) = path.split_first() else {
        return Some(table);
    };
    let child = table.get_mut(head)?.as_table_like_mut()?;
    table_like_at_path_mut(child, tail)
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
