//! `--list-languages`: the interface languages, and the one this run uses.
//!
//! Command-line output stays English, like the rest of the CLI.

use crate::cli::Cli;
use crate::config::Config;
use crate::domain::Language;
use crate::env_vars::OVERLAY_LANGUAGE_ENV;
use crate::i18n::{self, LanguageSource};

/// The language this run was given: its own `--language`, or, for an overlay
/// a daemon started, that daemon's.
pub(super) fn given_language(cli: &Cli) -> Option<Language> {
    given_language_with(cli, std::env::var(OVERLAY_LANGUAGE_ENV).ok().as_deref())
}

fn given_language_with(cli: &Cli, from_daemon: Option<&str>) -> Option<Language> {
    cli.language.or_else(|| {
        from_daemon
            .filter(|_| cli.active || cli.freeze)
            .and_then(i18n::language_from_code)
    })
}

pub(super) fn print_languages() {
    let configured = Config::load()
        .map(|loaded| loaded.config.ui.language)
        .map_err(|err| format!("{err:#}"));
    let requested = i18n::requested_language(configured.clone().unwrap_or_default());
    let in_effect = i18n::resolve(requested.0);

    print!("{}", render(configured, requested, in_effect));
}

/// The listing, from what was read: the configured language (or why the
/// config could not be read), the setting that decides, and what it resolves
/// to.
fn render(
    configured: Result<Language, String>,
    (requested, source): (Language, LanguageSource),
    in_effect: Language,
) -> String {
    let mut out = String::from("Interface languages:\n");
    for language in Language::ALL {
        out.push_str(&format!(
            "  {:<7} {}\n",
            language.code(),
            describe(language)
        ));
    }

    out.push('\n');
    match &configured {
        Ok(language) => out.push_str(&format!(
            "Configured:  {} ([ui] language)\n",
            language.code()
        )),
        // The first line names the file and the position; the rest of a
        // parse error is a source excerpt that would break up the listing.
        Err(err) => out.push_str(&format!(
            "Configured:  auto, the default, because the config could not be read: {}\n",
            err.lines().next().unwrap_or_default()
        )),
    }
    let decided_by = match source {
        LanguageSource::CommandLine => "--language",
        LanguageSource::Environment => "WAYSCRIBER_LANGUAGE",
        LanguageSource::Configured => "[ui] language",
    };
    let through_locale = if requested.is_auto() {
        " through the desktop locale"
    } else {
        ""
    };
    out.push_str(&format!(
        "In effect:   {} {}, from {decided_by}{through_locale}\n",
        in_effect.code(),
        describe(in_effect)
    ));

    out.push_str(
        "\nUse --language CODE for one run, or set [ui] language in config.toml or the configurator.\n",
    );
    out
}

fn describe(language: Language) -> String {
    match language.native_name() {
        None => "follow the desktop locale; English when it has no translation".to_string(),
        Some(native) if native == language.english_name() => native.to_string(),
        Some(native) => format!("{native} ({})", language.english_name()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> Cli {
        match Cli::try_parse_from(std::iter::once("wayscriber").chain(arguments.iter().copied())) {
            Ok(crate::cli::CliOutcome::Run(cli)) => cli,
            other => panic!("{arguments:?} did not parse to a run: {other:?}"),
        }
    }

    #[test]
    fn an_overlay_takes_its_daemons_language_unless_given_its_own() {
        let overlay = parse(&["--active"]);
        assert_eq!(
            given_language_with(&overlay, Some("zh-CN")),
            Some(Language::SimplifiedChinese)
        );
        assert_eq!(given_language_with(&overlay, None), None);
        assert_eq!(given_language_with(&overlay, Some("fr")), None);

        let own = parse(&["--freeze", "--language", "en"]);
        assert_eq!(
            given_language_with(&own, Some("zh-CN")),
            Some(Language::English)
        );

        // Only an overlay reads the daemon's input; a daemon or the About
        // window that inherited it does not.
        for arguments in [&["--daemon"][..], &["--about"]] {
            assert_eq!(given_language_with(&parse(arguments), Some("zh-CN")), None);
        }
    }

    #[test]
    fn the_listing_names_every_language_and_why_one_is_in_effect() {
        let listing = render(
            Ok(Language::Auto),
            (Language::Auto, LanguageSource::Configured),
            Language::SimplifiedChinese,
        );

        assert_eq!(
            listing,
            "Interface languages:\n\
             \x20 auto    follow the desktop locale; English when it has no translation\n\
             \x20 en      English\n\
             \x20 zh-CN   简体中文 (Simplified Chinese)\n\
             \n\
             Configured:  auto ([ui] language)\n\
             In effect:   zh-CN 简体中文 (Simplified Chinese), from [ui] language through the desktop locale\n\
             \n\
             Use --language CODE for one run, or set [ui] language in config.toml or the configurator.\n"
        );
    }

    #[test]
    fn the_listing_says_which_override_won() {
        let from_flag = render(
            Ok(Language::English),
            (Language::SimplifiedChinese, LanguageSource::CommandLine),
            Language::SimplifiedChinese,
        );
        let from_environment = render(
            Err("TOML parse error at line 1, column 4\n  |\n1 | [ui\n".to_string()),
            (Language::English, LanguageSource::Environment),
            Language::English,
        );

        assert!(
            from_flag
                .contains("In effect:   zh-CN 简体中文 (Simplified Chinese), from --language\n")
        );
        assert!(from_environment.contains(
            "Configured:  auto, the default, because the config could not be read: \
             TOML parse error at line 1, column 4\nIn effect:"
        ));
        assert!(from_environment.contains("In effect:   en English, from WAYSCRIBER_LANGUAGE\n"));
    }
}
