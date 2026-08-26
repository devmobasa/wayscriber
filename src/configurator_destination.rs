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

use crate::config::KeybindingsConfig;
use crate::domain::Action;
use std::ffi::OsString;

/// The flag that carries a destination to the configurator process.
///
/// Shared so the two crates cannot disagree about its spelling.
pub const OPEN_FLAG: &str = "--open";

/// Separates the screen from its query in a destination token.
const QUERY_SEPARATOR: char = '?';

/// The only query key a destination understands.
const SEARCH_KEY: &str = "search=";

/// The term that aims the Drawing screen at its quick-color palette.
///
/// Named once here so the overlay's palette route and the toast that offers it
/// cannot drift apart; the configurator lists the same words among the Drawing
/// tab's color search terms.
const QUICK_COLORS_SEARCH: &str = "Quick Colors";

/// The visible General UI label for automatic onboarding and discovery hints.
///
/// Keeping the launcher term identical to the control label makes the
/// Configurator reveal the owning section without a separate scroll protocol.
const ONBOARDING_HINTS_SEARCH: &str = "Show automatic guidance and tips";

/// The Drawing screen, focused on the quick-color palette.
pub fn quick_colors_destination() -> ConfiguratorDestination {
    ConfiguratorDestination::with_search(ConfiguratorScreen::Drawing, QUICK_COLORS_SEARCH)
}

/// The General UI section, filtered to the automatic-guidance preference.
pub fn onboarding_hints_destination() -> ConfiguratorDestination {
    ConfiguratorDestination::with_search(ConfiguratorScreen::UiToolbar, ONBOARDING_HINTS_SEARCH)
}

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

/// The Keybindings section that holds one action's shortcut, or `None` for an
/// action with no `[keybindings]` field at all.
///
/// The overlay's own shortcut edits are durable — the palette's Edit/Unbind/
/// Reset write that one action's `[keybindings]` entry — so this is not a
/// substitute for them. It names the subtab a toast's Edit chip should land on:
/// after a refusal, where the file gave the chord to another action and the run
/// kept its old shortcut, and after a save that landed, where the user may want
/// to see the entry the overlay just wrote. The grouping is the
/// configurator's, not the config file's — it splits
/// `[keybindings].core` across General, Drawing, and History and merges the
/// capture and zoom groups — so the correspondence cannot be derived from the
/// storage layout and is spelled out here instead. `keybindings_tab_matches_the_action_section`
/// (configurator crate, which can see both sides) is what keeps the two
/// agreeing.
pub fn keybindings_section_for_action(action: Action) -> Option<KeybindingsSection> {
    let section = match action {
        Action::Exit | Action::OpenConfigurator | Action::OpenAbout => KeybindingsSection::General,
        Action::EnterTextMode
        | Action::EnterStickyNoteMode
        | Action::ClearCanvas
        | Action::IncreaseThickness
        | Action::DecreaseThickness
        | Action::IncreaseMarkerOpacity
        | Action::DecreaseMarkerOpacity
        | Action::IncreaseFontSize
        | Action::DecreaseFontSize
        | Action::ToggleFill
        | Action::SetColorRed
        | Action::SetColorGreen
        | Action::SetColorBlue
        | Action::SetColorYellow
        | Action::SetColorOrange
        | Action::SetColorPink
        | Action::SetColorWhite
        | Action::SetColorBlack
        | Action::PickScreenColor => KeybindingsSection::Drawing,
        Action::SelectSelectionTool
        | Action::SelectPenTool
        | Action::SelectEraserTool
        | Action::ToggleEraserMode
        | Action::IncreasePenSmoothing
        | Action::DecreasePenSmoothing
        | Action::CycleFontFamily
        | Action::OpenFontPicker
        | Action::SelectMarkerTool
        | Action::SelectStepMarkerTool
        | Action::SelectLineTool
        | Action::SelectRectTool
        | Action::SelectEllipseTool
        | Action::SelectArrowTool
        | Action::SelectTriangleTool
        | Action::SelectParallelogramTool
        | Action::SelectRhombusTool
        | Action::SelectRegularPolygonTool
        | Action::SelectFreeformPolygonTool
        | Action::SelectBlurTool
        | Action::SelectSpotlightTool
        | Action::CycleBlurStyle
        | Action::CycleArrowStyle
        | Action::SelectHighlightTool
        | Action::ToggleHighlightTool
        | Action::ResetArrowLabelCounter
        | Action::ResetStepMarkerCounter => KeybindingsSection::Tools,
        Action::DuplicateSelection
        | Action::CopySelection
        | Action::PasteSelection
        | Action::SelectAll
        | Action::MoveSelectionToFront
        | Action::MoveSelectionToBack
        | Action::MoveSelectionToStart
        | Action::MoveSelectionToEnd
        | Action::MoveSelectionToTop
        | Action::MoveSelectionToBottom
        | Action::NudgeSelectionUp
        | Action::NudgeSelectionDown
        | Action::NudgeSelectionLeft
        | Action::NudgeSelectionRight
        | Action::NudgeSelectionUpLarge
        | Action::NudgeSelectionDownLarge
        | Action::DeleteSelection => KeybindingsSection::Selection,
        Action::Undo
        | Action::Redo
        | Action::UndoAll
        | Action::RedoAll
        | Action::UndoAllDelayed
        | Action::RedoAllDelayed => KeybindingsSection::History,
        Action::ToggleWhiteboard
        | Action::ToggleBlackboard
        | Action::ReturnToTransparent
        | Action::PagePrev
        | Action::PageNext
        | Action::PageNew
        | Action::PageDuplicate
        | Action::PageDelete
        | Action::Board1
        | Action::Board2
        | Action::Board3
        | Action::Board4
        | Action::Board5
        | Action::Board6
        | Action::Board7
        | Action::Board8
        | Action::Board9
        | Action::BoardNext
        | Action::BoardPrev
        | Action::BoardNew
        | Action::BoardDuplicate
        | Action::BoardDelete
        | Action::BoardPicker
        | Action::FocusNextOutput
        | Action::FocusPrevOutput => KeybindingsSection::Boards,
        Action::ToggleHelp
        | Action::ToggleQuickHelp
        | Action::ToggleStatusBar
        | Action::ToggleFloatingBadge
        | Action::ToggleZoomChip
        | Action::ToggleFocusMode
        | Action::ToggleClickHighlight
        | Action::ToggleInputHud
        | Action::ToggleToolbar
        | Action::ToggleLightMode
        | Action::ToggleLightModeDrawing
        | Action::ToggleRadialMenu
        | Action::CycleToolbarDisplay
        | Action::TogglePresenterMode
        | Action::RenderProfileNext
        | Action::RenderProfilePrevious
        | Action::RenderProfileOff
        | Action::ToggleSelectionProperties
        | Action::OpenContextMenu
        | Action::ToggleCommandPalette => KeybindingsSection::UiModes,
        Action::CaptureFullScreen
        | Action::CaptureActiveWindow
        | Action::CaptureSelection
        | Action::CaptureClipboardFull
        | Action::CaptureFileFull
        | Action::CaptureClipboardSelection
        | Action::CaptureFileSelection
        | Action::CaptureClipboardRegion
        | Action::CaptureFileRegion
        | Action::CaptureRegionInteractive
        | Action::MeasureMode
        | Action::ExportCanvasFile
        | Action::ExportCanvasClipboard
        | Action::ExportCanvasClipboardAndFile
        | Action::ExportBoardPdfFile
        | Action::ExportAllBoardsPdfFile
        | Action::OpenCaptureFolder
        | Action::CopyTextFromScreen
        | Action::ToggleFrozenMode
        | Action::ZoomIn
        | Action::ZoomOut
        | Action::ResetZoom
        | Action::ToggleZoomLock
        | Action::RefreshZoomCapture => KeybindingsSection::CaptureView,
        Action::ApplyPreset1
        | Action::ApplyPreset2
        | Action::ApplyPreset3
        | Action::ApplyPreset4
        | Action::ApplyPreset5
        | Action::SavePreset1
        | Action::SavePreset2
        | Action::SavePreset3
        | Action::SavePreset4
        | Action::SavePreset5
        | Action::ClearPreset1
        | Action::ClearPreset2
        | Action::ClearPreset3
        | Action::ClearPreset4
        | Action::ClearPreset5 => KeybindingsSection::Presets,
        // Runtime-only actions and the configurator routes themselves have no
        // `[keybindings]` field, so there is no row to land on.
        Action::BoardRestoreDeleted
        | Action::BoardSwitchRecent
        | Action::PageRestoreDeleted
        | Action::ClearSavedToolState
        | Action::ReplayTour
        | Action::SavePendingToFile
        | Action::OpenConfiguratorKeybindings
        | Action::OpenConfiguratorPresets
        | Action::OpenConfiguratorBoards
        | Action::OpenConfiguratorQuickColors
        | Action::OpenConfiguratorOnboardingHints => return None,
    };
    Some(section)
}

/// The configurator screen that edits one action's shortcut.
///
/// The search term is the action's own `[keybindings]` key with its underscores
/// opened out: the configurator searches that key verbatim alongside its row
/// label, so this is the one spelling guaranteed to select exactly the row the
/// user asked about while still reading as words in the search box.
pub fn keybindings_destination_for_action(action: Action) -> Option<ConfiguratorDestination> {
    let section = keybindings_section_for_action(action)?;
    let key = KeybindingsConfig::config_key_for_action(action)?;
    Some(ConfiguratorDestination::with_search(
        ConfiguratorScreen::Keybindings(Some(section)),
        key.replace('_', " "),
    ))
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
    UiHelpOverlay,
    UiPresenterMode,
    Drawing,
    Presets,
    Boards,
    History,
    Session,
    Capture,
    Performance,
    Daemon,
    Arrow,
    RenderProfiles,
    #[cfg(feature = "tablet-input")]
    Tablet,
    /// The Keybindings tab, optionally on a named section.
    Keybindings(Option<KeybindingsSection>),
}

impl ConfiguratorScreen {
    /// Every nameable screen, in the order help text lists them.
    pub const ALL: &'static [Self] = &[
        Self::UiToolbar,
        Self::UiToolbarVisibility,
        Self::UiStatusBar,
        Self::UiClickHighlight,
        Self::UiInputHud,
        Self::UiHelpOverlay,
        Self::UiPresenterMode,
        Self::Drawing,
        Self::Presets,
        Self::Boards,
        Self::History,
        Self::Session,
        Self::Capture,
        Self::Performance,
        Self::Daemon,
        Self::Arrow,
        Self::RenderProfiles,
        #[cfg(feature = "tablet-input")]
        Self::Tablet,
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
            Self::UiHelpOverlay => "ui/help-overlay",
            Self::UiPresenterMode => "ui/presenter-mode",
            Self::Drawing => "drawing",
            Self::Presets => "presets",
            Self::Boards => "boards",
            Self::History => "history",
            Self::Session => "session",
            Self::Capture => "capture",
            Self::Performance => "performance",
            Self::Daemon => "daemon",
            Self::Arrow => "arrow",
            Self::RenderProfiles => "render-profiles",
            #[cfg(feature = "tablet-input")]
            Self::Tablet => "tablet",
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
            "ui/help-overlay" => Some(Self::UiHelpOverlay),
            "ui/presenter-mode" => Some(Self::UiPresenterMode),
            "drawing" => Some(Self::Drawing),
            "presets" => Some(Self::Presets),
            "boards" => Some(Self::Boards),
            "history" => Some(Self::History),
            "session" => Some(Self::Session),
            "capture" => Some(Self::Capture),
            "performance" => Some(Self::Performance),
            "daemon" => Some(Self::Daemon),
            "arrow" => Some(Self::Arrow),
            "render-profiles" => Some(Self::RenderProfiles),
            #[cfg(feature = "tablet-input")]
            "tablet" => Some(Self::Tablet),
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
        for &screen in ConfiguratorScreen::ALL {
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
        for &screen in ConfiguratorScreen::ALL {
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
            ConfiguratorDestination::new(ConfiguratorScreen::UiHelpOverlay).as_arg(),
            "ui/help-overlay"
        );
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::UiPresenterMode).as_arg(),
            "ui/presenter-mode"
        );
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::Capture).as_arg(),
            "capture"
        );
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::Performance).as_arg(),
            "performance"
        );
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::Daemon).as_arg(),
            "daemon"
        );
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::Arrow).as_arg(),
            "arrow"
        );
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::RenderProfiles).as_arg(),
            "render-profiles"
        );
        #[cfg(feature = "tablet-input")]
        assert_eq!(
            ConfiguratorDestination::new(ConfiguratorScreen::Tablet).as_arg(),
            "tablet"
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
        #[cfg(not(feature = "tablet-input"))]
        assert_eq!(ConfiguratorDestination::parse("tablet"), None);
    }

    #[test]
    #[cfg(feature = "tablet-input")]
    fn tablet_is_a_destination_when_the_feature_is_on() {
        assert_eq!(
            ConfiguratorScreen::parse("tablet"),
            Some(ConfiguratorScreen::Tablet)
        );
        assert!(ConfiguratorScreen::ALL.contains(&ConfiguratorScreen::Tablet));
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

    /// Every shortcut the overlay can show is a shortcut it can hand over, so
    /// the mapping has to cover the whole stored keymap.
    #[test]
    fn every_configurable_action_names_a_keybindings_section() {
        for action in KeybindingsConfig::configurable_actions() {
            assert!(
                keybindings_section_for_action(*action).is_some(),
                "{action:?} stores a shortcut but names no Keybindings section"
            );
            assert!(
                keybindings_destination_for_action(*action).is_some(),
                "{action:?} stores a shortcut but has no destination"
            );
        }
    }

    /// The other half: an action with no `[keybindings]` field has no row to
    /// land on, and the affordance must say so rather than open a screen that
    /// cannot show it.
    #[test]
    fn actions_without_a_stored_shortcut_have_no_keybindings_destination() {
        for action in [
            Action::BoardRestoreDeleted,
            Action::BoardSwitchRecent,
            Action::PageRestoreDeleted,
            Action::ClearSavedToolState,
            Action::ReplayTour,
            Action::SavePendingToFile,
            Action::OpenConfiguratorKeybindings,
            Action::OpenConfiguratorPresets,
            Action::OpenConfiguratorBoards,
            Action::OpenConfiguratorQuickColors,
            Action::OpenConfiguratorOnboardingHints,
        ] {
            assert_eq!(
                keybindings_section_for_action(action),
                None,
                "{action:?} has no stored shortcut, so it names no section"
            );
            assert!(keybindings_destination_for_action(action).is_none());
        }
    }

    /// One row per section, spelled out: the token carries the subtab and the
    /// action's own config key as the search term.
    #[test]
    fn shortcut_destinations_name_the_section_and_search_the_config_key() {
        for (action, expected) in [
            (Action::Exit, "keybindings/general?search=exit"),
            (
                Action::ClearCanvas,
                "keybindings/drawing?search=clear canvas",
            ),
            (
                Action::SelectPenTool,
                "keybindings/tools?search=select pen tool",
            ),
            (
                Action::DeleteSelection,
                "keybindings/selection?search=delete selection",
            ),
            (Action::UndoAll, "keybindings/history?search=undo all"),
            (Action::BoardNext, "keybindings/boards?search=board next"),
            (
                Action::ToggleToolbar,
                "keybindings/ui-modes?search=toggle toolbar",
            ),
            (Action::ZoomIn, "keybindings/capture-view?search=zoom in"),
            (
                Action::SavePreset1,
                "keybindings/presets?search=save preset 1",
            ),
        ] {
            assert_eq!(
                keybindings_destination_for_action(action)
                    .map(|destination| destination.as_arg())
                    .as_deref(),
                Some(expected),
                "{action:?} did not land on its own row"
            );
        }
    }

    #[test]
    fn the_quick_colors_route_searches_the_drawing_palette() {
        assert_eq!(
            quick_colors_destination().as_arg(),
            "drawing?search=Quick Colors"
        );
    }

    #[test]
    fn the_onboarding_hint_route_searches_the_general_ui_setting() {
        assert_eq!(
            onboarding_hints_destination().as_arg(),
            "ui/toolbar?search=Show automatic guidance and tips"
        );
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
