use std::collections::HashMap;

use crate::config::{KeybindingsConfig, Shortcut};
use crate::{config, test_env};

use super::*;
use crate::config::action_meta::{action_description, action_label};
use crate::domain::Language;
use crate::input::state::test_support::make_test_input_state;

#[test]
fn cached_results_follow_the_active_language() {
    test_env::in_language_isolated_child(
        concat!(
            module_path!(),
            "::cached_results_follow_the_active_language"
        ),
        || {
            let mut state = make_test_input_state();
            state.set_action_bindings(HashMap::new());
            state.command_palette.query = "撤销".to_string();
            i18n::set_language(Language::English);
            assert!(
                !state
                    .filtered_commands()
                    .iter()
                    .any(|entry| entry.action == Action::Undo)
            );

            i18n::set_language(Language::SimplifiedChinese);
            assert_eq!(
                state.filtered_commands().first().map(|entry| entry.action),
                Some(Action::Undo)
            );
        },
    );
}

#[test]
fn chinese_palette_search_matches_labels_categories_and_mixed_terms() {
    test_env::in_language_isolated_child(
        concat!(
            module_path!(),
            "::chinese_palette_search_matches_labels_categories_and_mixed_terms"
        ),
        || {
            i18n::set_language(Language::SimplifiedChinese);
            let mut state = make_test_input_state();
            let mut bindings = KeybindingsConfig::default()
                .build_action_bindings()
                .expect("default bindings");
            bindings.insert(
                Action::ClearCanvas,
                vec![Shortcut::parse("Ctrl+Alt+ArrowLeft").expect("shortcut")],
            );
            state.set_action_bindings(bindings);

            for (query, action) in [
                ("撤销", Action::Undo),
                ("清空画布", Action::ClearCanvas),
                ("clear 画布", Action::ClearCanvas),
                ("undo", Action::Undo),
                ("pie menu", Action::ToggleRadialMenu),
                ("arrowleft", Action::ClearCanvas),
            ] {
                state.command_palette.query = query.to_string();
                let results = state.filtered_commands();
                assert!(
                    results.iter().any(|entry| entry.action == action),
                    "{query:?} must find {action:?}"
                );
                if query == "撤销" {
                    assert_eq!(
                        results.first().map(|entry| entry.action),
                        Some(Action::Undo)
                    );
                }
            }

            for (query, category) in [
                ("截图", ActionCategory::Capture),
                ("绘图", ActionCategory::Drawing),
            ] {
                state.command_palette.query = query.to_string();
                assert!(
                    state.filtered_commands().iter().any(|entry| {
                        entry.category == category
                            && !action_label(entry.action).contains(query)
                            && !action_description(entry.action).contains(query)
                    }),
                    "{query:?} must also find commands through their category"
                );
            }
        },
    );
}

#[test]
fn english_search_keeps_its_category_keys_and_optional_short_label_scores() {
    test_env::in_language_isolated_child(
        concat!(
            module_path!(),
            "::english_search_keeps_its_category_keys_and_optional_short_label_scores"
        ),
        || {
            let help = config::action_meta(Action::ToggleHelp).expect("help metadata");
            let next = config::action_meta(Action::BoardNext).expect("board metadata");
            for language in [Language::English, Language::SimplifiedChinese] {
                i18n::set_language(language);
                assert!(help.short_label.is_none());
                assert_eq!(action_meta_token_score(help, "help"), 75);
                assert_eq!(action_meta_query_bonus(help, "help"), 150);
                assert_eq!(next.short_label, Some("Next"));
                assert_eq!(action_meta_token_score(next, "next"), 100);
                assert_eq!(action_meta_query_bonus(next, "next"), 375);
            }

            assert_eq!(action_meta_token_score(next, "下一步"), 100);
            assert_eq!(action_meta_query_bonus(next, "下一步"), 100);

            i18n::set_language(Language::English);
            let mut state = make_test_input_state();
            state.set_action_bindings(HashMap::new());
            for (query, excluded) in [
                ("interface", Action::ToggleHelp),
                ("boards", Action::BoardNext),
            ] {
                state.command_palette.query = query.to_string();
                assert!(
                    !state
                        .filtered_commands()
                        .iter()
                        .any(|entry| entry.action == excluded)
                );
                let meta = config::action_meta(excluded).expect("metadata");
                assert_eq!(action_meta_token_score(meta, query), 0);
                assert_eq!(action_meta_query_bonus(meta, query), 0);
            }
        },
    );
}
