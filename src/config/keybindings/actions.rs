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
    SelectSelectionTool,
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

    // Board switching
    #[serde(rename = "board_1")]
    Board1,
    #[serde(rename = "board_2")]
    Board2,
    #[serde(rename = "board_3")]
    Board3,
    #[serde(rename = "board_4")]
    Board4,
    #[serde(rename = "board_5")]
    Board5,
    #[serde(rename = "board_6")]
    Board6,
    #[serde(rename = "board_7")]
    Board7,
    #[serde(rename = "board_8")]
    Board8,
    #[serde(rename = "board_9")]
    Board9,
    BoardNext,
    BoardPrev,
    BoardNew,
    BoardDelete,
    BoardPicker,
    BoardRestoreDeleted,
    BoardDuplicate,
    BoardSwitchRecent,

    // Page navigation
    PagePrev,
    PageNext,
    PageNew,
    PageDuplicate,
    PageDelete,
    PageRestoreDeleted,

    // UI toggles
    ToggleHelp,
    ToggleQuickHelp,
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
