use super::ActionMeta;

pub const ENTRIES: &[ActionMeta] = &[
    meta!(
        ToggleFrozenMode,
        "Freeze Screen",
        Some("Freeze"),
        "Freeze the screen capture",
        Zoom,
        true,
        true,
        true
    ),
    meta!(
        ZoomIn,
        "Zoom In",
        None,
        "Increase zoom level",
        Zoom,
        true,
        true,
        true
    ),
    meta!(
        ZoomOut,
        "Zoom Out",
        None,
        "Decrease zoom level",
        Zoom,
        true,
        true,
        true
    ),
    meta!(
        ResetZoom,
        "Reset Zoom",
        None,
        "Reset to 100% zoom",
        Zoom,
        true,
        true,
        true
    ),
    meta!(
        ToggleZoomLock,
        "Lock Zoom",
        None,
        "Lock/unlock zoom position",
        Zoom,
        true,
        true,
        true
    ),
    meta!(
        RefreshZoomCapture,
        "Refresh Zoom",
        None,
        "Refresh zoom capture",
        Zoom,
        false,
        true,
        false
    ),
];
