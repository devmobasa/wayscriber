use std::collections::BTreeMap;

use super::*;

const SIMPLIFIED_CHINESE_CATALOG: &str = include_str!("../../locales/zh-CN.toml");

fn catalog_pairs() -> BTreeMap<String, String> {
    toml::from_str(SIMPLIFIED_CHINESE_CATALOG).expect("zh-CN catalog should parse")
}

#[test]
fn english_source_strings_are_the_identity_catalog() {
    assert_eq!(lookup(Language::English, "Boards"), "Boards");
    assert_eq!(
        lookup(Language::English, "not a UI string"),
        "not a UI string"
    );
}

#[test]
fn simplified_chinese_serves_translations() {
    let catalog = catalog_pairs();
    assert!(!catalog.is_empty(), "the zh-CN catalog must not be empty");

    for (source, translation) in &catalog {
        assert_eq!(
            lookup(Language::SimplifiedChinese, source),
            translation,
            "{source:?}"
        );
    }
}

#[test]
fn unknown_strings_fall_through_in_every_language() {
    const UNTRANSLATED: &str = "4d1f0c31 no catalog entry";

    assert_eq!(
        lookup(Language::SimplifiedChinese, UNTRANSLATED),
        UNTRANSLATED
    );
    assert_eq!(lookup(Language::English, UNTRANSLATED), UNTRANSLATED);
    assert_eq!(lookup(Language::Auto, UNTRANSLATED), UNTRANSLATED);
}

#[test]
fn explicit_languages_never_resolve_to_auto() {
    assert_eq!(resolve(Language::English), Language::English);
    assert_eq!(
        resolve(Language::SimplifiedChinese),
        Language::SimplifiedChinese
    );
    assert!(!resolve(Language::Auto).is_auto());
}

#[test]
fn environment_override_beats_the_configured_language() {
    assert_eq!(
        select(Language::English, Some("zh-CN")),
        Language::SimplifiedChinese
    );
    assert_eq!(
        select(Language::SimplifiedChinese, Some("en")),
        Language::English
    );
    assert_eq!(select(Language::English, None), Language::English);
    assert_eq!(
        select(Language::SimplifiedChinese, None),
        Language::SimplifiedChinese
    );
    // An unknown override is ignored rather than silencing the UI.
    assert_eq!(
        select(Language::English, Some("klingon")),
        Language::English
    );
}

#[test]
fn locale_spellings_map_to_languages() {
    for locale in ["zh_CN.UTF-8", "zh-CN", "zh", "zh_CN@variant", "zh-Hans"] {
        assert_eq!(
            language_from_locale(locale),
            Some(Language::SimplifiedChinese),
            "locale {locale}"
        );
    }

    for locale in ["en_US.UTF-8", "C", "POSIX"] {
        assert_eq!(
            language_from_locale(locale),
            Some(Language::English),
            "locale {locale}"
        );
    }

    // Traditional Chinese has no catalog, and neither does an unknown locale:
    // both keep the search going.
    for locale in ["zh_TW.UTF-8", "zh-HK", "de_DE.UTF-8", ""] {
        assert_eq!(language_from_locale(locale), None, "locale {locale}");
    }
}

#[test]
fn locale_chains_take_the_first_translated_entry() {
    assert_eq!(
        language_from_locale_chain("zh_CN:en"),
        Some(Language::SimplifiedChinese)
    );
    assert_eq!(
        language_from_locale_chain("de_DE:zh_TW:en_US"),
        Some(Language::English)
    );
    assert_eq!(language_from_locale_chain("de_DE:fr_FR"), None);
}

#[test]
fn language_codes_accept_the_configured_spellings() {
    assert_eq!(language_from_code("auto"), Some(Language::Auto));
    assert_eq!(
        language_from_code("zh-CN"),
        Some(Language::SimplifiedChinese)
    );
    assert_eq!(
        language_from_code(" ZH-cn "),
        Some(Language::SimplifiedChinese)
    );
    assert_eq!(language_from_code("en"), Some(Language::English));
    assert_eq!(language_from_code("klingon"), None);
}

#[test]
fn unit_tests_keep_the_source_language_on_any_host_locale() {
    // Detection is deliberately neutral inside the crate: the suite compares
    // source-language text, so a translated desktop must not change what it
    // sees. tests/i18n.rs owns its own process and covers the environment path.
    assert_eq!(detect_from_env(), Language::English);
}

#[test]
fn placeholders_substitute_positionally_in_order() {
    assert_eq!(substitute("{} of {}", &[&1, &2]), "1 of 2");
    assert_eq!(substitute("no placeholders", &[]), "no placeholders");
    // A missing argument stays visible instead of silently dropping text.
    assert_eq!(substitute("{} and {}", &[&1]), "1 and {}");
    // Extra arguments are ignored.
    assert_eq!(substitute("{}", &[&1, &2]), "1");
}

#[test]
fn catalog_placeholders_survive_translation() {
    for (source, translation) in catalog_pairs() {
        let placeholders = source.matches("{}").count();
        if placeholders == 0 {
            continue;
        }

        assert_eq!(
            translation.matches("{}").count(),
            placeholders,
            "{source:?} changes the placeholder count"
        );

        let arguments: Vec<&dyn std::fmt::Display> = vec![&"x"; placeholders];
        let rendered = substitute(&translation, &arguments);
        assert!(
            !rendered.contains("{}"),
            "placeholder lost rendering {rendered:?}"
        );
    }
}

#[test]
fn catalog_entries_are_translated_and_non_empty() {
    let catalog = catalog_pairs();
    assert!(!catalog.is_empty(), "the zh-CN catalog must not be empty");

    for (source, translation) in &catalog {
        assert!(
            !translation.trim().is_empty(),
            "{source:?} must translate to something"
        );
        assert_ne!(
            translation.trim(),
            source.trim(),
            "{source:?} is unchanged; drop the entry or translate it"
        );
    }
}
