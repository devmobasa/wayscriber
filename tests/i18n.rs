//! Language selection end to end.
//!
//! The crate's own unit tests always read source strings, so the environment
//! path is covered here, in a process this test owns: configuration, the
//! `WAYSCRIBER_LANGUAGE` override, and the POSIX locale chain.

use std::env;
use wayscriber::domain::Language;
use wayscriber::i18n;

const OVERRIDE: &str = "WAYSCRIBER_LANGUAGE";
const LOCALE_DIR: &str = "WAYSCRIBER_LOCALE_DIR";

fn set_env(name: &str, value: &str) {
    // SAFETY: this test binary holds a single test, which spawns no threads,
    // so nothing else in the process reads or writes the environment.
    unsafe { env::set_var(name, value) };
}

fn unset_env(name: &str) {
    // SAFETY: this test binary holds a single test, which spawns no threads,
    // so nothing else in the process reads or writes the environment.
    unsafe { env::remove_var(name) };
}

fn clear_language_environment() {
    unset_env(OVERRIDE);
    for name in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        unset_env(name);
    }
}

#[test]
fn configured_and_ambient_languages_select_the_catalog() {
    clear_language_environment();
    // The catalogs load once, on the first translated lookup below: point the
    // override directory at an empty one so the embedded entries are what
    // this process reads, whatever the runner has installed.
    let no_overrides = tempfile::tempdir().expect("empty override directory");
    set_env(
        LOCALE_DIR,
        no_overrides.path().to_str().expect("UTF-8 temp path"),
    );

    // An explicit language wins over a translated desktop.
    set_env("LANG", "zh_CN.UTF-8");
    i18n::set_language(Language::English);
    assert_eq!(i18n::language(), Language::English);
    assert_eq!(i18n::tr("Clear Canvas"), "Clear Canvas");

    // `auto` follows the desktop locale once the config asks for it.
    i18n::set_language(Language::Auto);
    assert_eq!(i18n::language(), Language::SimplifiedChinese);
    assert_eq!(i18n::tr("Clear Canvas"), "清空画布");
    assert_eq!(
        i18n::trf("{} pages", &[&3]),
        "3 pages",
        "a source string without a catalog entry still fills its arguments"
    );
    assert!(i18n::catalog_len() > 0);

    // A locale with no catalog falls back to English instead of failing.
    set_env("LANG", "de_DE.UTF-8");
    i18n::set_language(Language::Auto);
    assert_eq!(i18n::language(), Language::English);

    // The override variable beats both the locale and the configuration.
    set_env("LANG", "de_DE.UTF-8");
    set_env(OVERRIDE, "zh-CN");
    i18n::set_language(Language::English);
    assert_eq!(i18n::language(), Language::SimplifiedChinese);

    // An override nobody understands is ignored, not fatal.
    set_env(OVERRIDE, "klingon");
    i18n::set_language(Language::English);
    assert_eq!(i18n::language(), Language::English);

    // Strings without a catalog entry pass through unchanged, which is what
    // keeps user text and unknown labels safe.
    assert_eq!(i18n::tr("Untranslated sample"), "Untranslated sample");

    clear_language_environment();
    i18n::set_language(Language::English);
}
