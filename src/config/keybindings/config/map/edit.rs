use super::super::KeybindingsConfig;
use crate::config::Action;

macro_rules! define_action_binding_accessors {
    (
        $( $action:ident => $group:ident.$field:ident, )+
        ;
        unsupported: [$( $unsupported:ident ),+ $(,)?]
    ) => {
        impl KeybindingsConfig {
            /// Bindings stored for one configurable action. Runtime-only
            /// actions return `None` because they have no persisted field.
            pub fn bindings_for_action(&self, action: Action) -> Option<&[String]> {
                match action {
                    $(Action::$action => Some(self.$group.$field.as_slice()),)+
                    $(Action::$unsupported)|+ => None,
                }
            }

            /// Replace every binding for one action. The caller validates the
            /// whole config before committing it, so conflicts cannot persist.
            pub fn set_bindings_for_action(
                &mut self,
                action: Action,
                bindings: Vec<String>,
            ) -> Result<(), String> {
                let target = match action {
                    $(Action::$action => &mut self.$group.$field,)+
                    $(Action::$unsupported)|+ => {
                        return Err(format!(
                            "{action:?} does not have a configurable keybinding"
                        ));
                    }
                };
                *target = bindings;
                Ok(())
            }
        }
    };
}

// This is the single action-to-storage contract used by the overlay editor.
// Adding a configurable Action is an exhaustive-match compile error here.
define_action_binding_accessors! {
    Exit => core.exit,
    EnterTextMode => core.enter_text_mode,
    EnterStickyNoteMode => core.enter_sticky_note_mode,
    ClearCanvas => core.clear_canvas,
    Undo => core.undo,
    Redo => core.redo,
    UndoAll => core.undo_all,
    RedoAll => core.redo_all,
    UndoAllDelayed => core.undo_all_delayed,
    RedoAllDelayed => core.redo_all_delayed,
    DuplicateSelection => selection.duplicate_selection,
    CopySelection => selection.copy_selection,
    PasteSelection => selection.paste_selection,
    SelectAll => selection.select_all,
    MoveSelectionToFront => selection.move_selection_to_front,
    MoveSelectionToBack => selection.move_selection_to_back,
    NudgeSelectionUp => selection.nudge_selection_up,
    NudgeSelectionDown => selection.nudge_selection_down,
    NudgeSelectionLeft => selection.nudge_selection_left,
    NudgeSelectionRight => selection.nudge_selection_right,
    NudgeSelectionUpLarge => selection.nudge_selection_up_large,
    NudgeSelectionDownLarge => selection.nudge_selection_down_large,
    MoveSelectionToStart => selection.move_selection_to_start,
    MoveSelectionToEnd => selection.move_selection_to_end,
    MoveSelectionToTop => selection.move_selection_to_top,
    MoveSelectionToBottom => selection.move_selection_to_bottom,
    DeleteSelection => selection.delete_selection,
    IncreaseThickness => tools.increase_thickness,
    DecreaseThickness => tools.decrease_thickness,
    IncreaseMarkerOpacity => tools.increase_marker_opacity,
    DecreaseMarkerOpacity => tools.decrease_marker_opacity,
    SelectSelectionTool => tools.select_selection_tool,
    SelectMarkerTool => tools.select_marker_tool,
    SelectStepMarkerTool => tools.select_step_marker_tool,
    SelectEraserTool => tools.select_eraser_tool,
    ToggleEraserMode => tools.toggle_eraser_mode,
    SelectPenTool => tools.select_pen_tool,
    SelectLineTool => tools.select_line_tool,
    SelectRectTool => tools.select_rect_tool,
    SelectEllipseTool => tools.select_ellipse_tool,
    SelectTriangleTool => tools.select_triangle_tool,
    SelectParallelogramTool => tools.select_parallelogram_tool,
    SelectRhombusTool => tools.select_rhombus_tool,
    SelectRegularPolygonTool => tools.select_regular_polygon_tool,
    SelectFreeformPolygonTool => tools.select_freeform_polygon_tool,
    SelectArrowTool => tools.select_arrow_tool,
    SelectBlurTool => tools.select_blur_tool,
    SelectHighlightTool => tools.select_highlight_tool,
    ToggleHighlightTool => tools.toggle_highlight_tool,
    IncreaseFontSize => tools.increase_font_size,
    DecreaseFontSize => tools.decrease_font_size,
    ResetArrowLabelCounter => tools.reset_arrow_labels,
    ResetStepMarkerCounter => tools.reset_step_markers,
    ToggleWhiteboard => board.toggle_whiteboard,
    ToggleBlackboard => board.toggle_blackboard,
    ReturnToTransparent => board.return_to_transparent,
    Board1 => board.board_1,
    Board2 => board.board_2,
    Board3 => board.board_3,
    Board4 => board.board_4,
    Board5 => board.board_5,
    Board6 => board.board_6,
    Board7 => board.board_7,
    Board8 => board.board_8,
    Board9 => board.board_9,
    BoardNext => board.board_next,
    BoardPrev => board.board_prev,
    BoardNew => board.board_new,
    BoardDelete => board.board_delete,
    BoardPicker => board.board_picker,
    BoardDuplicate => board.board_duplicate,
    FocusNextOutput => board.focus_next_output,
    FocusPrevOutput => board.focus_prev_output,
    PagePrev => board.page_prev,
    PageNext => board.page_next,
    PageNew => board.page_new,
    PageDuplicate => board.page_duplicate,
    PageDelete => board.page_delete,
    ToggleHelp => ui.toggle_help,
    ToggleQuickHelp => ui.toggle_quick_help,
    ToggleStatusBar => ui.toggle_status_bar,
    ToggleClickHighlight => ui.toggle_click_highlight,
    ToggleToolbar => ui.toggle_toolbar,
    TogglePresenterMode => ui.toggle_presenter_mode,
    ToggleLightMode => ui.toggle_light_mode,
    ToggleLightModeDrawing => ui.toggle_light_mode_drawing,
    RenderProfileNext => ui.render_profile_next,
    RenderProfilePrevious => ui.render_profile_previous,
    RenderProfileOff => ui.render_profile_off,
    ToggleFill => ui.toggle_fill,
    ToggleRadialMenu => ui.toggle_radial_menu,
    ToggleSelectionProperties => ui.toggle_selection_properties,
    OpenContextMenu => ui.open_context_menu,
    OpenConfigurator => ui.open_configurator,
    ToggleCommandPalette => ui.toggle_command_palette,
    SetColorRed => colors.set_color_red,
    SetColorGreen => colors.set_color_green,
    SetColorBlue => colors.set_color_blue,
    SetColorYellow => colors.set_color_yellow,
    SetColorOrange => colors.set_color_orange,
    SetColorPink => colors.set_color_pink,
    SetColorWhite => colors.set_color_white,
    SetColorBlack => colors.set_color_black,
    PickScreenColor => colors.pick_screen_color,
    CaptureFullScreen => capture.capture_full_screen,
    CaptureActiveWindow => capture.capture_active_window,
    CaptureSelection => capture.capture_selection,
    CaptureClipboardFull => capture.capture_clipboard_full,
    CaptureFileFull => capture.capture_file_full,
    CaptureClipboardSelection => capture.capture_clipboard_selection,
    CaptureFileSelection => capture.capture_file_selection,
    CaptureClipboardRegion => capture.capture_clipboard_region,
    CaptureFileRegion => capture.capture_file_region,
    ExportCanvasFile => capture.export_canvas_file,
    ExportCanvasClipboard => capture.export_canvas_clipboard,
    ExportCanvasClipboardAndFile => capture.export_canvas_clipboard_and_file,
    ExportBoardPdfFile => capture.export_board_pdf_file,
    ExportAllBoardsPdfFile => capture.export_all_boards_pdf_file,
    OpenCaptureFolder => capture.open_capture_folder,
    ToggleFrozenMode => zoom.toggle_frozen_mode,
    ZoomIn => zoom.zoom_in,
    ZoomOut => zoom.zoom_out,
    ResetZoom => zoom.reset_zoom,
    ToggleZoomLock => zoom.toggle_zoom_lock,
    RefreshZoomCapture => zoom.refresh_zoom_capture,
    ApplyPreset1 => presets.apply_preset_1,
    ApplyPreset2 => presets.apply_preset_2,
    ApplyPreset3 => presets.apply_preset_3,
    ApplyPreset4 => presets.apply_preset_4,
    ApplyPreset5 => presets.apply_preset_5,
    SavePreset1 => presets.save_preset_1,
    SavePreset2 => presets.save_preset_2,
    SavePreset3 => presets.save_preset_3,
    SavePreset4 => presets.save_preset_4,
    SavePreset5 => presets.save_preset_5,
    ClearPreset1 => presets.clear_preset_1,
    ClearPreset2 => presets.clear_preset_2,
    ClearPreset3 => presets.clear_preset_3,
    ClearPreset4 => presets.clear_preset_4,
    ClearPreset5 => presets.clear_preset_5,
    ; unsupported: [
        BoardRestoreDeleted,
        BoardSwitchRecent,
        PageRestoreDeleted,
        ClearSavedToolState,
        ReplayTour,
        SavePendingToFile,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_action_access_reads_replaces_and_unbinds() {
        let mut config = KeybindingsConfig::default();
        assert_eq!(
            config.bindings_for_action(Action::SelectPenTool),
            Some(&["F".to_string()][..])
        );

        config
            .set_bindings_for_action(Action::SelectPenTool, vec!["Ctrl+P".into()])
            .unwrap();
        assert_eq!(
            config.bindings_for_action(Action::SelectPenTool),
            Some(&["Ctrl+P".to_string()][..])
        );

        config
            .set_bindings_for_action(Action::SelectPenTool, Vec::new())
            .unwrap();
        assert_eq!(
            config.bindings_for_action(Action::SelectPenTool),
            Some(&[][..])
        );
    }

    #[test]
    fn runtime_only_actions_are_not_reported_as_configurable() {
        let mut config = KeybindingsConfig::default();
        assert_eq!(config.bindings_for_action(Action::ReplayTour), None);
        assert!(
            config
                .set_bindings_for_action(Action::ReplayTour, vec!["R".into()])
                .is_err()
        );
    }

    #[test]
    fn edited_bindings_still_use_whole_map_conflict_validation() {
        let mut candidate = KeybindingsConfig::default();
        candidate
            .set_bindings_for_action(Action::SelectPenTool, vec!["Ctrl+Z".into()])
            .unwrap();

        assert!(candidate.build_action_map().is_err());
    }
}
