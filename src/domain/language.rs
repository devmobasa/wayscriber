//! UI language selection (`[ui] language`).

use serde::{Deserialize, Serialize};

/// Locale-driven UI language.
///
/// `Auto` follows the desktop locale (`WAYSCRIBER_LANGUAGE`, then the POSIX
/// `LANGUAGE`/`LC_ALL`/`LC_MESSAGES`/`LANG` chain) and falls back to English
/// when that locale has no catalog. The explicit variants pin a language
/// regardless of the environment.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    /// Follow the desktop locale; English when it is not translated.
    #[default]
    #[serde(rename = "auto")]
    Auto,
    /// English UI strings.
    #[serde(rename = "en", alias = "english")]
    English,
    /// Simplified Chinese UI strings.
    #[serde(rename = "zh-CN", alias = "zh-cn", alias = "zh-Hans", alias = "zh")]
    SimplifiedChinese,
}

impl Language {
    /// The spelling used in configuration and logs.
    pub fn code(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::English => "en",
            Self::SimplifiedChinese => "zh-CN",
        }
    }

    /// The translation catalog this language reads, or `None` when the source
    /// strings already are that language (English).
    pub fn catalog_code(self) -> Option<&'static str> {
        match self {
            Self::Auto | Self::English => None,
            Self::SimplifiedChinese => Some("zh-CN"),
        }
    }

    /// Whether this setting defers to the desktop locale.
    pub fn is_auto(self) -> bool {
        matches!(self, Self::Auto)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_codes_round_trip_through_serde() {
        for language in [
            Language::Auto,
            Language::English,
            Language::SimplifiedChinese,
        ] {
            let encoded = serde_json::to_string(&language).expect("serialize language");
            assert_eq!(encoded, format!("\"{}\"", language.code()));
            let decoded: Language = serde_json::from_str(&encoded).expect("deserialize language");
            assert_eq!(decoded, language);
        }
    }

    #[test]
    fn language_accepts_common_locale_spellings() {
        for spelling in ["\"zh-CN\"", "\"zh-cn\"", "\"zh-Hans\"", "\"zh\""] {
            let decoded: Language = serde_json::from_str(spelling).expect("deserialize locale");
            assert_eq!(decoded, Language::SimplifiedChinese, "spelling {spelling}");
        }

        let english: Language = serde_json::from_str("\"english\"").expect("deserialize english");
        assert_eq!(english, Language::English);
    }

    #[test]
    fn default_language_follows_the_locale() {
        assert_eq!(Language::default(), Language::Auto);
    }
}
