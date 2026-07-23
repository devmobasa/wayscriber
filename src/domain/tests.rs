use std::fmt::Debug;
use std::fs;
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::{
    Action, BoardBackground, BoardSpec, Color, DragBindableTool, DragTool, EraserMode, Tool,
};

fn assert_json_names<T>(cases: &[(T, &str)])
where
    T: Copy + Debug + Eq + Serialize + DeserializeOwned,
{
    for &(value, expected) in cases {
        assert_eq!(
            serde_json::to_value(value).expect("serialize domain enum"),
            serde_json::Value::String(expected.to_string())
        );
        assert_eq!(
            serde_json::from_value::<T>(serde_json::Value::String(expected.to_string()))
                .expect("deserialize domain enum"),
            value
        );
    }
}

macro_rules! assert_action_json_names {
    ($(($value:path, $expected:literal $(,)?)),+ $(,)?) => {{
        fn established_name(action: Action) -> &'static str {
            match action {
                $($value => $expected,)+
            }
        }

        let actions = [$($value,)+];
        let cases: Vec<_> = actions
            .into_iter()
            .map(|action| (action, established_name(action)))
            .collect();
        assert_json_names(&cases);
    }};
}

#[test]
fn action_serialization_matches_established_contract() {
    assert_action_json_names!(
        (Action::Exit, "exit"),
        (Action::EnterTextMode, "enter_text_mode"),
        (Action::EnterStickyNoteMode, "enter_sticky_note_mode"),
        (Action::ClearCanvas, "clear_canvas"),
        (Action::Undo, "undo"),
        (Action::Redo, "redo"),
        (Action::UndoAll, "undo_all"),
        (Action::RedoAll, "redo_all"),
        (Action::UndoAllDelayed, "undo_all_delayed"),
        (Action::RedoAllDelayed, "redo_all_delayed"),
        (Action::DuplicateSelection, "duplicate_selection"),
        (Action::CopySelection, "copy_selection"),
        (Action::PasteSelection, "paste_selection"),
        (Action::SelectAll, "select_all"),
        (Action::MoveSelectionToFront, "move_selection_to_front"),
        (Action::MoveSelectionToBack, "move_selection_to_back"),
        (Action::NudgeSelectionUp, "nudge_selection_up"),
        (Action::NudgeSelectionDown, "nudge_selection_down"),
        (Action::NudgeSelectionLeft, "nudge_selection_left"),
        (Action::NudgeSelectionRight, "nudge_selection_right"),
        (Action::NudgeSelectionUpLarge, "nudge_selection_up_large"),
        (
            Action::NudgeSelectionDownLarge,
            "nudge_selection_down_large",
        ),
        (Action::MoveSelectionToStart, "move_selection_to_start"),
        (Action::MoveSelectionToEnd, "move_selection_to_end"),
        (Action::MoveSelectionToTop, "move_selection_to_top"),
        (Action::MoveSelectionToBottom, "move_selection_to_bottom"),
        (Action::DeleteSelection, "delete_selection"),
        (Action::IncreaseThickness, "increase_thickness"),
        (Action::DecreaseThickness, "decrease_thickness"),
        (Action::IncreaseMarkerOpacity, "increase_marker_opacity"),
        (Action::DecreaseMarkerOpacity, "decrease_marker_opacity"),
        (Action::SelectSelectionTool, "select_selection_tool"),
        (Action::SelectMarkerTool, "select_marker_tool"),
        (Action::SelectStepMarkerTool, "select_step_marker_tool"),
        (Action::SelectEraserTool, "select_eraser_tool"),
        (Action::ToggleEraserMode, "toggle_eraser_mode"),
        (Action::SelectPenTool, "select_pen_tool"),
        (Action::SelectLineTool, "select_line_tool"),
        (Action::SelectRectTool, "select_rect_tool"),
        (Action::SelectEllipseTool, "select_ellipse_tool"),
        (Action::SelectTriangleTool, "select_triangle_tool"),
        (Action::SelectParallelogramTool, "select_parallelogram_tool"),
        (Action::SelectRhombusTool, "select_rhombus_tool"),
        (
            Action::SelectRegularPolygonTool,
            "select_regular_polygon_tool",
        ),
        (
            Action::SelectFreeformPolygonTool,
            "select_freeform_polygon_tool",
        ),
        (Action::SelectArrowTool, "select_arrow_tool"),
        (Action::SelectBlurTool, "select_blur_tool"),
        (Action::SelectHighlightTool, "select_highlight_tool"),
        (Action::IncreaseFontSize, "increase_font_size"),
        (Action::DecreaseFontSize, "decrease_font_size"),
        (Action::ResetArrowLabelCounter, "reset_arrow_label_counter"),
        (Action::ResetStepMarkerCounter, "reset_step_marker_counter"),
        (Action::ToggleWhiteboard, "toggle_whiteboard"),
        (Action::ToggleBlackboard, "toggle_blackboard"),
        (Action::ReturnToTransparent, "return_to_transparent"),
        (Action::Board1, "board_1"),
        (Action::Board2, "board_2"),
        (Action::Board3, "board_3"),
        (Action::Board4, "board_4"),
        (Action::Board5, "board_5"),
        (Action::Board6, "board_6"),
        (Action::Board7, "board_7"),
        (Action::Board8, "board_8"),
        (Action::Board9, "board_9"),
        (Action::BoardNext, "board_next"),
        (Action::BoardPrev, "board_prev"),
        (Action::BoardNew, "board_new"),
        (Action::BoardDelete, "board_delete"),
        (Action::BoardPicker, "board_picker"),
        (Action::BoardRestoreDeleted, "board_restore_deleted"),
        (Action::BoardDuplicate, "board_duplicate"),
        (Action::BoardSwitchRecent, "board_switch_recent"),
        (Action::FocusNextOutput, "focus_next_output"),
        (Action::FocusPrevOutput, "focus_prev_output"),
        (Action::PagePrev, "page_prev"),
        (Action::PageNext, "page_next"),
        (Action::PageNew, "page_new"),
        (Action::PageDuplicate, "page_duplicate"),
        (Action::PageDelete, "page_delete"),
        (Action::PageRestoreDeleted, "page_restore_deleted"),
        (Action::ToggleHelp, "toggle_help"),
        (Action::ToggleQuickHelp, "toggle_quick_help"),
        (Action::ToggleStatusBar, "toggle_status_bar"),
        (Action::ToggleFloatingBadge, "toggle_floating_badge"),
        (Action::ToggleZoomChip, "toggle_zoom_chip"),
        (Action::ToggleClickHighlight, "toggle_click_highlight"),
        (Action::ToggleToolbar, "toggle_toolbar"),
        (Action::CycleToolbarDisplay, "cycle_toolbar_display"),
        (Action::TogglePresenterMode, "toggle_presenter_mode"),
        (Action::ToggleLightMode, "toggle_light_mode"),
        (Action::ToggleLightModeDrawing, "toggle_light_mode_drawing"),
        (Action::RenderProfileNext, "render_profile_next"),
        (Action::RenderProfilePrevious, "render_profile_previous"),
        (Action::RenderProfileOff, "render_profile_off"),
        (Action::ToggleHighlightTool, "toggle_highlight_tool"),
        (Action::ToggleFill, "toggle_fill"),
        (Action::ToggleRadialMenu, "toggle_radial_menu"),
        (
            Action::ToggleSelectionProperties,
            "toggle_selection_properties",
        ),
        (Action::OpenContextMenu, "open_context_menu"),
        (Action::OpenConfigurator, "open_configurator"),
        (Action::ClearSavedToolState, "clear_saved_tool_state"),
        (Action::SetColorRed, "set_color_red"),
        (Action::SetColorGreen, "set_color_green"),
        (Action::SetColorBlue, "set_color_blue"),
        (Action::SetColorYellow, "set_color_yellow"),
        (Action::SetColorOrange, "set_color_orange"),
        (Action::SetColorPink, "set_color_pink"),
        (Action::SetColorWhite, "set_color_white"),
        (Action::SetColorBlack, "set_color_black"),
        (Action::PickScreenColor, "pick_screen_color"),
        (Action::CaptureFullScreen, "capture_full_screen"),
        (Action::CaptureActiveWindow, "capture_active_window"),
        (Action::CaptureSelection, "capture_selection"),
        (Action::CaptureClipboardFull, "capture_clipboard_full"),
        (Action::CaptureFileFull, "capture_file_full"),
        (
            Action::CaptureClipboardSelection,
            "capture_clipboard_selection",
        ),
        (Action::CaptureFileSelection, "capture_file_selection"),
        (Action::CaptureClipboardRegion, "capture_clipboard_region"),
        (Action::CaptureFileRegion, "capture_file_region"),
        (Action::ExportCanvasFile, "export_canvas_file"),
        (Action::ExportCanvasClipboard, "export_canvas_clipboard"),
        (
            Action::ExportCanvasClipboardAndFile,
            "export_canvas_clipboard_and_file",
        ),
        (Action::ExportBoardPdfFile, "export_board_pdf_file"),
        (Action::ExportAllBoardsPdfFile, "export_all_boards_pdf_file"),
        (Action::OpenCaptureFolder, "open_capture_folder"),
        (Action::ToggleFrozenMode, "toggle_frozen_mode"),
        (Action::ZoomIn, "zoom_in"),
        (Action::ZoomOut, "zoom_out"),
        (Action::ResetZoom, "reset_zoom"),
        (Action::ToggleZoomLock, "toggle_zoom_lock"),
        (Action::RefreshZoomCapture, "refresh_zoom_capture"),
        (Action::ApplyPreset1, "apply_preset1"),
        (Action::ApplyPreset2, "apply_preset2"),
        (Action::ApplyPreset3, "apply_preset3"),
        (Action::ApplyPreset4, "apply_preset4"),
        (Action::ApplyPreset5, "apply_preset5"),
        (Action::SavePreset1, "save_preset1"),
        (Action::SavePreset2, "save_preset2"),
        (Action::SavePreset3, "save_preset3"),
        (Action::SavePreset4, "save_preset4"),
        (Action::SavePreset5, "save_preset5"),
        (Action::ClearPreset1, "clear_preset1"),
        (Action::ClearPreset2, "clear_preset2"),
        (Action::ClearPreset3, "clear_preset3"),
        (Action::ClearPreset4, "clear_preset4"),
        (Action::ClearPreset5, "clear_preset5"),
        (Action::ToggleCommandPalette, "toggle_command_palette"),
        (Action::ReplayTour, "replay_tour"),
        (Action::SavePendingToFile, "save_pending_to_file"),
    );
}

#[test]
fn tool_serialization_matches_established_contract() {
    assert_json_names(&[
        (Tool::Select, "select"),
        (Tool::Pen, "pen"),
        (Tool::Line, "line"),
        (Tool::Rect, "rect"),
        (Tool::Ellipse, "ellipse"),
        (Tool::Triangle, "triangle"),
        (Tool::Parallelogram, "parallelogram"),
        (Tool::Rhombus, "rhombus"),
        (Tool::RegularPolygon, "regular-polygon"),
        (Tool::FreeformPolygon, "freeform-polygon"),
        (Tool::Arrow, "arrow"),
        (Tool::Blur, "blur"),
        (Tool::Marker, "marker"),
        (Tool::Highlight, "highlight"),
        (Tool::StepMarker, "step-marker"),
        (Tool::Eraser, "eraser"),
    ]);

    assert_json_names(&[
        (DragTool::Default, "default"),
        (DragTool::Select, "select"),
        (DragTool::Pen, "pen"),
        (DragTool::Line, "line"),
        (DragTool::Rect, "rect"),
        (DragTool::Ellipse, "ellipse"),
        (DragTool::Triangle, "triangle"),
        (DragTool::Parallelogram, "parallelogram"),
        (DragTool::Rhombus, "rhombus"),
        (DragTool::RegularPolygon, "regular-polygon"),
        (DragTool::Arrow, "arrow"),
        (DragTool::Blur, "blur"),
        (DragTool::Marker, "marker"),
        (DragTool::Highlight, "highlight"),
        (DragTool::StepMarker, "step-marker"),
        (DragTool::Eraser, "eraser"),
    ]);

    assert_json_names(&[
        (DragBindableTool::Select, "select"),
        (DragBindableTool::Pen, "pen"),
        (DragBindableTool::Line, "line"),
        (DragBindableTool::Rect, "rect"),
        (DragBindableTool::Ellipse, "ellipse"),
        (DragBindableTool::Triangle, "triangle"),
        (DragBindableTool::Parallelogram, "parallelogram"),
        (DragBindableTool::Rhombus, "rhombus"),
        (DragBindableTool::RegularPolygon, "regular-polygon"),
        (DragBindableTool::Arrow, "arrow"),
        (DragBindableTool::Blur, "blur"),
        (DragBindableTool::Marker, "marker"),
        (DragBindableTool::Highlight, "highlight"),
        (DragBindableTool::StepMarker, "step-marker"),
        (DragBindableTool::Eraser, "eraser"),
    ]);

    assert_json_names(&[(EraserMode::Brush, "brush"), (EraserMode::Stroke, "stroke")]);
}

#[test]
fn color_serialization_matches_established_contract() {
    let color = Color {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 0.4,
    };
    assert_eq!(
        serde_json::to_value(color).expect("serialize color"),
        serde_json::json!({ "r": 0.1, "g": 0.2, "b": 0.3, "a": 0.4 })
    );
    assert_eq!(
        serde_json::from_value::<Color>(serde_json::json!({
            "r": 0.1,
            "g": 0.2,
            "b": 0.3,
            "a": 0.4
        }))
        .expect("deserialize color"),
        color
    );
}

#[test]
fn established_public_paths_reexport_domain_types() {
    let action: crate::config::Action = Action::Exit;
    let _: Action = action;
    let keybinding_action: crate::config::keybindings::Action = Action::Undo;
    let _: Action = keybinding_action;

    let tool: crate::input::Tool = Tool::Pen;
    let _: Tool = tool;
    let nested_tool: crate::input::tool::Tool = Tool::Line;
    let _: Tool = nested_tool;
    let drag_tool: crate::input::tool::DragTool = DragTool::Pen;
    let _: DragTool = drag_tool;
    let drag_bindable: crate::input::tool::DragBindableTool = DragBindableTool::Rect;
    let _: DragBindableTool = drag_bindable;
    let eraser_mode: crate::input::EraserMode = EraserMode::Brush;
    let _: EraserMode = eraser_mode;

    let color: crate::draw::Color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let _: Color = color;
    let nested_color: crate::draw::color::Color = crate::draw::color::RED;
    let _: Color = nested_color;

    let background: crate::input::BoardBackground = BoardBackground::Transparent;
    let _: BoardBackground = background;
    let board: crate::input::BoardSpec = BoardSpec {
        id: "board".to_string(),
        name: "Board".to_string(),
        background: BoardBackground::Transparent,
        default_pen_color: None,
        auto_adjust_pen: false,
        persist: true,
        pinned: false,
    };
    let _: BoardSpec = board;
}

#[test]
fn production_domain_sources_have_no_upward_crate_dependencies() {
    let domain_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/domain");
    let mut checked = 0;

    for entry in fs::read_dir(&domain_dir).expect("read src/domain") {
        let path = entry.expect("read domain entry").path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs")
            || path.file_name().and_then(|name| name.to_str()) == Some("tests.rs")
        {
            continue;
        }

        let source = fs::read_to_string(&path).expect("read domain source");
        assert!(
            !source.contains("crate::"),
            "{} contains an upward crate dependency",
            path.display()
        );
        checked += 1;
    }

    assert_eq!(
        checked, 5,
        "architecture test must cover every domain source"
    );
}
