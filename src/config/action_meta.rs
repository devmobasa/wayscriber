use super::Action;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionCategory {
    Core,
    Drawing,
    Tools,
    Colors,
    UI,
    Board,
    Zoom,
    Capture,
    Selection,
    History,
    Presets,
}

#[derive(Debug, Clone, Copy)]
pub struct ActionMeta {
    pub action: Action,
    pub label: &'static str,
    pub short_label: Option<&'static str>,
    pub description: &'static str,
    #[allow(dead_code)]
    pub category: ActionCategory,
    pub in_command_palette: bool,
    pub in_help: bool,
    pub in_toolbar: bool,
}

impl ActionMeta {
    pub fn short_label(self) -> &'static str {
        self.short_label.unwrap_or(self.label)
    }
}

macro_rules! meta {
    (
        $action:ident,
        $label:expr,
        $short:expr,
        $desc:expr,
        $category:ident,
        $in_palette:expr,
        $in_help:expr,
        $in_toolbar:expr
    ) => {
        ActionMeta {
            action: Action::$action,
            label: $label,
            short_label: $short,
            description: $desc,
            category: ActionCategory::$category,
            in_command_palette: $in_palette,
            in_help: $in_help,
            in_toolbar: $in_toolbar,
        }
    };
}

pub const ACTION_META: &[ActionMeta] = &[
    meta!(Exit, "Exit", None, "Close the overlay", Core, true, true, false),
    meta!(EnterTextMode, "Text Mode", Some("Text"), "Add text annotations", Tools, true, true, true),
    meta!(EnterStickyNoteMode, "Sticky Note", Some("Note"), "Add sticky note", Tools, true, true, true),
    meta!(ClearCanvas, "Clear Canvas", Some("Clear"), "Remove all drawings", History, true, true, true),
    meta!(Undo, "Undo", None, "Undo last action", History, true, true, true),
    meta!(Redo, "Redo", None, "Redo last undone action", History, true, true, true),
    meta!(UndoAll, "Undo All", None, "Undo all actions", History, false, false, true),
    meta!(RedoAll, "Redo All", None, "Redo all actions", History, false, false, true),
    meta!(UndoAllDelayed, "Undo All Delay", None, "Undo all actions with delay", History, false, false, true),
    meta!(RedoAllDelayed, "Redo All Delay", None, "Redo all actions with delay", History, false, false, true),
    meta!(SelectPenTool, "Pen Tool", Some("Pen"), "Freehand drawing", Tools, true, true, true),
    meta!(SelectLineTool, "Line Tool", Some("Line"), "Draw straight lines", Tools, true, true, true),
    meta!(SelectRectTool, "Rectangle Tool", Some("Rect"), "Draw rectangles", Tools, true, true, true),
    meta!(SelectEllipseTool, "Ellipse Tool", Some("Circle"), "Draw ellipses and circles", Tools, true, true, true),
    meta!(SelectArrowTool, "Arrow Tool", Some("Arrow"), "Draw arrows", Tools, true, true, true),
    meta!(SelectHighlightTool, "Highlight Tool", Some("Highlight"), "Highlight areas", Tools, true, false, false),
    meta!(ToggleHighlightTool, "Toggle Highlight", Some("Highlight"), "Toggle highlight tool and click highlight", Tools, false, true, true),
    meta!(SelectMarkerTool, "Marker Tool", Some("Marker"), "Semi-transparent marker", Tools, true, true, true),
    meta!(SelectEraserTool, "Eraser Tool", Some("Eraser"), "Erase drawings", Tools, true, true, true),
    meta!(ToggleEraserMode, "Toggle Eraser Mode", None, "Switch to/from eraser", Tools, true, false, true),
    meta!(IncreaseThickness, "Increase Thickness", None, "Make strokes thicker", Drawing, true, true, false),
    meta!(DecreaseThickness, "Decrease Thickness", None, "Make strokes thinner", Drawing, true, true, false),
    meta!(IncreaseMarkerOpacity, "Increase Marker Opacity", None, "Increase marker opacity", Drawing, false, false, false),
    meta!(DecreaseMarkerOpacity, "Decrease Marker Opacity", None, "Decrease marker opacity", Drawing, false, false, false),
    meta!(IncreaseFontSize, "Increase Font Size", None, "Make text larger", Drawing, false, true, false),
    meta!(DecreaseFontSize, "Decrease Font Size", None, "Make text smaller", Drawing, false, true, false),
    meta!(ResetArrowLabelCounter, "Reset Arrow Labels", None, "Reset arrow label counter", Drawing, false, false, false),
    meta!(ToggleFill, "Toggle Fill", Some("Fill"), "Enable/disable shape fill", Drawing, true, true, true),
    meta!(ToggleWhiteboard, "Whiteboard Mode", None, "Toggle whiteboard background", Board, true, true, false),
    meta!(ToggleBlackboard, "Blackboard Mode", None, "Toggle blackboard background", Board, true, true, false),
    meta!(ReturnToTransparent, "Transparent Mode", None, "Return to transparent overlay", Board, true, true, false),
    meta!(PagePrev, "Previous Page", Some("Prev"), "Go to previous page", Board, true, true, true),
    meta!(PageNext, "Next Page", Some("Next"), "Go to next page", Board, true, true, true),
    meta!(PageNew, "New Page", Some("New"), "Create a new page", Board, true, true, true),
    meta!(PageDuplicate, "Duplicate Page", Some("Dup"), "Duplicate the current page", Board, false, true, true),
    meta!(PageDelete, "Delete Page", Some("Del"), "Delete the current page", Board, false, true, true),
    meta!(ToggleHelp, "Toggle Help", None, "Show keyboard shortcuts", UI, true, true, false),
    meta!(ToggleToolbar, "Toggle Toolbar", None, "Show/hide toolbars", UI, true, true, false),
    meta!(ToggleStatusBar, "Toggle Status Bar", None, "Show/hide status bar", UI, true, true, false),
    meta!(TogglePresenterMode, "Presenter Mode", None, "Toggle presenter mode", UI, true, true, false),
    meta!(ToggleClickHighlight, "Click Highlight", None, "Toggle click highlighting", UI, true, true, false),
    meta!(ToggleSelectionProperties, "Selection Properties", None, "Show selection properties", UI, false, true, false),
    meta!(OpenContextMenu, "Context Menu", None, "Open the context menu", UI, false, true, false),
    meta!(OpenConfigurator, "Open Configurator", Some("Config UI"), "Open settings configurator", UI, true, true, true),
    meta!(ToggleCommandPalette, "Command Palette", None, "Toggle the command palette", UI, false, false, false),
    meta!(ReplayTour, "Replay Tour", None, "Start the guided tour again", UI, true, false, false),
    meta!(SetColorRed, "Red", None, "Set color to red", Colors, true, true, false),
    meta!(SetColorGreen, "Green", None, "Set color to green", Colors, true, true, false),
    meta!(SetColorBlue, "Blue", None, "Set color to blue", Colors, true, true, false),
    meta!(SetColorYellow, "Yellow", None, "Set color to yellow", Colors, true, true, false),
    meta!(SetColorOrange, "Orange", None, "Set color to orange", Colors, true, true, false),
    meta!(SetColorPink, "Pink", None, "Set color to pink", Colors, true, true, false),
    meta!(SetColorWhite, "White", None, "Set color to white", Colors, true, true, false),
    meta!(SetColorBlack, "Black", None, "Set color to black", Colors, true, true, false),
    meta!(CaptureFullScreen, "Capture Full Screen", None, "Capture the full screen", Capture, false, false, false),
    meta!(CaptureActiveWindow, "Capture Active Window", None, "Capture the active window", Capture, false, true, false),
    meta!(CaptureSelection, "Capture Selection", None, "Capture a selection using defaults", Capture, false, true, false),
    meta!(CaptureClipboardFull, "Capture to Clipboard", None, "Screenshot to clipboard", Capture, true, true, false),
    meta!(CaptureFileFull, "Capture to File", None, "Screenshot to file", Capture, true, true, false),
    meta!(CaptureClipboardSelection, "Capture Selection to Clipboard", None, "Selection to clipboard", Capture, false, true, false),
    meta!(CaptureFileSelection, "Capture Selection to File", None, "Selection to file", Capture, false, true, false),
    meta!(CaptureClipboardRegion, "Capture Region to Clipboard", None, "Region to clipboard", Capture, false, false, false),
    meta!(CaptureFileRegion, "Capture Region to File", None, "Region to file", Capture, false, false, false),
    meta!(OpenCaptureFolder, "Open Capture Folder", None, "Open screenshot folder", Capture, true, true, false),
    meta!(ToggleFrozenMode, "Freeze Screen", Some("Freeze"), "Freeze the screen capture", Zoom, true, true, true),
    meta!(ZoomIn, "Zoom In", None, "Increase zoom level", Zoom, true, true, true),
    meta!(ZoomOut, "Zoom Out", None, "Decrease zoom level", Zoom, true, true, true),
    meta!(ResetZoom, "Reset Zoom", None, "Reset to 100% zoom", Zoom, true, true, true),
    meta!(ToggleZoomLock, "Lock Zoom", None, "Lock/unlock zoom position", Zoom, true, true, true),
    meta!(RefreshZoomCapture, "Refresh Zoom", None, "Refresh zoom capture", Zoom, false, true, false),
    meta!(SelectAll, "Select All", None, "Select all shapes", Selection, true, true, false),
    meta!(DeleteSelection, "Delete Selection", None, "Delete selected shapes", Selection, true, true, false),
    meta!(DuplicateSelection, "Duplicate Selection", None, "Duplicate selected shapes", Selection, true, true, false),
    meta!(CopySelection, "Copy", None, "Copy selection to clipboard", Selection, true, true, false),
    meta!(PasteSelection, "Paste", None, "Paste from clipboard", Selection, true, true, false),
    meta!(ApplyPreset1, "Apply Preset 1", None, "Apply saved preset 1", Presets, true, false, true),
    meta!(ApplyPreset2, "Apply Preset 2", None, "Apply saved preset 2", Presets, true, false, true),
    meta!(ApplyPreset3, "Apply Preset 3", None, "Apply saved preset 3", Presets, true, false, true),
    meta!(ApplyPreset4, "Apply Preset 4", None, "Apply saved preset 4", Presets, true, false, true),
    meta!(ApplyPreset5, "Apply Preset 5", None, "Apply saved preset 5", Presets, true, false, true),
    meta!(SavePreset1, "Save Preset 1", None, "Save preset 1", Presets, false, false, true),
    meta!(SavePreset2, "Save Preset 2", None, "Save preset 2", Presets, false, false, true),
    meta!(SavePreset3, "Save Preset 3", None, "Save preset 3", Presets, false, false, true),
    meta!(SavePreset4, "Save Preset 4", None, "Save preset 4", Presets, false, false, true),
    meta!(SavePreset5, "Save Preset 5", None, "Save preset 5", Presets, false, false, true),
    meta!(ClearPreset1, "Clear Preset 1", None, "Clear preset 1", Presets, false, false, true),
    meta!(ClearPreset2, "Clear Preset 2", None, "Clear preset 2", Presets, false, false, true),
    meta!(ClearPreset3, "Clear Preset 3", None, "Clear preset 3", Presets, false, false, true),
    meta!(ClearPreset4, "Clear Preset 4", None, "Clear preset 4", Presets, false, false, true),
    meta!(ClearPreset5, "Clear Preset 5", None, "Clear preset 5", Presets, false, false, true),
];

pub fn action_meta(action: Action) -> Option<&'static ActionMeta> {
    ACTION_META.iter().find(|meta| meta.action == action)
}

pub fn action_label(action: Action) -> &'static str {
    action_meta(action).map(|meta| meta.label).unwrap_or("Action")
}

pub fn action_short_label(action: Action) -> &'static str {
    action_meta(action)
        .map(|meta| meta.short_label())
        .unwrap_or("Action")
}

#[allow(dead_code)]
pub fn action_description(action: Action) -> &'static str {
    action_meta(action)
        .map(|meta| meta.description)
        .unwrap_or("")
}
