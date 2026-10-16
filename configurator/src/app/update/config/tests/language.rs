//! The window language follows the draft through every transition that
//! replaces it, and through none that fails or is canceled.
//!
//! Each test runs in its own process (`in_language_child`), so the language
//! it selects reaches no other test. The override and locale variables are
//! cleared there, so `auto` resolves to English.

use wayscriber::domain::Language;
use wayscriber::i18n;

use super::{app_with_config_file, load_config_file, save_draft, status_contains};
use crate::app::effects::Effect;
use crate::models::LanguageOption;

const ENGLISH_FILE: &str = "[ui]\nlanguage = \"en\"\n";
const CHINESE_FILE: &str = "[ui]\nlanguage = \"zh-CN\"\n";

#[test]
fn the_first_load_applies_the_saved_language() {
    crate::test_env::in_language_child(
        concat!(
            module_path!(),
            "::the_first_load_applies_the_saved_language"
        ),
        || {
            let (app, _dir, _path) = app_with_config_file(CHINESE_FILE);

            assert_eq!(i18n::language(), Language::SimplifiedChinese);
            assert_eq!(app.draft.ui_language, LanguageOption::SimplifiedChinese);
            // The status is built after the language changes, so it is Chinese too.
            assert!(status_contains(&app.status, "已从磁盘加载配置。"));
        },
    );
}

#[test]
fn selecting_a_language_previews_it_without_writing() {
    crate::test_env::in_language_child(
        concat!(
            module_path!(),
            "::selecting_a_language_previews_it_without_writing"
        ),
        || {
            let (mut app, _dir, path) = app_with_config_file(ENGLISH_FILE);

            let _ = app.handle_ui_language_changed(LanguageOption::SimplifiedChinese);

            assert_eq!(i18n::language(), Language::SimplifiedChinese);
            assert_eq!(app.draft.ui_language, LanguageOption::SimplifiedChinese);
            assert!(app.is_dirty);
            assert_eq!(super::read_config(&path), ENGLISH_FILE);
        },
    );
}

#[test]
fn a_successful_reload_applies_the_loaded_language() {
    crate::test_env::in_language_child(
        concat!(
            module_path!(),
            "::a_successful_reload_applies_the_loaded_language"
        ),
        || {
            let (mut app, _dir, path) = app_with_config_file(ENGLISH_FILE);
            let _ = app.handle_ui_language_changed(LanguageOption::SimplifiedChinese);

            load_config_file(&mut app, &path);

            assert_eq!(app.draft.ui_language, LanguageOption::English);
            assert_eq!(i18n::language(), Language::English);
            assert!(status_contains(
                &app.status,
                "Configuration loaded from disk."
            ));
        },
    );
}

#[test]
fn a_failed_reload_keeps_the_preview() {
    crate::test_env::in_language_child(
        concat!(module_path!(), "::a_failed_reload_keeps_the_preview"),
        || {
            let (mut app, _dir, _path) = app_with_config_file(ENGLISH_FILE);
            let _ = app.handle_ui_language_changed(LanguageOption::SimplifiedChinese);
            let _ = app.handle_reload_requested();
            let _ = app.discard_before_leave();

            let _ = app.handle_config_loaded(Err("unreadable".to_string()));

            assert_eq!(app.draft.ui_language, LanguageOption::SimplifiedChinese);
            assert_eq!(i18n::language(), Language::SimplifiedChinese);
            assert!(status_contains(&app.status, "unreadable"));
        },
    );
}

#[test]
fn confirmed_defaults_apply_the_default_language() {
    crate::test_env::in_language_child(
        concat!(
            module_path!(),
            "::confirmed_defaults_apply_the_default_language"
        ),
        || {
            let (mut app, _dir, _path) = app_with_config_file(CHINESE_FILE);
            assert_eq!(i18n::language(), Language::SimplifiedChinese);

            let _ = app.handle_reset_to_defaults_requested();
            let _ = app.handle_reset_to_defaults_confirmed();

            // The default is `auto`, which this process resolves to English.
            assert_eq!(app.draft.ui_language, LanguageOption::Auto);
            assert_eq!(i18n::language(), Language::English);
            assert!(status_contains(
                &app.status,
                "Loaded default configuration (not saved)."
            ));
        },
    );
}

#[test]
fn a_canceled_defaults_request_keeps_the_language() {
    crate::test_env::in_language_child(
        concat!(
            module_path!(),
            "::a_canceled_defaults_request_keeps_the_language"
        ),
        || {
            let (mut app, _dir, _path) = app_with_config_file(CHINESE_FILE);

            let _ = app.handle_reset_to_defaults_requested();
            let _ = app.handle_reset_to_defaults_canceled();

            assert_eq!(i18n::language(), Language::SimplifiedChinese);
        },
    );
}

#[test]
fn a_finished_save_applies_the_saved_language() {
    crate::test_env::in_language_child(
        concat!(
            module_path!(),
            "::a_finished_save_applies_the_saved_language"
        ),
        || {
            let (mut app, _dir, path) = app_with_config_file(ENGLISH_FILE);
            let _ = app.handle_ui_language_changed(LanguageOption::SimplifiedChinese);
            // Leave the preview and come back, so the save itself is what applies it.
            i18n::set_language(Language::English);

            let _ = save_draft(&mut app);

            assert!(super::read_config(&path).contains("language = \"zh-CN\""));
            assert_eq!(i18n::language(), Language::SimplifiedChinese);
            assert!(status_contains(&app.status, "配置已成功保存。"));
        },
    );
}

#[test]
fn canceling_the_leave_prompt_keeps_the_preview() {
    crate::test_env::in_language_child(
        concat!(
            module_path!(),
            "::canceling_the_leave_prompt_keeps_the_preview"
        ),
        || {
            let (mut app, _dir, _path) = app_with_config_file(ENGLISH_FILE);
            let _ = app.handle_ui_language_changed(LanguageOption::SimplifiedChinese);

            let _ = app.handle_reload_requested();
            assert!(app.pending_leave().is_some());
            let _ = app.cancel_leave();

            assert_eq!(app.draft.ui_language, LanguageOption::SimplifiedChinese);
            assert_eq!(i18n::language(), Language::SimplifiedChinese);
        },
    );
}

#[test]
fn discarding_to_reload_applies_the_document_that_loads() {
    crate::test_env::in_language_child(
        concat!(
            module_path!(),
            "::discarding_to_reload_applies_the_document_that_loads"
        ),
        || {
            let (mut app, _dir, path) = app_with_config_file(ENGLISH_FILE);
            let _ = app.handle_ui_language_changed(LanguageOption::SimplifiedChinese);
            let _ = app.handle_reload_requested();

            let effects = app.discard_before_leave();
            assert!(matches!(effects.as_slice(), [Effect::LoadConfig]));
            // Another editor saved in the meantime: what loads is what counts.
            std::fs::write(&path, CHINESE_FILE).expect("rewrite config");
            load_config_file(&mut app, &path);

            assert_eq!(app.draft.ui_language, LanguageOption::SimplifiedChinese);
            assert_eq!(i18n::language(), Language::SimplifiedChinese);
            assert!(!app.is_dirty);
        },
    );
}
