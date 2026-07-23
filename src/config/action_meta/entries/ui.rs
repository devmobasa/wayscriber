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
        CycleToolbarDisplay,
        "Cycle Toolbar Display",
        Some("Toolbar Display"),
        "Cycle top toolbar: full, micro chip, hidden",
        UI,
        true,
        true,
        false,
        &["micro toolbar", "toolbar mode", "compact toolbar"]
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
        ToggleFloatingBadge,
        "Toggle Board/Page Badge",
        None,
        "Show/hide the floating board/page badge",
        UI,
        true,
        true,
        false,
        &["hide badge", "board badge", "page badge"]
    ),
    meta!(
        ToggleZoomChip,
        "Toggle Zoom Chip",
        None,
        "Show/hide the bottom-right zoom chip",
        UI,
        true,
        true,
        false,
        &["hide zoom", "zoom controls"]
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
        ToggleLightMode,
        "Light Mode",
        None,
        "Toggle passthrough light mode",
        UI,
        true,
        true,
        false,
        &["passthrough", "click through"]
    ),
    meta!(
        ToggleLightModeDrawing,
        "Light Drawing",
        None,
        "Toggle drawing while light mode is active",
        UI,
        true,
        true,
        false,
        &["passthrough draw", "quick draw"]
    ),
    meta!(
        RenderProfileNext,
        "Next Render Profile",
        Some("Next Profile"),
        "Switch to the next render color profile",
        UI,
        true,
        true,
        false,
        &["color profile", "print profile", "export theme"]
    ),
    meta!(
        RenderProfilePrevious,
        "Previous Render Profile",
        Some("Prev Profile"),
        "Switch to the previous render color profile",
        UI,
        true,
        true,
        false,
        &["color profile", "print profile", "export theme"]
    ),
    meta!(
        RenderProfileOff,
        "Render Profile Off",
        None,
        "Disable render color profile preview",
        UI,
        true,
        true,
        false,
        &["color profile off", "normal colors", "export theme off"]
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
        ToggleRadialMenu,
        "Radial Menu",
        None,
        "Toggle radial menu at cursor",
        UI,
        true,
        false,
        false,
        &["pie menu"]
    ),
    meta!(
        ToggleSelectionProperties,
        "Selection Properties",
        None,
        "Show selection properties",
        UI,
        true,
        true,
        false
    ),
    meta!(
        OpenContextMenu,
        "Context Menu",
        None,
        "Open the context menu",
        UI,
        true,
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
        ClearSavedToolState,
        "Reset Tool Defaults",
        None,
        "Clear saved tool state and apply config defaults",
        UI,
        true,
        true,
        false,
        &[
            "clear tool state",
            "clear saved tool state",
            "config defaults"
        ]
    ),
    meta!(
        ToggleCommandPalette,
        "Command Palette",
        None,
        "Search all commands",
        UI,
        true,
        true,
        true
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
