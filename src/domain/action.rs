use serde::{Deserialize, Serialize};

/// All possible actions that can be bound to keys.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    IncreasePenSmoothing,
    DecreasePenSmoothing,
    CycleFontFamily,
    SelectSelectionTool,
    SelectMarkerTool,
    SelectStepMarkerTool,
    SelectEraserTool,
    ToggleEraserMode,
    CycleBlurStyle,
    CycleArrowStyle,
    SelectPenTool,
    SelectLineTool,
    SelectRectTool,
    SelectEllipseTool,
    SelectTriangleTool,
    SelectParallelogramTool,
    SelectRhombusTool,
    SelectRegularPolygonTool,
    SelectFreeformPolygonTool,
    SelectArrowTool,
    SelectBlurTool,
    SelectSpotlightTool,
    SelectHighlightTool,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetArrowLabelCounter,
    ResetStepMarkerCounter,

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
    FocusNextOutput,
    FocusPrevOutput,

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
    /// Show/hide the floating board/page badge (the pill that appears when
    /// the status bar is hidden or `show_floating_badge_always` is set).
    ToggleFloatingBadge,
    /// Show/hide the bottom-right zoom chip.
    ToggleZoomChip,
    /// Hide every persistent chrome surface at once (toolbars, status bar,
    /// floating badge, zoom chip); a second press restores the exact prior
    /// visibility.
    ToggleFocusMode,
    ToggleClickHighlight,
    /// Show/hide the on-screen input HUD (keystrokes and clicks).
    ToggleInputHud,
    ToggleToolbar,
    /// Cycle the top toolbar's display: full strip → micro chip → hidden.
    CycleToolbarDisplay,
    TogglePresenterMode,
    ToggleLightMode,
    ToggleLightModeDrawing,
    RenderProfileNext,
    RenderProfilePrevious,
    RenderProfileOff,
    ToggleHighlightTool,
    ToggleFill,
    ToggleRadialMenu,
    ToggleSelectionProperties,
    OpenContextMenu,

    // Configurator
    OpenConfigurator,
    /// Open the configurator's Keybindings screen, which owns every shortcut.
    OpenConfiguratorKeybindings,
    /// Open the configurator's Presets screen, which owns the preset library.
    OpenConfiguratorPresets,
    /// Open the configurator's Boards screen, which owns the board templates a
    /// new session starts from.
    OpenConfiguratorBoards,
    /// Open the configurator's Drawing screen at the quick-color palette.
    OpenConfiguratorQuickColors,
    /// Open General UI at the automatic-guidance preference.
    OpenConfiguratorOnboardingHints,
    ClearSavedToolState,
    OpenAbout,

    // Color selections (using char to represent the color)
    SetColorRed,
    SetColorGreen,
    SetColorBlue,
    SetColorYellow,
    SetColorOrange,
    SetColorPink,
    SetColorWhite,
    SetColorBlack,
    /// Pick a drawing color from the currently displayed desktop capture.
    PickScreenColor,

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
    /// Select a screen region, then choose its destination in the review UI.
    CaptureRegionInteractive,
    /// Measure a logical screen region without capturing or delivering pixels.
    MeasureMode,
    ExportCanvasFile,
    ExportCanvasClipboard,
    ExportCanvasClipboardAndFile,
    ExportBoardPdfFile,
    ExportAllBoardsPdfFile,
    OpenCaptureFolder,
    /// Select a screen region and copy the text recognized in it.
    CopyTextFromScreen,
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

impl Action {
    /// Whether this action opens or addresses the native screen-region picker.
    pub const fn is_region_capture(self) -> bool {
        matches!(
            self,
            Self::CaptureSelection
                | Self::CaptureClipboardSelection
                | Self::CaptureFileSelection
                | Self::CaptureClipboardRegion
                | Self::CaptureFileRegion
                | Self::CaptureRegionInteractive
        )
    }
}
