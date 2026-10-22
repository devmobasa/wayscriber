//! The daemon's UI language.
//!
//! The tray menu, the portal's shortcut descriptions and the update notices
//! are chrome this process draws. An overlay is a separate child process that
//! selects its own language when it loads the config; it cannot set this one.

use log::warn;

use crate::config::Config;
use crate::domain::Language;
use crate::i18n;

/// Selects the saved `[ui] language`, once, before the tray, shortcut and
/// update-notice workers start, whichever of them are enabled. A change takes
/// effect when the daemon restarts.
pub(super) fn initialize_ui_language() {
    i18n::initialize(configured_language());
}

/// The saved language, or the default (`auto`) when the config cannot be
/// read: a broken file must not stop the daemon, and the desktop locale is
/// the best remaining guess.
fn configured_language() -> Language {
    match Config::load() {
        Ok(loaded) => loaded.config.ui.language,
        Err(err) => {
            warn!("Failed to read config for the UI language; following the desktop locale: {err}");
            Language::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use crate::{env_vars, test_env};

    use super::*;
    use crate::config::test_helpers::with_temp_config_home;

    fn write_config(contents: &str) {
        let path = Config::config_path().expect("config path");
        fs::create_dir_all(path.parent().expect("config dir")).expect("create config dir");
        fs::write(path, contents).expect("write config");
    }

    #[test]
    fn the_saved_language_is_the_one_selected() {
        for (contents, expected) in [
            ("[ui]\nlanguage = \"zh-CN\"\n", Language::SimplifiedChinese),
            ("[ui]\nlanguage = \"zh\"\n", Language::SimplifiedChinese),
            ("[ui]\nlanguage = \"en\"\n", Language::English),
            ("[ui]\nlanguage = \"auto\"\n", Language::Auto),
            ("", Language::Auto),
        ] {
            with_temp_config_home(|_| {
                write_config(contents);

                assert_eq!(configured_language(), expected, "{contents:?}");
            });
        }
    }

    #[test]
    fn an_unreadable_config_keeps_the_automatic_default() {
        with_temp_config_home(|_| {
            write_config("[ui\nlanguage = \"zh-CN\"\n");

            assert_eq!(configured_language(), Language::Auto);
        });
    }

    #[test]
    fn startup_selects_and_prepares_the_saved_language() {
        test_env::in_language_isolated_child(
            concat!(
                module_path!(),
                "::startup_selects_and_prepares_the_saved_language"
            ),
            || {
                with_temp_config_home(|_| {
                    write_config("[ui]\nlanguage = \"zh-CN\"\n");

                    initialize_ui_language();

                    assert_eq!(i18n::language(), Language::SimplifiedChinese);
                    assert!(i18n::catalog_len() > 0);
                    assert_eq!(i18n::tr("Boards"), "画板");
                });
            },
        );
    }

    #[test]
    fn the_override_variable_still_beats_the_saved_language() {
        test_env::in_language_isolated_child(
            concat!(
                module_path!(),
                "::the_override_variable_still_beats_the_saved_language"
            ),
            || {
                with_temp_config_home(|_| {
                    write_config("[ui]\nlanguage = \"en\"\n");
                    // SAFETY: this child process runs only this test.
                    unsafe {
                        env::set_var(env_vars::LANGUAGE_OVERRIDE_ENV, "zh-CN");
                    }

                    initialize_ui_language();

                    assert_eq!(i18n::language(), Language::SimplifiedChinese);
                });
            },
        );
    }
}
