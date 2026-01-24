use super::KeybindingField;
use crate::models::KeybindingsTabId;

impl KeybindingField {
    pub fn tab(&self) -> KeybindingsTabId {
        match self {
            Self::Exit | Self::OpenConfigurator => KeybindingsTabId::General,
            Self::EnterTextMode
            | Self::EnterStickyNoteMode
            | Self::ClearCanvas
            | Self::IncreaseThickness
            | Self::DecreaseThickness
            | Self::IncreaseMarkerOpacity
            | Self::DecreaseMarkerOpacity
            | Self::IncreaseFontSize
            | Self::DecreaseFontSize
            | Self::ToggleFill
            | Self::SetColorRed
            | Self::SetColorGreen
            | Self::SetColorBlue
            | Self::SetColorYellow
            | Self::SetColorOrange
            | Self::SetColorPink
            | Self::SetColorWhite
            | Self::SetColorBlack => KeybindingsTabId::Drawing,
            Self::SelectSelectionTool
            | Self::SelectPenTool
            | Self::SelectEraserTool
            | Self::ToggleEraserMode
            | Self::SelectMarkerTool
            | Self::SelectStepMarkerTool
            | Self::SelectLineTool
            | Self::SelectRectTool
            | Self::SelectEllipseTool
            | Self::SelectArrowTool
            | Self::SelectHighlightTool
            | Self::ToggleHighlightTool
            | Self::ResetArrowLabels
            | Self::ResetStepMarkers => KeybindingsTabId::Tools,
            Self::DuplicateSelection
            | Self::CopySelection
            | Self::PasteSelection
            | Self::SelectAll
            | Self::MoveSelectionToFront
            | Self::MoveSelectionToBack
            | Self::MoveSelectionToStart
            | Self::MoveSelectionToEnd
            | Self::MoveSelectionToTop
            | Self::MoveSelectionToBottom
            | Self::NudgeSelectionUp
            | Self::NudgeSelectionDown
            | Self::NudgeSelectionLeft
            | Self::NudgeSelectionRight
            | Self::NudgeSelectionUpLarge
            | Self::NudgeSelectionDownLarge
            | Self::DeleteSelection => KeybindingsTabId::Selection,
            Self::Undo
            | Self::Redo
            | Self::UndoAll
            | Self::RedoAll
            | Self::UndoAllDelayed
            | Self::RedoAllDelayed => KeybindingsTabId::History,
            Self::ToggleWhiteboard
            | Self::ToggleBlackboard
            | Self::ReturnToTransparent
            | Self::PagePrev
            | Self::PageNext
            | Self::PageNew
            | Self::PageDuplicate
            | Self::PageDelete
            | Self::Board1
            | Self::Board2
            | Self::Board3
            | Self::Board4
            | Self::Board5
            | Self::Board6
            | Self::Board7
            | Self::Board8
            | Self::Board9
            | Self::BoardNext
            | Self::BoardPrev
            | Self::BoardNew
            | Self::BoardDuplicate
            | Self::BoardDelete
            | Self::BoardPicker => KeybindingsTabId::Boards,
            Self::ToggleHelp
            | Self::ToggleQuickHelp
            | Self::ToggleStatusBar
            | Self::ToggleClickHighlight
            | Self::ToggleToolbar
            | Self::TogglePresenterMode
            | Self::ToggleSelectionProperties
            | Self::OpenContextMenu
            | Self::ToggleCommandPalette => KeybindingsTabId::UiModes,
            Self::CaptureFullScreen
            | Self::CaptureActiveWindow
            | Self::CaptureSelection
            | Self::CaptureClipboardFull
            | Self::CaptureFileFull
            | Self::CaptureClipboardSelection
            | Self::CaptureFileSelection
            | Self::CaptureClipboardRegion
            | Self::CaptureFileRegion
            | Self::OpenCaptureFolder
            | Self::ToggleFrozenMode
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::ResetZoom
            | Self::ToggleZoomLock
            | Self::RefreshZoomCapture => KeybindingsTabId::CaptureView,
            Self::ApplyPreset1
            | Self::ApplyPreset2
            | Self::ApplyPreset3
            | Self::ApplyPreset4
            | Self::ApplyPreset5
            | Self::SavePreset1
            | Self::SavePreset2
            | Self::SavePreset3
            | Self::SavePreset4
            | Self::SavePreset5
            | Self::ClearPreset1
            | Self::ClearPreset2
            | Self::ClearPreset3
            | Self::ClearPreset4
            | Self::ClearPreset5 => KeybindingsTabId::Presets,
        }
    }
}
