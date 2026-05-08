use crate::config::{Action, ToolbarLayoutMode, action_label, action_short_label};
use crate::draw::{Color, FontDescriptor};
use crate::input::{EraserMode, Tool, ToolbarDrawerTab};

use super::ToolbarSnapshot;

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

impl ToolbarEvent {
    pub fn action(&self) -> Option<Action> {
        match self {
            Self::SelectTool(tool) => action_for_tool(*tool),
            Self::EnterTextMode => Some(Action::EnterTextMode),
            Self::EnterStickyNoteMode => Some(Action::EnterStickyNoteMode),
            Self::ToggleFill(_) => Some(Action::ToggleFill),
            Self::Undo => Some(Action::Undo),
            Self::Redo => Some(Action::Redo),
            Self::UndoAll => Some(Action::UndoAll),
            Self::RedoAll => Some(Action::RedoAll),
            Self::UndoAllDelayed => Some(Action::UndoAllDelayed),
            Self::RedoAllDelayed => Some(Action::RedoAllDelayed),
            Self::ClearCanvas => Some(Action::ClearCanvas),
            Self::PagePrev => Some(Action::PagePrev),
            Self::PageNext => Some(Action::PageNext),
            Self::PageNew => Some(Action::PageNew),
            Self::PageDuplicate => Some(Action::PageDuplicate),
            Self::PageDelete => Some(Action::PageDelete),
            Self::BoardPrev => Some(Action::BoardPrev),
            Self::BoardNext => Some(Action::BoardNext),
            Self::BoardNew => Some(Action::BoardNew),
            Self::BoardDelete => Some(Action::BoardDelete),
            Self::BoardDuplicate => Some(Action::BoardDuplicate),
            Self::BoardRename | Self::ToggleBoardPicker => Some(Action::BoardPicker),
            Self::ToggleAllHighlight(_) => Some(Action::ToggleHighlightTool),
            Self::ToggleFreeze => Some(Action::ToggleFrozenMode),
            Self::ZoomIn => Some(Action::ZoomIn),
            Self::ZoomOut => Some(Action::ZoomOut),
            Self::ResetZoom => Some(Action::ResetZoom),
            Self::ResetStepMarkerCounter => Some(Action::ResetStepMarkerCounter),
            Self::ToggleZoomLock => Some(Action::ToggleZoomLock),
            Self::ApplyPreset(slot) => action_for_apply_preset(*slot),
            Self::SavePreset(slot) => action_for_save_preset(*slot),
            Self::ClearPreset(slot) => action_for_clear_preset(*slot),
            Self::OpenConfigurator => Some(Action::OpenConfigurator),
            _ => None,
        }
    }

    pub fn short_label(&self, snapshot: &ToolbarSnapshot, fallback: &'static str) -> &'static str {
        match self {
            Self::ToggleFreeze if snapshot.frozen_active => "Unfreeze",
            Self::ToggleZoomLock if snapshot.zoom_locked => "Unlock Zoom",
            _ => self.action().map(action_short_label).unwrap_or(fallback),
        }
    }

    pub fn tooltip_label(
        &self,
        snapshot: &ToolbarSnapshot,
        fallback: &'static str,
    ) -> &'static str {
        match self {
            Self::ToggleFreeze if snapshot.frozen_active => "Unfreeze",
            Self::ToggleZoomLock if snapshot.zoom_locked => "Unlock Zoom",
            _ => self.action().map(action_label).unwrap_or(fallback),
        }
    }
}

pub(crate) fn action_for_tool(tool: Tool) -> Option<Action> {
    match tool {
        Tool::Select => Some(Action::SelectSelectionTool),
        Tool::Pen => Some(Action::SelectPenTool),
        Tool::Line => Some(Action::SelectLineTool),
        Tool::Rect => Some(Action::SelectRectTool),
        Tool::Ellipse => Some(Action::SelectEllipseTool),
        Tool::Arrow => Some(Action::SelectArrowTool),
        Tool::Blur => Some(Action::SelectBlurTool),
        Tool::Marker => Some(Action::SelectMarkerTool),
        Tool::StepMarker => Some(Action::SelectStepMarkerTool),
        Tool::Highlight => Some(Action::SelectHighlightTool),
        Tool::Eraser => Some(Action::SelectEraserTool),
    }
}

pub(crate) fn action_for_apply_preset(slot: usize) -> Option<Action> {
    match slot {
        1 => Some(Action::ApplyPreset1),
        2 => Some(Action::ApplyPreset2),
        3 => Some(Action::ApplyPreset3),
        4 => Some(Action::ApplyPreset4),
        5 => Some(Action::ApplyPreset5),
        _ => None,
    }
}

pub(crate) fn action_for_save_preset(slot: usize) -> Option<Action> {
    match slot {
        1 => Some(Action::SavePreset1),
        2 => Some(Action::SavePreset2),
        3 => Some(Action::SavePreset3),
        4 => Some(Action::SavePreset4),
        5 => Some(Action::SavePreset5),
        _ => None,
    }
}

pub(crate) fn action_for_clear_preset(slot: usize) -> Option<Action> {
    match slot {
        1 => Some(Action::ClearPreset1),
        2 => Some(Action::ClearPreset2),
        3 => Some(Action::ClearPreset3),
        4 => Some(Action::ClearPreset4),
        5 => Some(Action::ClearPreset5),
        _ => None,
    }
}
