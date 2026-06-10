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
    PresenterMode,
}

impl UiTabId {
    pub const ALL: [UiTabId; 6] = [
        UiTabId::Toolbar,
        UiTabId::ToolbarVisibility,
        UiTabId::StatusBar,
        UiTabId::HelpOverlay,
        UiTabId::ClickHighlight,
        UiTabId::PresenterMode,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            UiTabId::Toolbar => "Toolbar",
            UiTabId::ToolbarVisibility => "Toolbar Visibility",
            UiTabId::StatusBar => "Status Bar",
            UiTabId::HelpOverlay => "Help Overlay",
            UiTabId::ClickHighlight => "Click Highlight",
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
