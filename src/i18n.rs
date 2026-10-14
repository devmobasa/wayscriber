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
use std::path::PathBuf;
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

/// Selects the language the UI will draw in.
///
/// `Auto` resolves through the locale chain, and `WAYSCRIBER_LANGUAGE`
/// overrides the argument entirely — a testing aid for checking a translation
/// without editing `config.toml`. Call this once at startup, before the first
/// surface is built.
pub fn set_language(language: Language) {
    let active = select(language, env_var(LANGUAGE_OVERRIDE_ENV).as_deref());

    ACTIVE.store(encode(active), Ordering::Relaxed);
    log::debug!("UI language: {}", active.code());
}

/// The selection rule: `WAYSCRIBER_LANGUAGE` beats the configured language,
/// which beats nothing — an explicit language is already final.
///
/// Pure so the precedence is testable without touching the environment.
fn select(configured: Language, override_code: Option<&str>) -> Language {
    if let Some(code) = override_code {
        match language_from_code(code) {
            Some(language) => return resolve(language),
            None => {
                log::warn!("{LANGUAGE_OVERRIDE_ENV}={code} is not a known language; ignoring it");
            }
        }
    }

    resolve(configured)
}

/// The language currently in effect; never [`Language::Auto`].
pub fn language() -> Language {
    decode(ACTIVE.load(Ordering::Relaxed))
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

/// The lookup [`tr`] performs, split out so tests can exercise a language
/// without moving the process-global selection.
fn lookup<'a>(language: Language, source: &'a str) -> &'a str {
    let Some(catalog) = language.catalog_code().and_then(catalog) else {
        return source;
    };

    catalog.get(source).map(String::as_str).unwrap_or(source)
}

/// Translates `source` and substitutes each `{}` with the next argument.
///
/// Placeholders are positional, so a catalog may reorder the sentence around
/// them. Missing arguments leave `{}` in place; extra arguments are ignored.
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

/// The embedded catalogs, each merged with its user override, built once.
fn catalogs() -> &'static [(&'static str, Catalog)] {
    static CATALOGS: OnceLock<Vec<(&'static str, Catalog)>> = OnceLock::new();

    CATALOGS.get_or_init(|| {
        EMBEDDED_CATALOGS
            .iter()
            .map(|(code, source)| {
                let mut catalog = parse_catalog(source, code);
                if let Some(overrides) = read_user_catalog(code) {
                    catalog.extend(parse_catalog(&overrides, code));
                }
                (*code, catalog)
            })
            .collect()
    })
}

fn catalog(code: &str) -> Option<&'static Catalog> {
    catalogs()
        .iter()
        .find(|(entry, _)| *entry == code)
        .map(|(_, catalog)| catalog)
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

fn read_user_catalog(code: &str) -> Option<String> {
    let path = locale_dir()?.join(format!("{code}.toml"));
    match std::fs::read_to_string(&path) {
        Ok(source) => {
            log::debug!("Locale override loaded from {}", path.display());
            Some(source)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => {
            log::warn!("Locale override {} could not be read: {err}", path.display());
            None
        }
    }
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
