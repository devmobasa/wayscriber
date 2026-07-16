use super::adapters;
use super::outcome::{ActionRoute, RoutingOutcome};
use crate::domain::Action;
use crate::input::state::InputState;

pub(crate) fn classify_action(action: Action) -> ActionRoute {
    match action {
        Action::Exit
        | Action::EnterTextMode
        | Action::EnterStickyNoteMode
        | Action::ClearCanvas => ActionRoute::Core,
        Action::Undo
        | Action::Redo
        | Action::UndoAll
        | Action::RedoAll
        | Action::UndoAllDelayed
        | Action::RedoAllDelayed => ActionRoute::History,
        Action::DuplicateSelection
        | Action::CopySelection
        | Action::PasteSelection
        | Action::SelectAll
        | Action::MoveSelectionToFront
        | Action::MoveSelectionToBack
        | Action::NudgeSelectionUp
        | Action::NudgeSelectionDown
        | Action::NudgeSelectionLeft
        | Action::NudgeSelectionRight
        | Action::NudgeSelectionUpLarge
        | Action::NudgeSelectionDownLarge
        | Action::MoveSelectionToStart
        | Action::MoveSelectionToEnd
        | Action::MoveSelectionToTop
        | Action::MoveSelectionToBottom
        | Action::DeleteSelection => ActionRoute::Selection,
        Action::IncreaseThickness
        | Action::DecreaseThickness
        | Action::IncreaseMarkerOpacity
        | Action::DecreaseMarkerOpacity
        | Action::SelectSelectionTool
        | Action::SelectMarkerTool
        | Action::SelectStepMarkerTool
        | Action::SelectEraserTool
        | Action::ToggleEraserMode
        | Action::SelectPenTool
        | Action::SelectLineTool
        | Action::SelectRectTool
        | Action::SelectEllipseTool
        | Action::SelectTriangleTool
        | Action::SelectParallelogramTool
        | Action::SelectRhombusTool
        | Action::SelectRegularPolygonTool
        | Action::SelectFreeformPolygonTool
        | Action::SelectArrowTool
        | Action::SelectBlurTool
        | Action::SelectHighlightTool
        | Action::IncreaseFontSize
        | Action::DecreaseFontSize
        | Action::ResetArrowLabelCounter
        | Action::ResetStepMarkerCounter
        | Action::ToggleHighlightTool
        | Action::ToggleFill => ActionRoute::Tool,
        Action::ToggleWhiteboard
        | Action::ToggleBlackboard
        | Action::ReturnToTransparent
        | Action::Board1
        | Action::Board2
        | Action::Board3
        | Action::Board4
        | Action::Board5
        | Action::Board6
        | Action::Board7
        | Action::Board8
        | Action::Board9
        | Action::BoardNext
        | Action::BoardPrev
        | Action::BoardNew
        | Action::BoardDelete
        | Action::BoardPicker
        | Action::BoardRestoreDeleted
        | Action::BoardDuplicate
        | Action::BoardSwitchRecent
        | Action::PagePrev
        | Action::PageNext
        | Action::PageNew
        | Action::PageDuplicate
        | Action::PageDelete
        | Action::PageRestoreDeleted => ActionRoute::BoardPages,
        Action::ToggleHelp
        | Action::ToggleQuickHelp
        | Action::ToggleStatusBar
        | Action::ToggleClickHighlight
        | Action::ToggleToolbar
        | Action::TogglePresenterMode
        | Action::ToggleLightMode
        | Action::ToggleLightModeDrawing
        | Action::RenderProfileNext
        | Action::RenderProfilePrevious
        | Action::RenderProfileOff
        | Action::ToggleRadialMenu
        | Action::ToggleSelectionProperties
        | Action::OpenContextMenu
        | Action::OpenConfigurator
        | Action::ClearSavedToolState
        | Action::OpenCaptureFolder
        | Action::ToggleCommandPalette
        | Action::ReplayTour => ActionRoute::Ui,
        Action::SetColorRed
        | Action::SetColorGreen
        | Action::SetColorBlue
        | Action::SetColorYellow
        | Action::SetColorOrange
        | Action::SetColorPink
        | Action::SetColorWhite
        | Action::SetColorBlack
        | Action::PickScreenColor => ActionRoute::Color,
        Action::CaptureFullScreen
        | Action::CaptureActiveWindow
        | Action::CaptureSelection
        | Action::CaptureClipboardFull
        | Action::CaptureFileFull
        | Action::CaptureClipboardSelection
        | Action::CaptureFileSelection
        | Action::CaptureClipboardRegion
        | Action::CaptureFileRegion
        | Action::ExportCanvasFile
        | Action::ExportCanvasClipboard
        | Action::ExportCanvasClipboardAndFile
        | Action::ExportBoardPdfFile
        | Action::ExportAllBoardsPdfFile
        | Action::ToggleFrozenMode
        | Action::ZoomIn
        | Action::ZoomOut
        | Action::ResetZoom
        | Action::ToggleZoomLock
        | Action::RefreshZoomCapture
        | Action::FocusNextOutput
        | Action::FocusPrevOutput
        | Action::SavePendingToFile => ActionRoute::CaptureZoom,
        Action::ApplyPreset1
        | Action::ApplyPreset2
        | Action::ApplyPreset3
        | Action::ApplyPreset4
        | Action::ApplyPreset5
        | Action::SavePreset1
        | Action::SavePreset2
        | Action::SavePreset3
        | Action::SavePreset4
        | Action::SavePreset5
        | Action::ClearPreset1
        | Action::ClearPreset2
        | Action::ClearPreset3
        | Action::ClearPreset4
        | Action::ClearPreset5 => ActionRoute::Preset,
    }
}

pub(crate) fn route_action(state: &mut InputState, action: Action) -> RoutingOutcome {
    if !matches!(
        action,
        Action::OpenContextMenu | Action::ToggleSelectionProperties
    ) {
        adapters::close_properties_panel_before_action(state);
    }

    let route = classify_action(action);
    adapters::dispatch_action(state, action, route);
    RoutingOutcome::DispatchedAction(route)
}
