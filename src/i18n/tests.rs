use std::collections::BTreeMap;
use std::ffi::CString;
use std::os::unix;
use std::sync::mpsc;
use std::time::Duration;
use std::{fs, thread};

use crate::test_env;

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
    // Catalogs load once per process, with the runner's override directory
    // if any; the child's is empty, so lookup serves the embedded entries.
    test_env::in_language_isolated_child(
        concat!(module_path!(), "::simplified_chinese_serves_translations"),
        || {
            let catalog = catalog_pairs();
            assert!(!catalog.is_empty(), "the zh-CN catalog must not be empty");

            for (source, translation) in &catalog {
                assert_eq!(
                    lookup(Language::SimplifiedChinese, source),
                    translation,
                    "{source:?}"
                );
            }
        },
    );
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
fn the_command_line_beats_the_environment_which_beats_the_config() {
    use LanguageSource::{CommandLine, Configured, Environment};

    assert_eq!(
        select(Language::English, None, Some("zh-CN")),
        (Language::SimplifiedChinese, Environment)
    );
    assert_eq!(
        select(Language::SimplifiedChinese, None, Some("en")),
        (Language::English, Environment)
    );
    assert_eq!(
        select(Language::SimplifiedChinese, None, None),
        (Language::SimplifiedChinese, Configured)
    );
    assert_eq!(
        select(
            Language::English,
            Some(Language::SimplifiedChinese),
            Some("en")
        ),
        (Language::SimplifiedChinese, CommandLine)
    );
    // `--language auto` follows the desktop locale whatever the config says.
    assert_eq!(
        select(Language::SimplifiedChinese, Some(Language::Auto), None),
        (Language::Auto, CommandLine)
    );
    // An unknown override is ignored rather than silencing the UI.
    assert_eq!(
        select(Language::English, None, Some("klingon")),
        (Language::English, Configured)
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

        let arguments: Vec<&dyn Display> = vec![&"x"; placeholders];
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

/// The embedded catalog with whatever `contents` an override file holds.
fn with_override(contents: &[u8]) -> Catalog {
    let directory = tempfile::tempdir().expect("override directory");
    fs::write(directory.path().join("zh-CN.toml"), contents).expect("write override");

    load_catalog("zh-CN", SIMPLIFIED_CHINESE_CATALOG, Some(directory.path()))
}

#[test]
fn a_valid_override_replaces_only_its_entries() {
    let embedded = catalog_pairs();

    let catalog = with_override("\"Boards\" = \"白板\"\n".as_bytes());

    assert_eq!(catalog.get("Boards").map(String::as_str), Some("白板"));
    assert_eq!(catalog.len(), embedded.len());
    assert_eq!(catalog.get("Undo"), embedded.get("Undo"));
}

#[test]
fn without_an_override_the_embedded_catalog_is_used() {
    let embedded = catalog_pairs();
    let empty = tempfile::tempdir().expect("override directory");

    assert_eq!(
        load_catalog("zh-CN", SIMPLIFIED_CHINESE_CATALOG, Some(empty.path())),
        embedded
    );
    assert_eq!(
        load_catalog("zh-CN", SIMPLIFIED_CHINESE_CATALOG, None),
        embedded
    );
}

#[test]
fn a_rejected_override_keeps_the_embedded_entries() {
    let embedded = catalog_pairs();
    let mut oversize = b"\"Boards\" = \"".to_vec();
    oversize.resize(MAX_OVERRIDE_BYTES as usize + 1, b'x');
    oversize.extend_from_slice(b"\"\n");

    for (case, contents) in [
        ("malformed TOML", b"\"Boards\" = \n".to_vec()),
        ("invalid UTF-8", b"\"Boards\" = \"\xff\"\n".to_vec()),
        ("oversize", oversize),
    ] {
        assert_eq!(with_override(&contents), embedded, "{case}");
    }
}

#[test]
fn an_override_entry_that_changes_its_placeholders_is_dropped() {
    let embedded = catalog_pairs();
    const KEY: &str = "Background setup action failed: {}";
    assert!(embedded.contains_key(KEY));

    let catalog =
        with_override(format!("\"{KEY}\" = \"失败\"\n\"Boards\" = \"白板\"\n").as_bytes());

    assert_eq!(catalog.get(KEY), embedded.get(KEY));
    assert_eq!(catalog.get("Boards").map(String::as_str), Some("白板"));
}

#[test]
fn an_override_that_is_too_large_is_refused_after_a_bounded_read() {
    let directory = tempfile::tempdir().expect("override directory");
    let path = directory.path().join("zh-CN.toml");
    fs::write(&path, vec![b'#'; MAX_OVERRIDE_BYTES as usize + 1]).expect("write");

    assert!(matches!(
        read_override(&path),
        Err(OverrideRejection::TooLarge)
    ));

    fs::write(&path, vec![b'#'; MAX_OVERRIDE_BYTES as usize]).expect("write");
    assert!(read_override(&path).is_ok(), "the limit itself is allowed");
}

#[test]
fn a_symlinked_override_is_refused() {
    let directory = tempfile::tempdir().expect("override directory");
    let target = directory.path().join("real.toml");
    fs::write(&target, "\"Boards\" = \"白板\"\n").expect("write target");
    unix::fs::symlink(&target, directory.path().join("zh-CN.toml")).expect("symlink");

    assert!(matches!(
        read_override(&directory.path().join("zh-CN.toml")),
        Err(OverrideRejection::SymbolicLink)
    ));
    assert_eq!(
        load_catalog("zh-CN", SIMPLIFIED_CHINESE_CATALOG, Some(directory.path())),
        catalog_pairs()
    );
}

#[test]
fn a_fifo_override_is_refused_without_waiting_for_a_writer() {
    let directory = tempfile::tempdir().expect("override directory");
    let path = directory.path().join("zh-CN.toml");
    let raw_path = CString::new(path.as_os_str().as_encoded_bytes()).expect("path");
    // SAFETY: `raw_path` is a valid NUL-terminated path for the call.
    assert_eq!(unsafe { libc::mkfifo(raw_path.as_ptr(), 0o600) }, 0);

    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(matches!(
            read_override(&path),
            Err(OverrideRejection::NotARegularFile)
        ));
    });

    // No process ever opens the write end, so a blocking open never returns.
    let refused = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("reading a FIFO override must not block");
    assert!(refused);
}

#[test]
fn the_first_lookup_merges_the_override_directory() {
    let directory = tempfile::tempdir().expect("override directory");
    fs::write(
        directory.path().join("zh-CN.toml"),
        "\"Boards\" = \"白板\"\n",
    )
    .expect("write override");

    test_env::in_language_isolated_child_with_overrides(
        concat!(
            module_path!(),
            "::the_first_lookup_merges_the_override_directory"
        ),
        directory.path(),
        || {
            initialize(Language::SimplifiedChinese);

            assert_eq!(tr("Boards"), "白板");
            assert_eq!(tr("Undo"), catalog_pairs()["Undo"]);
        },
    );
}

#[test]
fn the_command_line_language_holds_for_the_whole_process() {
    test_env::in_language_isolated_child(
        concat!(
            module_path!(),
            "::the_command_line_language_holds_for_the_whole_process"
        ),
        || {
            // SAFETY: this child process runs only this test.
            unsafe { env::set_var(LANGUAGE_OVERRIDE_ENV, "en") };
            set_command_line_language(Language::SimplifiedChinese);

            set_language(Language::English);

            assert_eq!(command_line_language(), Some(Language::SimplifiedChinese));
            assert_eq!(language(), Language::SimplifiedChinese);
            assert_eq!(
                requested_language(Language::English),
                (Language::SimplifiedChinese, LanguageSource::CommandLine)
            );
        },
    );
}
