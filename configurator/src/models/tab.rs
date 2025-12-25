#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabId {
    Drawing,
    Arrow,
    Performance,
    Ui,
    Board,
    Capture,
    Session,
    Keybindings,
}

impl TabId {
    pub const ALL: [TabId; 8] = [
        TabId::Drawing,
        TabId::Ui,
        TabId::Board,
        TabId::Performance,
        TabId::Capture,
        TabId::Session,
        TabId::Keybindings,
        TabId::Arrow,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            TabId::Drawing => "Drawing",
            TabId::Arrow => "Arrow",
            TabId::Performance => "Performance",
            TabId::Ui => "UI",
            TabId::Board => "Board",
            TabId::Capture => "Capture",
            TabId::Session => "Session",
            TabId::Keybindings => "Keybindings",
        }
    }
}
