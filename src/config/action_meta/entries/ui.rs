use super::ActionMeta;

pub const ENTRIES: &[ActionMeta] = &[
    meta!(
        ToggleHelp,
        "Toggle Help",
        None,
        "Show keyboard shortcuts",
        UI,
        true,
        true,
        false
    ),
    meta!(
        ToggleQuickHelp,
        "Quick Reference",
        None,
        "Show quick reference shortcuts",
        UI,
        true,
        true,
        false
    ),
    meta!(
        ToggleToolbar,
        "Toggle Toolbar",
        None,
        "Show/hide toolbars",
        UI,
        true,
        true,
        false
    ),
    meta!(
        ToggleStatusBar,
        "Toggle Status Bar",
        None,
        "Show/hide status bar",
        UI,
        true,
        true,
        false
    ),
    meta!(
        TogglePresenterMode,
        "Presenter Mode",
        None,
        "Toggle presenter mode",
        UI,
        true,
        true,
        false
    ),
    meta!(
        ToggleClickHighlight,
        "Click Highlight",
        None,
        "Toggle click highlighting",
        UI,
        true,
        true,
        false
    ),
    meta!(
        ToggleSelectionProperties,
        "Selection Properties",
        None,
        "Show selection properties",
        UI,
        false,
        true,
        false
    ),
    meta!(
        OpenContextMenu,
        "Context Menu",
        None,
        "Open the context menu",
        UI,
        false,
        true,
        false
    ),
    meta!(
        OpenConfigurator,
        "Open Configurator",
        Some("Config UI"),
        "Open settings configurator",
        UI,
        true,
        true,
        true
    ),
    meta!(
        ToggleCommandPalette,
        "Command Palette",
        None,
        "Search all commands",
        UI,
        true,
        true,
        false
    ),
    meta!(
        ReplayTour,
        "Replay Tour",
        None,
        "Start the guided tour again",
        UI,
        true,
        false,
        false
    ),
];
