use crate::config::ToolbarLayoutMode;
use crate::draw::{Color, FontDescriptor};
use crate::input::{EraserMode, Tool, ToolbarDrawerTab};

/// Events emitted by the floating toolbar UI.
#[derive(Debug, Clone)]
pub enum ToolbarEvent {
    SelectTool(Tool),
    SetColor(Color),
    SetThickness(f64),
    NudgeThickness(f64),
    SetMarkerOpacity(f64),
    NudgeMarkerOpacity(f64),
    SetEraserMode(EraserMode),
    SetFont(FontDescriptor),
    SetFontSize(f64),
    ToggleFill(bool),
    ToggleArrowLabels(bool),
    ResetArrowLabelCounter,
    ResetStepMarkerCounter,
    SetUndoDelay(f64),
    SetRedoDelay(f64),
    UndoAll,
    RedoAll,
    UndoAllDelayed,
    RedoAllDelayed,
    Undo,
    Redo,
    ClearCanvas,
    PagePrev,
    PageNext,
    PageNew,
    PageDuplicate,
    PageDelete,
    BoardPrev,
    BoardNext,
    BoardNew,
    BoardDelete,
    BoardDuplicate,
    #[allow(dead_code)]
    BoardRename,
    ToggleBoardPicker,
    EnterTextMode,
    EnterStickyNoteMode,
    /// Toggle both highlight tool and click highlight together
    ToggleAllHighlight(bool),
    /// Toggle highlight tool ring visibility while the highlight tool is active
    ToggleHighlightToolRing(bool),
    ToggleFreeze,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    ToggleZoomLock,
    #[allow(dead_code)]
    RefreshZoomCapture,
    ApplyPreset(usize),
    SavePreset(usize),
    ClearPreset(usize),
    OpenConfigurator,
    OpenConfigFile,
    ToggleCustomSection(bool),
    ToggleDelaySliders(bool),
    SetCustomUndoDelay(f64),
    SetCustomRedoDelay(f64),
    SetCustomUndoSteps(usize),
    SetCustomRedoSteps(usize),
    CustomUndo,
    CustomRedo,
    /// Close the top toolbar panel
    CloseTopToolbar,
    /// Close the side toolbar panel
    CloseSideToolbar,
    /// Pin/unpin the top toolbar (saves to config)
    PinTopToolbar(bool),
    /// Pin/unpin the side toolbar (saves to config)
    PinSideToolbar(bool),
    /// Toggle between icon mode and text mode
    ToggleIconMode(bool),
    /// Toggle extended color palette
    ToggleMoreColors(bool),
    /// Copy current color as hex to clipboard
    CopyHexColor,
    /// Paste hex color from clipboard
    PasteHexColor,
    /// Open the color picker popup
    OpenColorPickerPopup,
    /// Toggle Actions section visibility (undo all, redo all, etc.)
    ToggleActionsSection(bool),
    /// Toggle advanced action buttons
    ToggleActionsAdvanced(bool),
    /// Toggle zoom action buttons
    ToggleZoomActions(bool),
    /// Toggle Pages section visibility
    TogglePagesSection(bool),
    /// Toggle Boards section visibility
    ToggleBoardsSection(bool),
    /// Toggle presets section visibility
    TogglePresets(bool),
    /// Toggle Step Undo/Redo section visibility
    ToggleStepSection(bool),
    /// Toggle persistent text controls visibility
    ToggleTextControls(bool),
    /// Toggle context-aware UI (show/hide controls based on active tool)
    ToggleContextAwareUi(bool),
    /// Toggle preset action toast notifications
    TogglePresetToasts(bool),
    /// Toggle cursor tool preview bubble
    #[allow(dead_code)]
    ToggleToolPreview(bool),
    /// Toggle status bar visibility
    ToggleStatusBar(bool),
    /// Toggle board label in the status bar
    ToggleStatusBoardBadge(bool),
    /// Toggle page counter in the status bar
    ToggleStatusPageBadge(bool),
    /// Toggle the board/page badge when the status bar is visible
    /// (renamed from TogglePageBadgeWithStatusBar for clarity)
    ToggleFloatingBadgeAlways(bool),
    /// Toggle the side drawer (Canvas/Settings)
    ToggleDrawer(bool),
    /// Switch the active drawer tab
    SetDrawerTab(ToolbarDrawerTab),
    /// Set toolbar layout mode
    SetToolbarLayoutMode(ToolbarLayoutMode),
    /// Toggle the simple-mode shape picker
    ToggleShapePicker(bool),
    /// Drag handle for top toolbar (toolbar coords; screen coords when inline toolbars are active)
    MoveTopToolbar {
        x: f64,
        y: f64,
    },
    /// Drag handle for side toolbar (toolbar coords; screen coords when inline toolbars are active)
    MoveSideToolbar {
        x: f64,
        y: f64,
    },
}
