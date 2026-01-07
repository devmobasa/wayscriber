use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// All possible actions that can be bound to keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    // Exit and cancellation
    Exit,

    // Drawing actions
    EnterTextMode,
    EnterStickyNoteMode,
    ClearCanvas,
    Undo,
    Redo,
    UndoAll,
    RedoAll,
    UndoAllDelayed,
    RedoAllDelayed,
    DuplicateSelection,
    CopySelection,
    PasteSelection,
    SelectAll,
    MoveSelectionToFront,
    MoveSelectionToBack,
    NudgeSelectionUp,
    NudgeSelectionDown,
    NudgeSelectionLeft,
    NudgeSelectionRight,
    NudgeSelectionUpLarge,
    NudgeSelectionDownLarge,
    MoveSelectionToStart,
    MoveSelectionToEnd,
    MoveSelectionToTop,
    MoveSelectionToBottom,
    DeleteSelection,

    // Thickness controls
    IncreaseThickness,
    DecreaseThickness,
    IncreaseMarkerOpacity,
    DecreaseMarkerOpacity,
    SelectMarkerTool,
    SelectEraserTool,
    ToggleEraserMode,
    SelectPenTool,
    SelectLineTool,
    SelectRectTool,
    SelectEllipseTool,
    SelectArrowTool,
    SelectHighlightTool,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetArrowLabelCounter,

    // Board mode toggles
    ToggleWhiteboard,
    ToggleBlackboard,
    ReturnToTransparent,

    // Page navigation
    PagePrev,
    PageNext,
    PageNew,
    PageDuplicate,
    PageDelete,

    // UI toggles
    ToggleHelp,
    ToggleStatusBar,
    ToggleClickHighlight,
    ToggleToolbar,
    TogglePresenterMode,
    ToggleHighlightTool,
    ToggleFill,
    ToggleSelectionProperties,
    OpenContextMenu,

    // Configurator
    OpenConfigurator,

    // Color selections (using char to represent the color)
    SetColorRed,
    SetColorGreen,
    SetColorBlue,
    SetColorYellow,
    SetColorOrange,
    SetColorPink,
    SetColorWhite,
    SetColorBlack,

    // Screenshot capture actions
    CaptureFullScreen,
    CaptureActiveWindow,
    CaptureSelection,
    CaptureClipboardFull,
    CaptureFileFull,
    CaptureClipboardSelection,
    CaptureFileSelection,
    CaptureClipboardRegion,
    CaptureFileRegion,
    OpenCaptureFolder,
    ToggleFrozenMode,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    ToggleZoomLock,
    RefreshZoomCapture,

    // Preset slots
    ApplyPreset1,
    ApplyPreset2,
    ApplyPreset3,
    ApplyPreset4,
    ApplyPreset5,
    SavePreset1,
    SavePreset2,
    SavePreset3,
    SavePreset4,
    SavePreset5,
    ClearPreset1,
    ClearPreset2,
    ClearPreset3,
    ClearPreset4,
    ClearPreset5,

    // Command palette
    ToggleCommandPalette,

    // Onboarding
    ReplayTour,

    // Clipboard fallback
    SavePendingToFile,
}
