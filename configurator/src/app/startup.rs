use wayscriber::configurator_destination::{ConfiguratorScreen, KeybindingsSection};

use crate::models::{KeybindingsTabId, SearchQuery, TabId, UiTabId};

use super::effects::Effect;
use super::state::ConfiguratorApp;

impl ConfiguratorApp {
    /// Opens whatever the launching process asked for, once.
    ///
    /// Consumed by the first config load and empty from then on: a later
    /// Reload is the user asking for the file again, not a fresh launch, and
    /// snapping the tabs back would undo wherever they navigated in between.
    pub(crate) fn apply_startup_request(&mut self) -> Vec<Effect> {
        let request = std::mem::take(&mut self.startup_request);

        if let Some(problem) = request.problem() {
            self.status = self.status.clone().with_note(&problem.message());
        }

        let Some(destination) = request.destination() else {
            // Nothing to route to, so this is an ordinary launch: the usual
            // first screen, including its search-box focus.
            return self.handle_startup_search_focus_config_fallback();
        };

        let term = destination.search().map(str::to_string);
        self.show_destination_screen(destination.screen());

        // The startup focus offer is answered either way: the user asked for a
        // screen, so the fallback must not fire again later.
        self.startup_search_focus_pending = false;

        match term {
            Some(term) => {
                self.search_query = SearchQuery::new(term);
                // Aligned after the tab is set, never before: alignment only
                // moves off a tab with no matches, so a destination whose own
                // screen matches the term keeps it, and one whose screen has
                // nothing to show lands where the matches actually are.
                self.align_active_tabs_for_search();
                // The box now holds text the user will want to edit or clear.
                self.handle_search_focus_requested()
            }
            None => {
                // A plain screen request is not a search: focusing the search
                // box would send the first keystroke to the filter instead of
                // to the screen the user asked for.
                Vec::new()
            }
        }
    }

    fn show_destination_screen(&mut self, screen: ConfiguratorScreen) {
        match screen {
            ConfiguratorScreen::UiToolbar => self.show_ui_tab(UiTabId::Toolbar),
            ConfiguratorScreen::UiToolbarVisibility => {
                self.show_ui_tab(UiTabId::ToolbarVisibility);
            }
            ConfiguratorScreen::UiStatusBar => self.show_ui_tab(UiTabId::StatusBar),
            ConfiguratorScreen::UiClickHighlight => self.show_ui_tab(UiTabId::ClickHighlight),
            ConfiguratorScreen::UiInputHud => self.show_ui_tab(UiTabId::InputHud),
            ConfiguratorScreen::UiHelpOverlay => self.show_ui_tab(UiTabId::HelpOverlay),
            ConfiguratorScreen::UiPresenterMode => self.show_ui_tab(UiTabId::PresenterMode),
            ConfiguratorScreen::Drawing => self.active_tab = TabId::Drawing,
            ConfiguratorScreen::Presets => self.active_tab = TabId::Presets,
            ConfiguratorScreen::Boards => self.active_tab = TabId::Boards,
            ConfiguratorScreen::History => self.active_tab = TabId::History,
            ConfiguratorScreen::Session => self.active_tab = TabId::Session,
            ConfiguratorScreen::Capture => self.active_tab = TabId::Capture,
            ConfiguratorScreen::Performance => self.active_tab = TabId::Performance,
            ConfiguratorScreen::Daemon => self.active_tab = TabId::Daemon,
            ConfiguratorScreen::Arrow => self.active_tab = TabId::Arrow,
            ConfiguratorScreen::RenderProfiles => self.active_tab = TabId::RenderProfiles,
            #[cfg(feature = "tablet-input")]
            ConfiguratorScreen::Tablet => self.active_tab = TabId::Tablet,
            ConfiguratorScreen::Keybindings(section) => {
                self.active_tab = TabId::Keybindings;
                // No section means the tab, on whichever subtab it already
                // shows; the launcher only knew which tab it wanted.
                if let Some(section) = section {
                    self.active_keybindings_tab = keybindings_tab(section);
                }
            }
        }
    }

    fn show_ui_tab(&mut self, tab: UiTabId) {
        self.active_tab = TabId::Ui;
        self.active_ui_tab = tab;
    }
}

fn keybindings_tab(section: KeybindingsSection) -> KeybindingsTabId {
    match section {
        KeybindingsSection::General => KeybindingsTabId::General,
        KeybindingsSection::Drawing => KeybindingsTabId::Drawing,
        KeybindingsSection::Tools => KeybindingsTabId::Tools,
        KeybindingsSection::Selection => KeybindingsTabId::Selection,
        KeybindingsSection::History => KeybindingsTabId::History,
        KeybindingsSection::Boards => KeybindingsTabId::Boards,
        KeybindingsSection::UiModes => KeybindingsTabId::UiModes,
        KeybindingsSection::CaptureView => KeybindingsTabId::CaptureView,
        KeybindingsSection::Presets => KeybindingsTabId::Presets,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use wayscriber::config::ConfigDocument;
    use wayscriber::configurator_destination::ConfiguratorDestination;

    use super::*;
    use crate::app::state::StatusMessage;
    use crate::messages::CommandMessage;
    use crate::models::StartupRequest;
    use crate::test_temp::{TempDir, tempdir};

    fn status_text(status: &StatusMessage) -> String {
        status.text().unwrap_or_default().to_string()
    }

    fn args(values: &[&str]) -> Vec<std::ffi::OsString> {
        std::iter::once("wayscriber-configurator")
            .chain(values.iter().copied())
            .map(std::ffi::OsString::from)
            .collect()
    }

    /// A config file of its own, so the load path under test is the real one.
    fn config_file() -> (TempDir, std::path::PathBuf) {
        let dir = tempdir().expect("temporary test directory");
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "").expect("write test config");
        (dir, path)
    }

    /// The load exactly as the app receives it: through the message the
    /// `load_config_from_disk` task delivers.
    fn load_config_file(app: &mut ConfiguratorApp, path: &Path) {
        let document = ConfigDocument::load_from_path(path).expect("load test config document");
        let _ = app.update_command(CommandMessage::ConfigLoaded(Ok((Box::new(document), None))));
    }

    /// The app as the launcher would start it, after its first config load.
    fn app_launched_with(values: &[&str]) -> (ConfiguratorApp, TempDir, std::path::PathBuf) {
        let (dir, path) = config_file();
        let (mut app, _effects) =
            ConfiguratorApp::new_app_with_startup(StartupRequest::from_args(args(values)));
        load_config_file(&mut app, &path);
        (app, dir, path)
    }

    #[test]
    fn no_destination_keeps_the_usual_first_screen_and_search_focus() {
        let (app, _dir, _path) = app_launched_with(&[]);

        assert_eq!(app.active_tab, TabId::Daemon);
        assert_eq!(app.active_ui_tab, UiTabId::Toolbar);
        assert_eq!(app.search_focus_serial, 1);
        assert!(!app.startup_search_focus_pending);
        assert!(app.search_query.raw().is_empty());
    }

    #[test]
    fn ui_destinations_open_their_tab_and_subtab() {
        for (argument, ui_tab) in [
            ("ui/toolbar", UiTabId::Toolbar),
            ("ui/toolbar-visibility", UiTabId::ToolbarVisibility),
            ("ui/status-bar", UiTabId::StatusBar),
            ("ui/click-highlight", UiTabId::ClickHighlight),
            ("ui/input-hud", UiTabId::InputHud),
            ("ui/help-overlay", UiTabId::HelpOverlay),
            ("ui/presenter-mode", UiTabId::PresenterMode),
        ] {
            let (app, _dir, _path) = app_launched_with(&["--open", argument]);

            assert_eq!(app.active_tab, TabId::Ui, "{argument}");
            assert_eq!(app.active_ui_tab, ui_tab, "{argument}");
        }
    }

    #[test]
    fn plain_tab_destinations_open_their_tab() {
        for (argument, tab) in [
            ("history", TabId::History),
            ("boards", TabId::Boards),
            ("presets", TabId::Presets),
            ("drawing", TabId::Drawing),
            ("session", TabId::Session),
            ("capture", TabId::Capture),
            ("performance", TabId::Performance),
            ("daemon", TabId::Daemon),
            ("arrow", TabId::Arrow),
            ("render-profiles", TabId::RenderProfiles),
            #[cfg(feature = "tablet-input")]
            ("tablet", TabId::Tablet),
            ("keybindings", TabId::Keybindings),
        ] {
            let (app, _dir, _path) = app_launched_with(&["--open", argument]);

            assert_eq!(app.active_tab, tab, "{argument}");
        }
    }

    #[test]
    fn every_keybindings_section_opens_its_subtab() {
        for (argument, keybindings_tab) in [
            ("keybindings/general", KeybindingsTabId::General),
            ("keybindings/drawing", KeybindingsTabId::Drawing),
            ("keybindings/tools", KeybindingsTabId::Tools),
            ("keybindings/selection", KeybindingsTabId::Selection),
            ("keybindings/history", KeybindingsTabId::History),
            ("keybindings/boards", KeybindingsTabId::Boards),
            ("keybindings/ui-modes", KeybindingsTabId::UiModes),
            ("keybindings/capture-view", KeybindingsTabId::CaptureView),
            ("keybindings/presets", KeybindingsTabId::Presets),
        ] {
            let (app, _dir, _path) = app_launched_with(&["--open", argument]);

            assert_eq!(app.active_tab, TabId::Keybindings, "{argument}");
            assert_eq!(app.active_keybindings_tab, keybindings_tab, "{argument}");
        }
    }

    /// The quick-colors row of the navigation table: the term matches several
    /// tabs, and the destination still decides which one opens.
    #[test]
    fn the_quick_colors_destination_lands_on_drawing_with_the_term_searched() {
        let (app, _dir, _path) = app_launched_with(&["--open=drawing?search=Quick Colors"]);

        assert_eq!(app.active_tab, TabId::Drawing);
        assert_eq!(app.search_query.raw(), "Quick Colors");
        assert!(app.search_summary().tab(TabId::Drawing).is_some());
        // A term means the search box has content to edit or clear.
        assert_eq!(app.search_focus_serial, 1);
        assert!(!app.startup_search_focus_pending);
    }

    #[test]
    fn the_onboarding_hint_destination_lands_on_the_general_ui_setting() {
        let launched =
            wayscriber::configurator_destination::onboarding_hints_destination().as_arg();
        let (app, _dir, _path) = app_launched_with(&["--open", &launched]);

        assert_eq!(app.active_tab, TabId::Ui);
        assert_eq!(app.search_query.raw(), "Show automatic guidance and tips");
        let summary = app.search_summary();
        let ui = summary
            .tab(TabId::Ui)
            .expect("General UI destination match");
        assert!(ui.area_matches(crate::app::search::SearchArea::UiGeneral));
    }

    /// The shortcut-action row: subtab plus action search.
    #[test]
    fn a_keybinding_action_destination_lands_on_its_subtab_with_the_term_searched() {
        let (app, _dir, _path) =
            app_launched_with(&["--open=keybindings/drawing?search=Clear Canvas"]);

        assert_eq!(app.active_tab, TabId::Keybindings);
        assert_eq!(app.active_keybindings_tab, KeybindingsTabId::Drawing);
        assert_eq!(app.search_query.raw(), "Clear Canvas");
        assert!(app.search_summary().tab(TabId::Keybindings).is_some());
    }

    /// The same correction as the tab-level one, a level down: a subtab that
    /// cannot show the searched action gives way to the one that can.
    #[test]
    fn a_term_that_misses_the_named_keybindings_subtab_follows_the_matches() {
        let (app, _dir, _path) =
            app_launched_with(&["--open=keybindings/general?search=Clear Canvas"]);

        assert_eq!(app.active_tab, TabId::Keybindings);
        assert_eq!(app.active_keybindings_tab, KeybindingsTabId::Drawing);
    }

    /// A term with no match on the requested screen is the one case where
    /// alignment overrules the destination: an empty tab helps nobody.
    #[test]
    fn a_term_that_misses_the_destination_tab_follows_the_matches() {
        let (app, _dir, _path) = app_launched_with(&["--open=boards?search=input hud"]);

        assert_ne!(app.active_tab, TabId::Boards);
        assert!(app.search_summary().tab(app.active_tab).is_some());
    }

    #[test]
    fn a_plain_screen_request_does_not_take_the_search_focus() {
        let (app, _dir, _path) = app_launched_with(&["--open", "ui/status-bar"]);

        assert_eq!(app.search_focus_serial, 0);
        assert!(!app.startup_search_focus_pending);
        assert!(app.search_query.raw().is_empty());
    }

    #[test]
    fn an_unknown_destination_falls_back_with_a_visible_message() {
        let (app, _dir, _path) = app_launched_with(&["--open", "ui/nowhere"]);

        assert_eq!(app.active_tab, TabId::Daemon);
        assert!(matches!(app.status, StatusMessage::Warning(_)));
        let text = status_text(&app.status);
        assert!(text.contains("Unknown destination: ui/nowhere"), "{text}");
        // The load result is still reported alongside the note.
        assert!(text.contains("Configuration loaded from disk."), "{text}");
        // Falling back means the ordinary first screen, focus included.
        assert_eq!(app.search_focus_serial, 1);
        assert!(!app.startup_search_focus_pending);
    }

    #[test]
    #[cfg(not(feature = "tablet-input"))]
    fn tablet_is_an_unknown_destination_without_tablet_input() {
        let (app, _dir, _path) = app_launched_with(&["--open", "tablet"]);

        assert_eq!(app.active_tab, TabId::Daemon);
        let text = status_text(&app.status);
        assert!(text.contains("Unknown destination: tablet"), "{text}");
    }

    #[test]
    fn an_unrecognized_argument_still_opens_its_destination() {
        let (app, _dir, _path) = app_launched_with(&["--frobnicate", "--open", "session"]);

        assert_eq!(app.active_tab, TabId::Session);
        let text = status_text(&app.status);
        assert!(text.contains("--frobnicate"), "{text}");
    }

    /// Reload is not a relaunch: the destination applies to the first load and
    /// leaves later ones alone.
    #[test]
    fn the_destination_applies_to_the_first_load_only() {
        let (mut app, _dir, path) = app_launched_with(&["--open", "ui/status-bar"]);
        assert_eq!(app.active_tab, TabId::Ui);

        app.active_tab = TabId::Capture;
        app.active_ui_tab = UiTabId::HelpOverlay;
        load_config_file(&mut app, &path);

        assert_eq!(app.active_tab, TabId::Capture);
        assert_eq!(app.active_ui_tab, UiTabId::HelpOverlay);
    }

    #[test]
    fn a_failed_first_load_still_honors_the_destination() {
        let (mut app, _effects) =
            ConfiguratorApp::new_app_with_startup(StartupRequest::from_args(args(&[
                "--open", "boards",
            ])));

        let _ = app.update_command(CommandMessage::ConfigLoaded(Err("broken".to_string())));

        assert_eq!(app.active_tab, TabId::Boards);
    }

    /// The overlay names a shortcut's section from the main crate, which
    /// cannot see `KeybindingField::tab()`; this side can see both, so it is
    /// where the two groupings are held together. The configurator splits the
    /// config's `core` group across three tabs and merges capture with zoom, so
    /// nothing derives one from the other — only this check keeps a new or
    /// moved action from sending the user to the wrong subtab.
    #[test]
    fn every_action_section_matches_the_field_tab_that_holds_it() {
        use crate::models::KeybindingField;
        use wayscriber::config::keybindings::KeybindingsConfig;
        use wayscriber::configurator_destination::keybindings_section_for_action;

        let fields = KeybindingField::all();
        let actions = KeybindingsConfig::configurable_actions();
        assert_eq!(
            fields.len(),
            actions.len(),
            "the configurator and the config disagree about how many shortcuts exist"
        );

        for action in actions {
            let key = KeybindingsConfig::config_key_for_action(*action)
                .unwrap_or_else(|| panic!("{action:?} is configurable, so it has a config key"));
            let field = fields
                .iter()
                .find(|field| field.field_key() == key)
                .unwrap_or_else(|| panic!("no configurator field edits {key}"));
            let section = keybindings_section_for_action(*action)
                .unwrap_or_else(|| panic!("{action:?} names no Keybindings section"));

            assert_eq!(
                keybindings_tab(section),
                field.tab(),
                "{action:?} would open the wrong Keybindings subtab"
            );
        }
    }

    /// The other half of a shortcut destination: its search term has to select
    /// the action's own row. Alignment moves off a subtab with no matches, so a
    /// term that misses would visibly land somewhere else.
    #[test]
    fn every_action_destination_keeps_the_subtab_it_asks_for() {
        use wayscriber::config::keybindings::KeybindingsConfig;
        use wayscriber::configurator_destination::{
            ConfiguratorScreen, keybindings_destination_for_action,
        };

        let (mut app, _dir, _path) = app_launched_with(&[]);

        for action in KeybindingsConfig::configurable_actions() {
            let destination = keybindings_destination_for_action(*action)
                .unwrap_or_else(|| panic!("{action:?} has no destination"));
            let ConfiguratorScreen::Keybindings(Some(section)) = destination.screen() else {
                panic!("{action:?} should name a Keybindings section");
            };
            let expected = keybindings_tab(section);

            app.active_tab = TabId::Keybindings;
            app.active_keybindings_tab = expected;
            app.search_query = SearchQuery::new(
                destination
                    .search()
                    .unwrap_or_else(|| panic!("{action:?} should carry a search term")),
            );
            app.align_active_tabs_for_search();

            assert_eq!(
                app.active_tab,
                TabId::Keybindings,
                "{action:?} searched itself off the Keybindings tab"
            );
            assert_eq!(
                app.active_keybindings_tab, expected,
                "{action:?} searched itself off its own subtab"
            );
        }
    }

    /// The whole round trip: a destination a launcher builds, spelled as one
    /// argv token, lands on the screen that destination names.
    #[test]
    fn a_launcher_built_destination_lands_where_it_names() {
        let launched = ConfiguratorDestination::with_search(
            ConfiguratorScreen::Keybindings(Some(KeybindingsSection::Boards)),
            "Next Board",
        );

        let (app, _dir, _path) = app_launched_with(&["--open", &launched.as_arg()]);

        assert_eq!(app.active_tab, TabId::Keybindings);
        assert_eq!(app.active_keybindings_tab, KeybindingsTabId::Boards);
        assert_eq!(app.search_query.raw(), "Next Board");
    }
}
