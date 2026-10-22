use wayscriber::configurator_destination::KeybindingsSection;
use wayscriber::i18n::tr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabId {
    Drawing,
    Presets,
    Arrow,
    History,
    Performance,
    Ui,
    Boards,
    RenderProfiles,
    Capture,
    Daemon,
    Session,
    Keybindings,
    #[cfg(feature = "tablet-input")]
    Tablet,
}

impl TabId {
    #[cfg(feature = "tablet-input")]
    pub const ALL: [TabId; 13] = [
        TabId::Daemon,
        TabId::Drawing,
        TabId::Presets,
        TabId::Ui,
        TabId::Boards,
        TabId::RenderProfiles,
        TabId::Performance,
        TabId::History,
        TabId::Capture,
        TabId::Session,
        TabId::Keybindings,
        TabId::Arrow,
        TabId::Tablet,
    ];

    #[cfg(not(feature = "tablet-input"))]
    pub const ALL: [TabId; 12] = [
        TabId::Daemon,
        TabId::Drawing,
        TabId::Presets,
        TabId::Ui,
        TabId::Boards,
        TabId::RenderProfiles,
        TabId::Performance,
        TabId::History,
        TabId::Capture,
        TabId::Session,
        TabId::Keybindings,
        TabId::Arrow,
    ];

    /// Fixed `GtkStack` child name. Never translated: the stack looks pages up
    /// by this name, and the display title changes with the UI language.
    pub fn stack_name(&self) -> &'static str {
        match self {
            TabId::Drawing => "drawing",
            TabId::Presets => "presets",
            TabId::Arrow => "arrow",
            TabId::History => "history",
            TabId::Performance => "performance",
            TabId::Ui => "ui",
            TabId::Boards => "boards",
            TabId::RenderProfiles => "render-profiles",
            TabId::Capture => "capture",
            TabId::Daemon => "daemon",
            TabId::Session => "session",
            TabId::Keybindings => "keybindings",
            #[cfg(feature = "tablet-input")]
            TabId::Tablet => "tablet",
        }
    }

    /// The title in English, the source text [`Self::title`] translates.
    /// Search matches it in every UI language.
    pub fn source_title(&self) -> &'static str {
        match self {
            TabId::Drawing => "Drawing",
            TabId::Presets => "Presets",
            TabId::Arrow => "Arrow",
            TabId::History => "History",
            TabId::Performance => "Performance",
            TabId::Ui => "UI",
            TabId::Boards => "Boards",
            TabId::RenderProfiles => "Render Profiles",
            TabId::Capture => "Capture",
            TabId::Daemon => "Background Mode",
            TabId::Session => "Session",
            TabId::Keybindings => "Keybindings",
            #[cfg(feature = "tablet-input")]
            TabId::Tablet => "Tablet",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            TabId::Drawing => tr("Drawing"),
            TabId::Presets => tr("Presets"),
            TabId::Arrow => tr("Arrow"),
            TabId::History => tr("History"),
            TabId::Performance => tr("Performance"),
            TabId::Ui => "UI",
            TabId::Boards => tr("Boards"),
            TabId::RenderProfiles => tr("Render Profiles"),
            TabId::Capture => tr("Capture"),
            TabId::Daemon => tr("Background Mode"),
            TabId::Session => tr("Session"),
            TabId::Keybindings => tr("Keybindings"),
            #[cfg(feature = "tablet-input")]
            TabId::Tablet => tr("Tablet"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiTabId {
    Toolbar,
    ToolbarVisibility,
    StatusBar,
    HelpOverlay,
    ClickHighlight,
    InputHud,
    PresenterMode,
}

impl UiTabId {
    pub const ALL: [UiTabId; 7] = [
        UiTabId::Toolbar,
        UiTabId::ToolbarVisibility,
        UiTabId::StatusBar,
        UiTabId::HelpOverlay,
        UiTabId::ClickHighlight,
        UiTabId::InputHud,
        UiTabId::PresenterMode,
    ];

    /// Fixed `GtkStack` child name; see [`TabId::stack_name`].
    pub fn stack_name(&self) -> &'static str {
        match self {
            UiTabId::Toolbar => "toolbar",
            UiTabId::ToolbarVisibility => "toolbar-visibility",
            UiTabId::StatusBar => "status-bar",
            UiTabId::HelpOverlay => "help-overlay",
            UiTabId::ClickHighlight => "click-highlight",
            UiTabId::InputHud => "input-hud",
            UiTabId::PresenterMode => "presenter-mode",
        }
    }

    /// The name a stack child reports back, as a tab.
    pub fn from_stack_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.stack_name() == name)
    }

    /// The title in English; see [`TabId::source_title`].
    pub fn source_title(&self) -> &'static str {
        match self {
            UiTabId::Toolbar => "Toolbar",
            UiTabId::ToolbarVisibility => "Toolbar Visibility",
            UiTabId::StatusBar => "Status Bar",
            UiTabId::HelpOverlay => "Help Overlay",
            UiTabId::ClickHighlight => "Click Highlight",
            UiTabId::InputHud => "Input HUD",
            UiTabId::PresenterMode => "Presenter Mode",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            UiTabId::Toolbar => tr("Toolbar"),
            UiTabId::ToolbarVisibility => tr("Toolbar Visibility"),
            UiTabId::StatusBar => tr("Status Bar"),
            UiTabId::HelpOverlay => tr("Help Overlay"),
            UiTabId::ClickHighlight => tr("Click Highlight"),
            UiTabId::InputHud => tr("Input HUD"),
            UiTabId::PresenterMode => tr("Presenter Mode"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeybindingsTabId {
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

impl From<KeybindingsSection> for KeybindingsTabId {
    fn from(section: KeybindingsSection) -> Self {
        use wayscriber::configurator_destination::KeybindingsSection;

        match section {
            KeybindingsSection::General => Self::General,
            KeybindingsSection::Drawing => Self::Drawing,
            KeybindingsSection::Tools => Self::Tools,
            KeybindingsSection::Selection => Self::Selection,
            KeybindingsSection::History => Self::History,
            KeybindingsSection::Boards => Self::Boards,
            KeybindingsSection::UiModes => Self::UiModes,
            KeybindingsSection::CaptureView => Self::CaptureView,
            KeybindingsSection::Presets => Self::Presets,
        }
    }
}

impl KeybindingsTabId {
    pub const ALL: [KeybindingsTabId; 9] = [
        KeybindingsTabId::General,
        KeybindingsTabId::Drawing,
        KeybindingsTabId::Tools,
        KeybindingsTabId::Selection,
        KeybindingsTabId::History,
        KeybindingsTabId::Boards,
        KeybindingsTabId::UiModes,
        KeybindingsTabId::CaptureView,
        KeybindingsTabId::Presets,
    ];

    /// The title in English; see [`TabId::source_title`].
    pub fn source_title(&self) -> &'static str {
        match self {
            KeybindingsTabId::General => "General",
            KeybindingsTabId::Drawing => "Drawing",
            KeybindingsTabId::Tools => "Tools",
            KeybindingsTabId::Selection => "Selection",
            KeybindingsTabId::History => "History",
            KeybindingsTabId::Boards => "Boards",
            KeybindingsTabId::UiModes => "UI & Modes",
            KeybindingsTabId::CaptureView => "Capture & View",
            KeybindingsTabId::Presets => "Presets",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            KeybindingsTabId::General => tr("General"),
            KeybindingsTabId::Drawing => tr("Drawing"),
            KeybindingsTabId::Tools => tr("Tools"),
            KeybindingsTabId::Selection => tr("Selection"),
            KeybindingsTabId::History => tr("History"),
            KeybindingsTabId::Boards => tr("Boards"),
            KeybindingsTabId::UiModes => tr("UI & Modes"),
            KeybindingsTabId::CaptureView => tr("Capture & View"),
            KeybindingsTabId::Presets => tr("Presets"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use wayscriber::i18n;

    use crate::test_env;

    use super::*;
    use wayscriber::domain::Language;

    #[test]
    fn english_titles_are_the_source_titles() {
        // In-process tests always draw in English; see `in_language_child`.
        for tab in TabId::ALL {
            assert_eq!(tab.title(), tab.source_title(), "{tab:?}");
        }
        for tab in UiTabId::ALL {
            assert_eq!(tab.title(), tab.source_title(), "{tab:?}");
        }
        for tab in KeybindingsTabId::ALL {
            assert_eq!(tab.title(), tab.source_title(), "{tab:?}");
        }
    }

    #[test]
    fn stack_names_are_fixed_unique_and_untranslated() {
        test_env::in_language_child(
            concat!(
                module_path!(),
                "::stack_names_are_fixed_unique_and_untranslated"
            ),
            stack_names_survive_translation,
        );
    }

    fn stack_names_survive_translation() {
        let english: Vec<&str> = TabId::ALL.iter().map(TabId::stack_name).collect();
        i18n::set_language(Language::SimplifiedChinese);
        let chinese: Vec<&str> = TabId::ALL.iter().map(TabId::stack_name).collect();

        assert_eq!(english, chinese);
        assert_eq!(chinese.iter().collect::<BTreeSet<_>>().len(), chinese.len());
        assert!(chinese.iter().all(|name| name.is_ascii()));
        // Translated titles are what used to name the children.
        assert!(
            TabId::ALL
                .iter()
                .any(|tab| tab.title() != tab.source_title())
        );

        let ui_names: BTreeSet<&str> = UiTabId::ALL.iter().map(UiTabId::stack_name).collect();
        assert_eq!(ui_names.len(), UiTabId::ALL.len());
        for tab in UiTabId::ALL {
            assert_eq!(UiTabId::from_stack_name(tab.stack_name()), Some(tab));
            assert_eq!(UiTabId::from_stack_name(tab.title()), None, "{tab:?}");
        }
    }
}
