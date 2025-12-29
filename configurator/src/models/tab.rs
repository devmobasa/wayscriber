#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabId {
    Drawing,
    Presets,
    Arrow,
    History,
    Performance,
    Ui,
    Board,
    Capture,
    Session,
    Keybindings,
    Tablet,
}

impl TabId {
    pub const ALL: [TabId; 11] = [
        TabId::Drawing,
        TabId::Presets,
        TabId::Ui,
        TabId::Board,
        TabId::Performance,
        TabId::History,
        TabId::Capture,
        TabId::Session,
        TabId::Keybindings,
        TabId::Arrow,
        TabId::Tablet,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            TabId::Drawing => "Drawing",
            TabId::Presets => "Presets",
            TabId::Arrow => "Arrow",
            TabId::History => "History",
            TabId::Performance => "Performance",
            TabId::Ui => "UI",
            TabId::Board => "Board",
            TabId::Capture => "Capture",
            TabId::Session => "Session",
            TabId::Keybindings => "Keybindings",
            TabId::Tablet => "Tablet",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiTabId {
    Toolbar,
    StatusBar,
    HelpOverlay,
    ClickHighlight,
}

impl UiTabId {
    pub const ALL: [UiTabId; 4] = [
        UiTabId::Toolbar,
        UiTabId::StatusBar,
        UiTabId::HelpOverlay,
        UiTabId::ClickHighlight,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            UiTabId::Toolbar => "Toolbar",
            UiTabId::StatusBar => "Status Bar",
            UiTabId::HelpOverlay => "Help Overlay",
            UiTabId::ClickHighlight => "Click Highlight",
        }
    }
}
