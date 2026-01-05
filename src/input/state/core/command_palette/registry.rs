//! Registry of all commands available in the command palette.

use crate::config::keybindings::Action;

/// Category for grouping commands.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
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

/// A command entry in the palette.
#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub action: Action,
    pub label: &'static str,
    pub description: &'static str,
    #[allow(dead_code)]
    pub category: CommandCategory,
}

/// Static registry of all commands.
pub static COMMAND_REGISTRY: &[CommandEntry] = &[
    // Core
    CommandEntry {
        action: Action::Exit,
        label: "Exit",
        description: "Close the overlay",
        category: CommandCategory::Core,
    },
    // Tools
    CommandEntry {
        action: Action::SelectPenTool,
        label: "Pen Tool",
        description: "Freehand drawing",
        category: CommandCategory::Tools,
    },
    CommandEntry {
        action: Action::SelectLineTool,
        label: "Line Tool",
        description: "Draw straight lines",
        category: CommandCategory::Tools,
    },
    CommandEntry {
        action: Action::SelectRectTool,
        label: "Rectangle Tool",
        description: "Draw rectangles",
        category: CommandCategory::Tools,
    },
    CommandEntry {
        action: Action::SelectEllipseTool,
        label: "Ellipse Tool",
        description: "Draw ellipses and circles",
        category: CommandCategory::Tools,
    },
    CommandEntry {
        action: Action::SelectArrowTool,
        label: "Arrow Tool",
        description: "Draw arrows",
        category: CommandCategory::Tools,
    },
    CommandEntry {
        action: Action::SelectHighlightTool,
        label: "Highlight Tool",
        description: "Highlight areas",
        category: CommandCategory::Tools,
    },
    CommandEntry {
        action: Action::SelectMarkerTool,
        label: "Marker Tool",
        description: "Semi-transparent marker",
        category: CommandCategory::Tools,
    },
    CommandEntry {
        action: Action::SelectEraserTool,
        label: "Eraser Tool",
        description: "Erase drawings",
        category: CommandCategory::Tools,
    },
    CommandEntry {
        action: Action::EnterTextMode,
        label: "Text Mode",
        description: "Add text annotations",
        category: CommandCategory::Tools,
    },
    CommandEntry {
        action: Action::EnterStickyNoteMode,
        label: "Sticky Note",
        description: "Add sticky note",
        category: CommandCategory::Tools,
    },
    // Colors
    CommandEntry {
        action: Action::SetColorRed,
        label: "Red",
        description: "Set color to red",
        category: CommandCategory::Colors,
    },
    CommandEntry {
        action: Action::SetColorGreen,
        label: "Green",
        description: "Set color to green",
        category: CommandCategory::Colors,
    },
    CommandEntry {
        action: Action::SetColorBlue,
        label: "Blue",
        description: "Set color to blue",
        category: CommandCategory::Colors,
    },
    CommandEntry {
        action: Action::SetColorYellow,
        label: "Yellow",
        description: "Set color to yellow",
        category: CommandCategory::Colors,
    },
    CommandEntry {
        action: Action::SetColorOrange,
        label: "Orange",
        description: "Set color to orange",
        category: CommandCategory::Colors,
    },
    CommandEntry {
        action: Action::SetColorPink,
        label: "Pink",
        description: "Set color to pink",
        category: CommandCategory::Colors,
    },
    CommandEntry {
        action: Action::SetColorWhite,
        label: "White",
        description: "Set color to white",
        category: CommandCategory::Colors,
    },
    CommandEntry {
        action: Action::SetColorBlack,
        label: "Black",
        description: "Set color to black",
        category: CommandCategory::Colors,
    },
    // Drawing
    CommandEntry {
        action: Action::IncreaseThickness,
        label: "Increase Thickness",
        description: "Make strokes thicker",
        category: CommandCategory::Drawing,
    },
    CommandEntry {
        action: Action::DecreaseThickness,
        label: "Decrease Thickness",
        description: "Make strokes thinner",
        category: CommandCategory::Drawing,
    },
    CommandEntry {
        action: Action::ToggleFill,
        label: "Toggle Fill",
        description: "Enable/disable shape fill",
        category: CommandCategory::Drawing,
    },
    CommandEntry {
        action: Action::ToggleEraserMode,
        label: "Toggle Eraser Mode",
        description: "Switch to/from eraser",
        category: CommandCategory::Drawing,
    },
    // History
    CommandEntry {
        action: Action::Undo,
        label: "Undo",
        description: "Undo last action",
        category: CommandCategory::History,
    },
    CommandEntry {
        action: Action::Redo,
        label: "Redo",
        description: "Redo last undone action",
        category: CommandCategory::History,
    },
    CommandEntry {
        action: Action::ClearCanvas,
        label: "Clear Canvas",
        description: "Remove all drawings",
        category: CommandCategory::History,
    },
    // Selection
    CommandEntry {
        action: Action::SelectAll,
        label: "Select All",
        description: "Select all shapes",
        category: CommandCategory::Selection,
    },
    CommandEntry {
        action: Action::DeleteSelection,
        label: "Delete Selection",
        description: "Delete selected shapes",
        category: CommandCategory::Selection,
    },
    CommandEntry {
        action: Action::DuplicateSelection,
        label: "Duplicate Selection",
        description: "Duplicate selected shapes",
        category: CommandCategory::Selection,
    },
    CommandEntry {
        action: Action::CopySelection,
        label: "Copy",
        description: "Copy selection to clipboard",
        category: CommandCategory::Selection,
    },
    CommandEntry {
        action: Action::PasteSelection,
        label: "Paste",
        description: "Paste from clipboard",
        category: CommandCategory::Selection,
    },
    // UI
    CommandEntry {
        action: Action::ToggleHelp,
        label: "Toggle Help",
        description: "Show keyboard shortcuts",
        category: CommandCategory::UI,
    },
    CommandEntry {
        action: Action::ToggleToolbar,
        label: "Toggle Toolbar",
        description: "Show/hide toolbars",
        category: CommandCategory::UI,
    },
    CommandEntry {
        action: Action::ToggleStatusBar,
        label: "Toggle Status Bar",
        description: "Show/hide status bar",
        category: CommandCategory::UI,
    },
    CommandEntry {
        action: Action::TogglePresenterMode,
        label: "Presenter Mode",
        description: "Toggle presenter mode",
        category: CommandCategory::UI,
    },
    CommandEntry {
        action: Action::ToggleClickHighlight,
        label: "Click Highlight",
        description: "Toggle click highlighting",
        category: CommandCategory::UI,
    },
    CommandEntry {
        action: Action::OpenConfigurator,
        label: "Open Configurator",
        description: "Open settings configurator",
        category: CommandCategory::UI,
    },
    // Board
    CommandEntry {
        action: Action::ToggleWhiteboard,
        label: "Whiteboard Mode",
        description: "Toggle whiteboard background",
        category: CommandCategory::Board,
    },
    CommandEntry {
        action: Action::ToggleBlackboard,
        label: "Blackboard Mode",
        description: "Toggle blackboard background",
        category: CommandCategory::Board,
    },
    CommandEntry {
        action: Action::ReturnToTransparent,
        label: "Transparent Mode",
        description: "Return to transparent overlay",
        category: CommandCategory::Board,
    },
    CommandEntry {
        action: Action::PageNext,
        label: "Next Page",
        description: "Go to next page",
        category: CommandCategory::Board,
    },
    CommandEntry {
        action: Action::PagePrev,
        label: "Previous Page",
        description: "Go to previous page",
        category: CommandCategory::Board,
    },
    CommandEntry {
        action: Action::PageNew,
        label: "New Page",
        description: "Create a new page",
        category: CommandCategory::Board,
    },
    // Zoom
    CommandEntry {
        action: Action::ZoomIn,
        label: "Zoom In",
        description: "Increase zoom level",
        category: CommandCategory::Zoom,
    },
    CommandEntry {
        action: Action::ZoomOut,
        label: "Zoom Out",
        description: "Decrease zoom level",
        category: CommandCategory::Zoom,
    },
    CommandEntry {
        action: Action::ResetZoom,
        label: "Reset Zoom",
        description: "Reset to 100% zoom",
        category: CommandCategory::Zoom,
    },
    CommandEntry {
        action: Action::ToggleZoomLock,
        label: "Lock Zoom",
        description: "Lock/unlock zoom position",
        category: CommandCategory::Zoom,
    },
    CommandEntry {
        action: Action::ToggleFrozenMode,
        label: "Freeze Screen",
        description: "Freeze the screen capture",
        category: CommandCategory::Zoom,
    },
    // Capture
    CommandEntry {
        action: Action::CaptureClipboardFull,
        label: "Capture to Clipboard",
        description: "Screenshot to clipboard",
        category: CommandCategory::Capture,
    },
    CommandEntry {
        action: Action::CaptureFileFull,
        label: "Capture to File",
        description: "Screenshot to file",
        category: CommandCategory::Capture,
    },
    CommandEntry {
        action: Action::OpenCaptureFolder,
        label: "Open Capture Folder",
        description: "Open screenshot folder",
        category: CommandCategory::Capture,
    },
    // Presets
    CommandEntry {
        action: Action::ApplyPreset1,
        label: "Apply Preset 1",
        description: "Apply saved preset 1",
        category: CommandCategory::Presets,
    },
    CommandEntry {
        action: Action::ApplyPreset2,
        label: "Apply Preset 2",
        description: "Apply saved preset 2",
        category: CommandCategory::Presets,
    },
    CommandEntry {
        action: Action::ApplyPreset3,
        label: "Apply Preset 3",
        description: "Apply saved preset 3",
        category: CommandCategory::Presets,
    },
    CommandEntry {
        action: Action::ApplyPreset4,
        label: "Apply Preset 4",
        description: "Apply saved preset 4",
        category: CommandCategory::Presets,
    },
    CommandEntry {
        action: Action::ApplyPreset5,
        label: "Apply Preset 5",
        description: "Apply saved preset 5",
        category: CommandCategory::Presets,
    },
    // Help
    CommandEntry {
        action: Action::ReplayTour,
        label: "Replay Tour",
        description: "Start the guided tour again",
        category: CommandCategory::UI,
    },
];
