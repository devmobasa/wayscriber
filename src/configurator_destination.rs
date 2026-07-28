//! The screen a configurator launch should land on.
//!
//! The overlay and the tray both start `wayscriber-configurator`, and both need
//! to say *where* the user should arrive when the control they used no longer
//! edits the file itself. The process broker rejects every environment variable
//! outside its six-name allowlist, so argv is the only channel: one
//! [`OPEN_FLAG`] token plus one destination token that survives
//! [`ConfiguratorDestination::as_arg`] on the launching side and
//! [`ConfiguratorDestination::parse`] on the configurator side.
//!
//! The vocabulary lives in the main crate because the configurator already
//! depends on it as a library and the dependency cannot run the other way. It
//! sits beside `cli.rs` rather than inside `config/` because it names
//! configurator screens, not configuration values: nothing here is ever read
//! from or written to `config.toml`.

use std::ffi::OsString;

/// The flag that carries a destination to the configurator process.
///
/// Shared so the two crates cannot disagree about its spelling.
pub const OPEN_FLAG: &str = "--open";

/// Separates the screen from its query in a destination token.
const QUERY_SEPARATOR: char = '?';

/// The only query key a destination understands.
const SEARCH_KEY: &str = "search=";

/// A keybinding subtab, mirroring the configurator's own Keybindings sections.
///
/// Duplicated rather than shared because the configurator's tab types are
/// private to its UI; the mapping between the two lives in the configurator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeybindingsSection {
    General,
    Drawing,
    Tools,
    Selection,
    History,
    Boards,
    UiModes,
    CaptureView,
    Presets,
}

/// A configurator screen a launcher can name.
///
/// One variant per row of the navigation table: a tab, plus the subtab where
/// the configurator splits a tab into several.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfiguratorScreen {
    UiToolbar,
    UiToolbarVisibility,
    UiStatusBar,
    UiClickHighlight,
    UiInputHud,
    Drawing,
    Presets,
    Boards,
    History,
    Session,
    /// The Keybindings tab, optionally on a named section.
    Keybindings(Option<KeybindingsSection>),
}

impl ConfiguratorScreen {
    /// Every nameable screen, in the order help text lists them.
    pub const ALL: [Self; 20] = [
        Self::UiToolbar,
        Self::UiToolbarVisibility,
        Self::UiStatusBar,
        Self::UiClickHighlight,
        Self::UiInputHud,
        Self::Drawing,
        Self::Presets,
        Self::Boards,
        Self::History,
        Self::Session,
        Self::Keybindings(None),
        Self::Keybindings(Some(KeybindingsSection::General)),
        Self::Keybindings(Some(KeybindingsSection::Drawing)),
        Self::Keybindings(Some(KeybindingsSection::Tools)),
        Self::Keybindings(Some(KeybindingsSection::Selection)),
        Self::Keybindings(Some(KeybindingsSection::History)),
        Self::Keybindings(Some(KeybindingsSection::Boards)),
        Self::Keybindings(Some(KeybindingsSection::UiModes)),
        Self::Keybindings(Some(KeybindingsSection::CaptureView)),
        Self::Keybindings(Some(KeybindingsSection::Presets)),
    ];

    /// The token that names this screen on the command line.
    pub const fn as_arg(self) -> &'static str {
        match self {
            Self::UiToolbar => "ui/toolbar",
            Self::UiToolbarVisibility => "ui/toolbar-visibility",
            Self::UiStatusBar => "ui/status-bar",
            Self::UiClickHighlight => "ui/click-highlight",
            Self::UiInputHud => "ui/input-hud",
            Self::Drawing => "drawing",
            Self::Presets => "presets",
            Self::Boards => "boards",
            Self::History => "history",
            Self::Session => "session",
            Self::Keybindings(None) => "keybindings",
            Self::Keybindings(Some(KeybindingsSection::General)) => "keybindings/general",
            Self::Keybindings(Some(KeybindingsSection::Drawing)) => "keybindings/drawing",
            Self::Keybindings(Some(KeybindingsSection::Tools)) => "keybindings/tools",
            Self::Keybindings(Some(KeybindingsSection::Selection)) => "keybindings/selection",
            Self::Keybindings(Some(KeybindingsSection::History)) => "keybindings/history",
            Self::Keybindings(Some(KeybindingsSection::Boards)) => "keybindings/boards",
            Self::Keybindings(Some(KeybindingsSection::UiModes)) => "keybindings/ui-modes",
            Self::Keybindings(Some(KeybindingsSection::CaptureView)) => "keybindings/capture-view",
            Self::Keybindings(Some(KeybindingsSection::Presets)) => "keybindings/presets",
        }
    }

    /// The screen a token names, if this build has it.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "ui/toolbar" => Some(Self::UiToolbar),
            "ui/toolbar-visibility" => Some(Self::UiToolbarVisibility),
            "ui/status-bar" => Some(Self::UiStatusBar),
            "ui/click-highlight" => Some(Self::UiClickHighlight),
            "ui/input-hud" => Some(Self::UiInputHud),
            "drawing" => Some(Self::Drawing),
            "presets" => Some(Self::Presets),
            "boards" => Some(Self::Boards),
            "history" => Some(Self::History),
            "session" => Some(Self::Session),
            "keybindings" => Some(Self::Keybindings(None)),
            "keybindings/general" => Some(Self::Keybindings(Some(KeybindingsSection::General))),
            "keybindings/drawing" => Some(Self::Keybindings(Some(KeybindingsSection::Drawing))),
            "keybindings/tools" => Some(Self::Keybindings(Some(KeybindingsSection::Tools))),
            "keybindings/selection" => Some(Self::Keybindings(Some(KeybindingsSection::Selection))),
            "keybindings/history" => Some(Self::Keybindings(Some(KeybindingsSection::History))),
            "keybindings/boards" => Some(Self::Keybindings(Some(KeybindingsSection::Boards))),
            "keybindings/ui-modes" => Some(Self::Keybindings(Some(KeybindingsSection::UiModes))),
            "keybindings/capture-view" => {
                Some(Self::Keybindings(Some(KeybindingsSection::CaptureView)))
            }
            "keybindings/presets" => Some(Self::Keybindings(Some(KeybindingsSection::Presets))),
            _ => None,
        }
    }
}

/// A screen to open, and optionally the search term to open it with.
///
/// The search term is how the two table rows that need finer aim than a subtab
/// express it: quick colors inside Drawing, and one action inside Keybindings.
/// The configurator has no scroll-to-widget capability, so search is the whole
/// of the extra precision available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguratorDestination {
    screen: ConfiguratorScreen,
    search: Option<String>,
}

impl ConfiguratorDestination {
    /// A destination that only names a screen.
    pub const fn new(screen: ConfiguratorScreen) -> Self {
        Self {
            screen,
            search: None,
        }
    }

    /// A destination that also fills the configurator's search box.
    ///
    /// A blank term is dropped: it would leave the search box holding
    /// whitespace that matches everything, which is the same screen the plain
    /// destination already opens.
    pub fn with_search(screen: ConfiguratorScreen, term: impl AsRef<str>) -> Self {
        let term = term.as_ref().trim();
        Self {
            screen,
            search: (!term.is_empty()).then(|| term.to_string()),
        }
    }

    pub const fn screen(&self) -> ConfiguratorScreen {
        self.screen
    }

    pub fn search(&self) -> Option<&str> {
        self.search.as_deref()
    }

    /// The single argv token that carries this destination.
    ///
    /// The term is written verbatim after `?search=` and runs to the end of the
    /// token. Nothing needs escaping: the token goes from the broker straight
    /// into `exec`, never through a shell or a file, so a term may contain
    /// spaces, `?`, and `=` and still come back unchanged.
    pub fn as_arg(&self) -> String {
        let screen = self.screen.as_arg();
        match &self.search {
            None => screen.to_string(),
            Some(term) => format!("{screen}{QUERY_SEPARATOR}{SEARCH_KEY}{term}"),
        }
    }

    /// The destination a token names, if this build understands all of it.
    ///
    /// A screen this build does not have, or a query it does not understand,
    /// yields `None` rather than a partial landing: the configurator says so
    /// and opens its usual first screen instead.
    pub fn parse(text: &str) -> Option<Self> {
        let (screen_token, term) = match text.split_once(QUERY_SEPARATOR) {
            None => (text, None),
            Some((screen_token, query)) => (screen_token, Some(query.strip_prefix(SEARCH_KEY)?)),
        };
        let screen = ConfiguratorScreen::parse(screen_token)?;
        Some(match term {
            Some(term) => Self::with_search(screen, term),
            None => Self::new(screen),
        })
    }
}

/// The arguments that launch the configurator at a destination.
///
/// Split out from the spawn calls so the overlay and the tray build the same
/// argv, and so that argv is testable without a process broker.
pub(crate) fn configurator_launch_arguments(
    destination: Option<&ConfiguratorDestination>,
) -> Vec<OsString> {
    match destination {
        None => Vec::new(),
        Some(destination) => vec![
            OsString::from(OPEN_FLAG),
            OsString::from(destination.as_arg()),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The terms the table's two searching rows would realistically carry,
    /// plus the punctuation a search box accepts.
    const SAMPLE_TERMS: [&str; 4] = ["Quick Colors", "Clear Canvas", "#ff0000", "board? preset="];

    #[test]
    fn every_screen_round_trips_without_a_search_term() {
        for screen in ConfiguratorScreen::ALL {
            let destination = ConfiguratorDestination::new(screen);
            assert_eq!(
                ConfiguratorDestination::parse(&destination.as_arg()),
                Some(destination.clone()),
                "screen {screen:?} did not round-trip through {}",
                destination.as_arg()
            );
        }
    }

    #[test]
    fn every_screen_round_trips_with_a_search_term() {
        for screen in ConfiguratorScreen::ALL {
            for term in SAMPLE_TERMS {
                let destination = ConfiguratorDestination::with_search(screen, term);
                assert_eq!(destination.search(), Some(term));
                assert_eq!(
                    ConfiguratorDestination::parse(&destination.as_arg()),
                    Some(destination.clone()),
                    "screen {screen:?} with term {term:?} did not round-trip through {}",
                    destination.as_arg()
                );
            }
        }
    }

    #[test]
    fn screen_tokens_are_unique() {
        let mut tokens = ConfiguratorScreen::ALL
            .iter()
            .map(|screen| screen.as_arg())
            .collect::<Vec<_>>();
        let total = tokens.len();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(
            tokens.len(),
            total,
            "two screens share a command-line token"
        );
    }

    #[test]
    fn navigation_table_rows_have_expected_tokens() {
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::UiToolbar).as_arg(),
            "ui/toolbar"
        );
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::UiToolbarVisibility).as_arg(),
            "ui/toolbar-visibility"
        );
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::UiStatusBar).as_arg(),
            "ui/status-bar"
        );
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::UiClickHighlight).as_arg(),
            "ui/click-highlight"
        );
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::UiInputHud).as_arg(),
            "ui/input-hud"
        );
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::History).as_arg(),
            "history"
        );
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::Boards).as_arg(),
            "boards"
        );
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::Presets).as_arg(),
            "presets"
        );
        assert_eq!(
            ConfiguratorDestination::with_search(ConfiguratorScreen::Drawing, "Quick Colors")
                .as_arg(),
            "drawing?search=Quick Colors"
        );
        assert_eq!(
            ConfiguratorDestination::with_search(
                ConfiguratorScreen::Keybindings(Some(KeybindingsSection::General)),
                "Clear Canvas"
            )
            .as_arg(),
            "keybindings/general?search=Clear Canvas"
        );
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::Session).as_arg(),
            "session"
        );
    }

    #[test]
    fn parse_rejects_unknown_screens_and_queries() {
        assert_eq!(ConfiguratorDestination::parse(""), None);
        assert_eq!(ConfiguratorDestination::parse("ui"), None);
        assert_eq!(ConfiguratorDestination::parse("ui/toolbars"), None);
        assert_eq!(ConfiguratorDestination::parse("Ui/Toolbar"), None);
        assert_eq!(
            ConfiguratorDestination::parse("keybindings/general/x"),
            None
        );
        assert_eq!(ConfiguratorDestination::parse("drawing?focus=colors"), None);
        assert_eq!(ConfiguratorDestination::parse("drawing?"), None);
    }

    #[test]
    fn blank_search_terms_collapse_to_a_plain_screen() {
        let destination = ConfiguratorDestination::with_search(ConfiguratorScreen::Drawing, "   ");
        assert_eq!(destination.search(), None);
        assert_eq!(destination.as_arg(), "drawing");
        assert_eq!(
            ConfiguratorDestination::parse("drawing?search=   "),
            Some(destination)
        );
    }

    #[test]
    fn search_terms_are_trimmed_but_keep_inner_spacing() {
        let destination =
            ConfiguratorDestination::with_search(ConfiguratorScreen::Drawing, "  Quick  Colors  ");
        assert_eq!(destination.search(), Some("Quick  Colors"));
        assert_eq!(destination.as_arg(), "drawing?search=Quick  Colors");
    }

    #[test]
    fn launch_arguments_are_empty_without_a_destination() {
        assert!(configurator_launch_arguments(None).is_empty());
    }

    #[test]
    fn launch_arguments_carry_the_flag_and_one_token() {
        let destination =
            ConfiguratorDestination::with_search(ConfiguratorScreen::Drawing, "Quick Colors");
        let arguments = configurator_launch_arguments(Some(&destination));

        assert_eq!(
            arguments,
            vec![
                OsString::from("--open"),
                OsString::from("drawing?search=Quick Colors"),
            ]
        );
    }
}
