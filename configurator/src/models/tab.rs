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
            UiTabId::Toolbar => "Toolbar",
            UiTabId::ToolbarVisibility => "Toolbar Visibility",
            UiTabId::StatusBar => "Status Bar",
            UiTabId::HelpOverlay => "Help Overlay",
            UiTabId::ClickHighlight => "Click Highlight",
            UiTabId::InputHud => "Input HUD",
            UiTabId::PresenterMode => "Presenter Mode",
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
}
