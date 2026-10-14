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

impl From<wayscriber::configurator_destination::KeybindingsSection> for KeybindingsTabId {
    fn from(section: wayscriber::configurator_destination::KeybindingsSection) -> Self {
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
