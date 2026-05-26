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
        ExportCanvasFile,
        "Export Canvas to File",
        Some("Canvas to File"),
        "Export persisted canvas as PNG",
        Capture,
        true,
        true,
        false,
        &["save board", "export board", "export canvas", "png"]
    ),
    meta!(
        ExportCanvasClipboard,
        "Export Canvas to Clipboard",
        Some("Canvas to Clipboard"),
        "Copy persisted canvas PNG to clipboard",
        Capture,
        true,
        true,
        false,
        &["export board", "export canvas", "png", "clipboard"]
    ),
    meta!(
        ExportCanvasClipboardAndFile,
        "Export Canvas to Clipboard and File",
        Some("Canvas to Clipboard and File"),
        "Copy persisted canvas PNG to clipboard and save it",
        Capture,
        true,
        true,
        false,
        &[
            "save board",
            "export board",
            "export canvas",
            "png",
            "clipboard"
        ]
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
