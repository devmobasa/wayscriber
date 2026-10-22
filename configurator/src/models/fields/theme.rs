use std::fmt::{Display, Formatter};
use std::{fmt, iter};

use wayscriber::i18n::tr;

use wayscriber::config::{Language, ReducedMotion, UiTheme};
use wayscriber::i18n;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiThemeOption {
    Auto,
    Dark,
    Light,
}

impl UiThemeOption {
    pub fn list() -> Vec<Self> {
        vec![
            UiThemeOption::Auto,
            UiThemeOption::Dark,
            UiThemeOption::Light,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            UiThemeOption::Auto => tr("Auto"),
            UiThemeOption::Dark => tr("Dark"),
            UiThemeOption::Light => tr("Light"),
        }
    }

    pub fn to_theme(self) -> UiTheme {
        match self {
            UiThemeOption::Auto => UiTheme::Auto,
            UiThemeOption::Dark => UiTheme::Dark,
            UiThemeOption::Light => UiTheme::Light,
        }
    }

    pub fn from_theme(theme: UiTheme) -> Self {
        match theme {
            UiTheme::Auto => UiThemeOption::Auto,
            UiTheme::Dark => UiThemeOption::Dark,
            UiTheme::Light => UiThemeOption::Light,
        }
    }
}

impl Display for UiThemeOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReducedMotionOption {
    Auto,
    On,
    Off,
}

impl ReducedMotionOption {
    pub fn list() -> Vec<Self> {
        vec![
            ReducedMotionOption::Auto,
            ReducedMotionOption::On,
            ReducedMotionOption::Off,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ReducedMotionOption::Auto => tr("Auto"),
            ReducedMotionOption::On => tr("On"),
            ReducedMotionOption::Off => tr("Off"),
        }
    }

    pub fn to_reduced_motion(self) -> ReducedMotion {
        match self {
            ReducedMotionOption::Auto => ReducedMotion::Auto,
            ReducedMotionOption::On => ReducedMotion::On,
            ReducedMotionOption::Off => ReducedMotion::Off,
        }
    }

    pub fn from_reduced_motion(value: ReducedMotion) -> Self {
        match value {
            ReducedMotion::Auto => ReducedMotionOption::Auto,
            ReducedMotion::On => ReducedMotionOption::On,
            ReducedMotion::Off => ReducedMotionOption::Off,
        }
    }
}

impl Display for ReducedMotionOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageOption {
    Auto,
    English,
    SimplifiedChinese,
}

impl LanguageOption {
    /// A globe, which marks the Language row in every interface language.
    pub const ROW_ICON: &'static str = "preferences-desktop-locale-symbolic";

    pub fn list() -> Vec<Self> {
        vec![
            LanguageOption::Auto,
            LanguageOption::English,
            LanguageOption::SimplifiedChinese,
        ]
    }

    /// Each language is named in itself, never translated: picking a
    /// language previews it at once, and a user who cannot read the preview
    /// must still recognize their own language to get back. Only the
    /// automatic choice, which is not a language, follows the UI.
    pub fn label(&self) -> &'static str {
        match self.to_language().native_name() {
            Some(name) => name,
            None => tr("Automatic (system locale)"),
        }
    }

    /// The Language row's title: the word for "language" in the interface
    /// language, then in each other supported language (`语言 / Language`),
    /// so a user who cannot read the interface still recognizes the row.
    pub fn row_title() -> String {
        let active = i18n::language();
        iter::once(active)
            .chain(
                Language::ALL
                    .into_iter()
                    .filter(|language| *language != active),
            )
            .filter_map(Language::word_for_language)
            .collect::<Vec<_>>()
            .join(" / ")
    }

    pub fn to_language(self) -> Language {
        match self {
            LanguageOption::Auto => Language::Auto,
            LanguageOption::English => Language::English,
            LanguageOption::SimplifiedChinese => Language::SimplifiedChinese,
        }
    }

    pub fn from_language(value: Language) -> Self {
        match value {
            Language::Auto => LanguageOption::Auto,
            Language::English => LanguageOption::English,
            Language::SimplifiedChinese => LanguageOption::SimplifiedChinese,
        }
    }
}

impl Display for LanguageOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
