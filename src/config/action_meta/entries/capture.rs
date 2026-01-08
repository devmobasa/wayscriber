use super::ActionMeta;

pub const ENTRIES: &[ActionMeta] = &[
    meta!(
        CaptureFullScreen,
        "Capture Full Screen",
        None,
        "Capture the full screen",
        Capture,
        false,
        false,
        false
    ),
    meta!(
        CaptureActiveWindow,
        "Capture Active Window",
        None,
        "Capture the active window",
        Capture,
        false,
        true,
        false
    ),
    meta!(
        CaptureSelection,
        "Capture Selection",
        None,
        "Capture a selection using defaults",
        Capture,
        false,
        true,
        false
    ),
    meta!(
        CaptureClipboardFull,
        "Capture to Clipboard",
        None,
        "Screenshot to clipboard",
        Capture,
        true,
        true,
        false
    ),
    meta!(
        CaptureFileFull,
        "Capture to File",
        None,
        "Screenshot to file",
        Capture,
        true,
        true,
        false
    ),
    meta!(
        CaptureClipboardSelection,
        "Capture Selection to Clipboard",
        None,
        "Selection to clipboard",
        Capture,
        false,
        true,
        false
    ),
    meta!(
        CaptureFileSelection,
        "Capture Selection to File",
        None,
        "Selection to file",
        Capture,
        false,
        true,
        false
    ),
    meta!(
        CaptureClipboardRegion,
        "Capture Region to Clipboard",
        None,
        "Region to clipboard",
        Capture,
        false,
        false,
        false
    ),
    meta!(
        CaptureFileRegion,
        "Capture Region to File",
        None,
        "Region to file",
        Capture,
        false,
        false,
        false
    ),
    meta!(
        OpenCaptureFolder,
        "Open Capture Folder",
        None,
        "Open screenshot folder",
        Capture,
        true,
        true,
        false
    ),
];
