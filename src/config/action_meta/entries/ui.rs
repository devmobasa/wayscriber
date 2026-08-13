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
        ToggleFocusMode,
        "Focus Mode",
        None,
        "Hide all UI chrome; press again to restore it",
        UI,
        true,
        true,
        false,
        &[
            "clean screen",
            "hide all ui",
            "hide everything",
            "distraction free"
        ]
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
        ToggleInputHud,
        "Input HUD",
        None,
        "Show keystrokes and clicks on screen",
        UI,
        true,
        true,
        false,
        &[
            "keystrokes",
            "keycast",
            "show keys",
            "screencast",
            "key overlay"
        ]
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
        OpenConfiguratorKeybindings,
        "Edit Shortcuts\u{2026}",
        Some("Shortcuts"),
        "Open the configurator's Keybindings screen",
        UI,
        true,
        false,
        false,
        &[
            "edit shortcuts",
            "rebind",
            "keybindings",
            "hotkeys",
            "change shortcut"
        ]
    ),
    meta!(
        OpenConfiguratorPresets,
        "Edit Presets\u{2026}",
        Some("Presets"),
        "Open the configurator's Presets screen",
        UI,
        true,
        false,
        false,
        &["edit presets", "preset library", "save preset for good"]
    ),
    meta!(
        OpenConfiguratorBoards,
        "Edit Board Defaults\u{2026}",
        Some("Board Defaults"),
        "Open the configurator's Boards screen (templates for new sessions)",
        UI,
        true,
        false,
        false,
        &[
            "edit boards",
            "board defaults",
            "board templates",
            "default boards"
        ]
    ),
    meta!(
        OpenConfiguratorQuickColors,
        "Edit Quick Colors\u{2026}",
        Some("Quick Colors"),
        "Open the configurator's Drawing screen at the quick-color palette",
        UI,
        true,
        false,
        false,
        &["edit quick colors", "palette", "swatches", "color library"]
    ),
    meta!(
        OpenConfiguratorOnboardingHints,
        "Tip Settings…",
        Some("Tip Settings"),
        "Open General UI at the automatic-guidance preference",
        UI,
        true,
        false,
        false
    ),
    meta!(
        OpenAbout,
        "About Wayscriber",
        Some("About"),
        "Show version, links, and update status",
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
