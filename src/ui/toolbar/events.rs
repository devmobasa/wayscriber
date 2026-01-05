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
    EnterTextMode,
    EnterStickyNoteMode,
    /// Toggle both highlight tool and click highlight together
    ToggleAllHighlight(bool),
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
    /// Toggle Actions section visibility (undo all, redo all, etc.)
    ToggleActionsSection(bool),
    /// Toggle advanced action buttons
    ToggleActionsAdvanced(bool),
    /// Toggle zoom action buttons
    ToggleZoomActions(bool),
    /// Toggle Pages section visibility
    TogglePagesSection(bool),
    /// Toggle presets section visibility
    TogglePresets(bool),
    /// Toggle Step Undo/Redo section visibility
    ToggleStepSection(bool),
    /// Toggle persistent text controls visibility
    ToggleTextControls(bool),
    /// Toggle preset action toast notifications
    TogglePresetToasts(bool),
    /// Toggle cursor tool preview bubble
    #[allow(dead_code)]
    ToggleToolPreview(bool),
    /// Toggle status bar visibility
    ToggleStatusBar(bool),
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
