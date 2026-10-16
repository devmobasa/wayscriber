//! UI translations (`[ui] language`).
//!
//! English source strings are the catalog keys, the way gettext uses `msgid`:
//! `tr("Boards")` draws the string the active language supplies and falls
//! through to `"Boards"` when that language has no entry. Because lookup is by
//! exact match, wrapping a string that is never translated is free, and a
//! catalog can cover the UI incrementally.
//!
//! Catalogs are TOML tables in `locales/`, embedded at build time. A file the
//! user drops in `$XDG_DATA_HOME/wayscriber/locales/<code>.toml` (or the
//! directory `WAYSCRIBER_LOCALE_DIR` names) merges over the embedded one, so a
//! translation can be corrected without rebuilding. Runtime language
//! selection is process-global state, read by chrome renderers; the settings
//! that drive it stay in `UiConfig`/`InputState` like every other preference.
//!
//! Only UI chrome goes through here. Canvas content — typed text, sticky
//! notes, board names — is user data and is never translated.

use std::collections::BTreeMap;
use std::fmt::{Display, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::domain::Language;
use crate::env_vars::{LANGUAGE_OVERRIDE_ENV, LOCALE_DIR_ENV};

/// Translation catalogs shipped in the binary. Adding one means adding a
/// `locales/<code>.toml`, a `Language` variant, and an entry here.
const EMBEDDED_CATALOGS: &[(&str, &str)] = &[("zh-CN", include_str!("../locales/zh-CN.toml"))];

/// Source string to translated string.
type Catalog = BTreeMap<String, String>;

/// Atoms: English is 0 so a never-configured process needs no catalog.
static ACTIVE: AtomicU8 = AtomicU8::new(0);

/// The `--language` this process runs with: its own, or, for an overlay, its
/// daemon's.
static COMMAND_LINE: OnceLock<Language> = OnceLock::new();

/// Selects the language the UI will draw in.
///
/// `Auto` resolves through the locale chain. `--language` and then
/// `WAYSCRIBER_LANGUAGE` override the argument entirely, for one run, without
/// editing `config.toml`. Each process calls this before it builds translated
/// chrome; the configurator calls it again whenever the language of its draft
/// changes, to preview that language.
pub fn set_language(language: Language) {
    let active = resolve(requested_language(language).0);

    ACTIVE.store(encode(active), Ordering::Relaxed);
    log::debug!("UI language: {}", active.code());
}

/// Records `--language` for the rest of this process. Set once, at startup,
/// before any language is selected.
pub fn set_command_line_language(language: Language) {
    let _ = COMMAND_LINE.set(language);
}

/// The `--language` this process was started with, so the overlays and About
/// windows it starts can be given the same one.
pub fn command_line_language() -> Option<Language> {
    COMMAND_LINE.get().copied()
}

/// Which setting decided the language in effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageSource {
    /// `--language`.
    CommandLine,
    /// The `WAYSCRIBER_LANGUAGE` environment variable.
    Environment,
    /// `[ui] language` in the config.
    Configured,
}

/// The language the deciding setting asks for, possibly [`Language::Auto`],
/// and which setting that is, given the configured language.
pub fn requested_language(configured: Language) -> (Language, LanguageSource) {
    select(
        configured,
        command_line_language(),
        env_var(LANGUAGE_OVERRIDE_ENV).as_deref(),
    )
}

/// Start-up for a process that draws translated chrome: selects the
/// configured language and, when it draws from a catalog, loads that catalog
/// before the first surface is built. The overlay, the daemon and the About
/// window call this with their loaded config; the configurator previews
/// languages instead, and prepares its catalogs from a blocking job.
pub fn initialize(language: Language) {
    set_language(language);
    if self::language().catalog_code().is_some() {
        prepare_catalogs();
    }
}

/// The selection rule: `--language` beats `WAYSCRIBER_LANGUAGE`, which beats
/// the configured language. The winner may still be `Auto`; [`resolve`]
/// decides what that draws.
///
/// Pure so the precedence is testable without touching the environment.
fn select(
    configured: Language,
    command_line: Option<Language>,
    override_code: Option<&str>,
) -> (Language, LanguageSource) {
    if let Some(language) = command_line {
        return (language, LanguageSource::CommandLine);
    }
    if let Some(code) = override_code {
        match language_from_code(code) {
            Some(language) => return (language, LanguageSource::Environment),
            None => {
                log::warn!("{LANGUAGE_OVERRIDE_ENV}={code} is not a known language; ignoring it");
            }
        }
    }

    (configured, LanguageSource::Configured)
}

/// The language currently in effect; never [`Language::Auto`].
pub fn language() -> Language {
    decode(ACTIVE.load(Ordering::Relaxed))
}

/// Puts back a language a test read from [`language`], as it was: already
/// resolved, so neither the override nor the locale chain applies again.
#[cfg(test)]
pub(crate) fn restore_language(language: Language) {
    ACTIVE.store(encode(language), Ordering::Relaxed);
}

/// Resolves a configured language to the one actually drawn.
///
/// Explicit languages resolve to themselves (English is the fallback, so
/// asking for it is always satisfiable). `Auto` follows
/// [`detect_from_env`], and a locale without a catalog falls back to English.
pub fn resolve(language: Language) -> Language {
    if !language.is_auto() {
        return language;
    }

    detect_from_env()
}

/// Reads the desktop locale, most specific variable first.
///
/// `LANGUAGE` is a colon-separated priority list (`zh_CN:en`), so its first
/// translated entry wins. Locales without a catalog leave the slot empty and
/// the search continues; English is the final fallback.
pub fn detect_from_env() -> Language {
    // The test suite asserts source-language strings, so the locale of whoever
    // runs it must not leak in: a Chinese desktop would otherwise translate the
    // diagnostics other tests compare against. Detection itself is covered by
    // `language_from_locale_chain` and by tests/i18n.rs, which owns its own
    // process and can set the variables safely.
    #[cfg(test)]
    {
        Language::English
    }

    #[cfg(not(test))]
    {
        for name in crate::env_vars::LOCALE_ENV_KEYS {
            let Some(value) = env_var(name) else {
                continue;
            };

            if let Some(language) = language_from_locale_chain(&value) {
                return language;
            }
        }

        Language::English
    }
}

/// The first translated locale in a `LANGUAGE`-style colon list.
pub fn language_from_locale_chain(value: &str) -> Option<Language> {
    value.split(':').find_map(language_from_locale)
}

/// Maps a POSIX locale such as `zh_CN.UTF-8` to a language.
///
/// Returns `None` when this build has no catalog for the locale, which is the
/// caller's cue to keep looking or fall back to English. `zh_TW`/`zh_HK` are
/// Traditional Chinese and stay untranslated until they have their own
/// catalog.
pub fn language_from_locale(locale: &str) -> Option<Language> {
    let normalized = locale
        .trim()
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .replace('_', "-")
        .to_ascii_lowercase();
    let mut parts = normalized.split('-');
    let base = parts.next().unwrap_or_default();
    let region = parts.next().unwrap_or_default();

    match base {
        "en" | "c" | "posix" => Some(Language::English),
        "zh" if matches!(region, "tw" | "hk" | "mo" | "hant") => None,
        "zh" => Some(Language::SimplifiedChinese),
        _ => None,
    }
}

/// Parses the spellings `WAYSCRIBER_LANGUAGE` and configuration accept.
pub fn language_from_code(code: &str) -> Option<Language> {
    match code.trim().to_ascii_lowercase().as_str() {
        "auto" => Some(Language::Auto),
        "en" | "english" => Some(Language::English),
        "zh" | "zh-cn" | "zh-hans" | "chinese" => Some(Language::SimplifiedChinese),
        _ => None,
    }
}

/// Translates one UI source string.
///
/// The argument must be a literal so that the guard test can find every
/// translatable string in the sources. Unknown strings pass through.
pub fn tr(source: &'static str) -> &'static str {
    lookup(language(), source)
}

/// The lookup [`tr`] performs, for a language the caller names instead of the
/// process-global one. Tables built for one language use it so they never mix
/// two; bind it to a local named `tr` so the catalog guard still reads each
/// literal.
pub(crate) fn lookup(language: Language, source: &str) -> &str {
    let Some(catalog) = language.catalog_code().and_then(catalog) else {
        return source;
    };

    catalog.get(source).map(String::as_str).unwrap_or(source)
}

/// Translates `source` and substitutes each `{}` with the next argument.
///
/// Arguments fill the placeholders in order: the first `{}` in the translated
/// text always receives the first argument. A translation can place its
/// placeholders anywhere in the sentence but cannot swap two values, and only
/// anonymous `{}` is supported; `{name}` and `{0}` stay literal. Missing
/// arguments leave `{}` in place; extra arguments are ignored.
pub fn trf(source: &'static str, args: &[&dyn Display]) -> String {
    substitute(tr(source), args)
}

fn substitute(template: &str, args: &[&dyn Display]) -> String {
    if args.is_empty() {
        return template.to_string();
    }

    let mut result = String::with_capacity(template.len() + 16);
    let mut remaining = template;
    let mut arguments = args.iter();

    while let Some(index) = remaining.find("{}") {
        result.push_str(&remaining[..index]);
        match arguments.next() {
            Some(argument) => {
                let _ = write!(result, "{argument}");
            }
            None => result.push_str("{}"),
        }
        remaining = &remaining[index + 2..];
    }
    result.push_str(remaining);

    result
}

/// How many entries the active catalog holds; `0` for English.
pub fn catalog_len() -> usize {
    active_catalog().map_or(0, Catalog::len)
}

fn encode(language: Language) -> u8 {
    match language {
        Language::SimplifiedChinese => 1,
        // Auto never reaches here; treating it as English keeps a stray call
        // from silencing the UI.
        Language::Auto | Language::English => 0,
    }
}

fn decode(value: u8) -> Language {
    if value == 1 {
        Language::SimplifiedChinese
    } else {
        Language::English
    }
}

fn active_catalog() -> Option<&'static Catalog> {
    language().catalog_code().and_then(catalog)
}

/// Loads every catalog and its user override now, instead of on the first
/// translated lookup.
///
/// Reading an override is file I/O. A process calls this where that is
/// acceptable, before it draws translated chrome: the overlay, daemon and
/// About window at startup, the configurator from its blocking load job. A
/// lookup before that still loads the catalogs itself, as a fallback.
pub fn prepare_catalogs() {
    let _ = catalogs();
}

/// The embedded catalogs, each merged with its user override, built once.
fn catalogs() -> &'static [(&'static str, Catalog)] {
    static CATALOGS: OnceLock<Vec<(&'static str, Catalog)>> = OnceLock::new();

    CATALOGS.get_or_init(|| {
        let directory = locale_dir();
        EMBEDDED_CATALOGS
            .iter()
            .map(|(code, source)| (*code, load_catalog(code, source, directory.as_deref())))
            .collect()
    })
}

fn catalog(code: &str) -> Option<&'static Catalog> {
    catalogs()
        .iter()
        .find(|(entry, _)| *entry == code)
        .map(|(_, catalog)| catalog)
}

/// One embedded catalog with the override `directory` holds for it merged
/// over it. A rejected override, or a rejected entry in one, leaves the
/// embedded entries in place.
fn load_catalog(code: &str, embedded: &str, directory: Option<&Path>) -> Catalog {
    let mut catalog = parse_catalog(embedded, code);
    let Some(directory) = directory else {
        return catalog;
    };

    let path = directory.join(format!("{code}.toml"));
    let source = match read_override(&path) {
        Ok(source) => source,
        Err(OverrideRejection::Missing) => return catalog,
        Err(rejection) => {
            log::warn!("Locale override {} ignored: {rejection}", path.display());
            return catalog;
        }
    };

    let mut overrides = parse_catalog(&source, code);
    let malformed: Vec<String> = overrides
        .iter()
        .filter(|(key, value)| key.matches("{}").count() != value.matches("{}").count())
        .map(|(key, _)| key.clone())
        .collect();
    if !malformed.is_empty() {
        log::warn!(
            "Locale override {} keeps the embedded text for {} entries whose {{}} placeholders \
             do not match their key, such as {:?}",
            path.display(),
            malformed.len(),
            malformed[0]
        );
        overrides.retain(|key, _| !malformed.contains(key));
    }
    log::debug!("Locale override loaded from {}", path.display());
    catalog.extend(overrides);

    catalog
}

fn parse_catalog(source: &str, code: &str) -> Catalog {
    match toml::from_str::<Catalog>(source) {
        Ok(catalog) => catalog,
        Err(err) => {
            // A translation must never break the overlay: keep the English
            // source strings and say why in the log.
            log::warn!("Locale catalog {code} could not be read: {err}");
            Catalog::new()
        }
    }
}

/// The largest override read. The embedded Chinese catalog is about 140 KB.
const MAX_OVERRIDE_BYTES: u64 = 1024 * 1024;

/// Why an override file was not used.
#[derive(Debug)]
enum OverrideRejection {
    Missing,
    SymbolicLink,
    NotARegularFile,
    TooLarge,
    InvalidUtf8,
    Unreadable(std::io::Error),
}

impl std::fmt::Display for OverrideRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => formatter.write_str("no such file"),
            Self::SymbolicLink => formatter.write_str("it is a symbolic link"),
            Self::NotARegularFile => formatter.write_str("it is not a regular file"),
            Self::TooLarge => write!(formatter, "it is larger than {MAX_OVERRIDE_BYTES} bytes"),
            Self::InvalidUtf8 => formatter.write_str("it is not UTF-8"),
            Self::Unreadable(err) => write!(formatter, "{err}"),
        }
    }
}

/// Reads one override file, refusing anything but a bounded regular file.
///
/// The open does not follow a symlink and does not block, so a FIFO cannot
/// stall the first translated frame. The checks run on the opened descriptor,
/// so the file cannot be swapped between a check and the read.
fn read_override(path: &Path) -> Result<String, OverrideRejection> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;

    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|err| match err.raw_os_error() {
            _ if err.kind() == std::io::ErrorKind::NotFound => OverrideRejection::Missing,
            Some(libc::ELOOP) => OverrideRejection::SymbolicLink,
            _ => OverrideRejection::Unreadable(err),
        })?;
    let metadata = file.metadata().map_err(OverrideRejection::Unreadable)?;
    if !metadata.is_file() {
        return Err(OverrideRejection::NotARegularFile);
    }

    let mut bytes = Vec::new();
    file.take(MAX_OVERRIDE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(OverrideRejection::Unreadable)?;
    if bytes.len() as u64 > MAX_OVERRIDE_BYTES {
        return Err(OverrideRejection::TooLarge);
    }

    String::from_utf8(bytes).map_err(|_| OverrideRejection::InvalidUtf8)
}

fn locale_dir() -> Option<PathBuf> {
    if let Some(dir) = env_var(LOCALE_DIR_ENV) {
        return Some(PathBuf::from(dir));
    }

    crate::paths::data_dir().map(|dir| dir.join("locales"))
}

fn env_var(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
