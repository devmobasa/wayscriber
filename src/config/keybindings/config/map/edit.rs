use super::super::KeybindingsConfig;
use crate::config::Action;

macro_rules! define_action_binding_accessors {
    (
        $( $action:ident => $group:ident.$field:ident, )+
        ;
        unsupported: [$( $unsupported:ident ),+ $(,)?]
    ) => {
        /// One action backed by a persisted `[keybindings]` field.
        ///
        /// The variants and their storage access are generated from the same
        /// declaration, so consumers cannot maintain a second incomplete
        /// action-to-field registry.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum ConfigurableAction {
            $( $action, )+
        }

        impl ConfigurableAction {
            const ALL: &'static [Self] = &[$(Self::$action,)+];
            const ACTIONS: &'static [Action] = &[$(Action::$action,)+];

            /// Every persisted keybinding field in canonical storage order.
            pub const fn all() -> &'static [Self] {
                Self::ALL
            }

            const fn actions() -> &'static [Action] {
                Self::ACTIONS
            }

            /// The runtime action controlled by this field.
            pub const fn action(self) -> Action {
                match self {
                    $(Self::$action => Action::$action,)+
                }
            }

            /// The persisted field for a runtime action, when it has one.
            pub const fn from_action(action: Action) -> Option<Self> {
                match action {
                    $(Action::$action => Some(Self::$action),)+
                    $(Action::$unsupported)|+ => None,
                }
            }

            /// The flattened `[keybindings]` TOML key for this field.
            pub const fn field_key(self) -> &'static str {
                match self {
                    $(Self::$action => stringify!($field),)+
                }
            }

            /// Resolve a flattened `[keybindings]` TOML key.
            pub fn from_field_key(key: &str) -> Option<Self> {
                match key {
                    $(stringify!($field) => Some(Self::$action),)+
                    _ => None,
                }
            }

            /// Read this field's persisted bindings.
            pub fn get(self, config: &KeybindingsConfig) -> &[String] {
                match self {
                    $(Self::$action => config.$group.$field.as_slice(),)+
                }
            }

            /// Replace this field's persisted bindings.
            pub fn set(self, config: &mut KeybindingsConfig, bindings: Vec<String>) {
                let target = match self {
                    $(Self::$action => &mut config.$group.$field,)+
                };
                *target = bindings;
            }

            /// Canonical user-facing label for this action.
            pub fn label(self) -> &'static str {
                crate::config::action_label(self.action())
            }
        }

        impl KeybindingsConfig {
            /// Every action with a persisted `[keybindings]` field, in the
            /// order declared below (the same group order the keymap
            /// traversal uses). Adding a configurable action extends this
            /// list automatically, which is what lets migrations and the
            /// defaults snapshot test see a newcomer without being told.
            pub fn configurable_actions() -> &'static [Action] {
                ConfigurableAction::actions()
            }

            /// Bindings stored for one configurable action. Runtime-only
            /// actions return `None` because they have no persisted field.
            pub fn bindings_for_action(&self, action: Action) -> Option<&[String]> {
                ConfigurableAction::from_action(action).map(|field| field.get(self))
            }

            /// The `[keybindings]` key that stores one action's bindings.
            /// Field names are the TOML keys (the sections are `#[serde(flatten)]`
            /// and nothing is renamed), so this doubles as the config path used
            /// in diagnostics. Runtime-only actions return `None`.
            pub fn config_key_for_action(action: Action) -> Option<&'static str> {
                ConfigurableAction::from_action(action).map(ConfigurableAction::field_key)
            }

            /// Replace every binding for one action. The caller validates the
            /// whole config before committing it, so conflicts cannot persist.
            pub fn set_bindings_for_action(
                &mut self,
                action: Action,
                bindings: Vec<String>,
            ) -> Result<(), String> {
                let Some(field) = ConfigurableAction::from_action(action) else {
                    return Err(format!("{action:?} does not have a configurable keybinding"));
                };
                field.set(self, bindings);
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
    IncreasePenSmoothing => tools.increase_pen_smoothing,
    DecreasePenSmoothing => tools.decrease_pen_smoothing,
    CycleFontFamily => tools.cycle_font_family,
    OpenFontPicker => tools.open_font_picker,
    CycleBlurStyle => tools.cycle_blur_style,
    CycleArrowStyle => tools.cycle_arrow_style,
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
    SelectSpotlightTool => tools.select_spotlight_tool,
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
    ToggleFloatingBadge => ui.toggle_floating_badge,
    ToggleZoomChip => ui.toggle_zoom_chip,
    ToggleFocusMode => ui.toggle_focus_mode,
    ToggleClickHighlight => ui.toggle_click_highlight,
    ToggleInputHud => ui.toggle_input_hud,
    ToggleToolbar => ui.toggle_toolbar,
    CycleToolbarDisplay => ui.cycle_toolbar_display,
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
    OpenAbout => ui.open_about,
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
    CaptureRegionInteractive => capture.capture_region_interactive,
    MeasureMode => capture.measure_mode,
    ExportCanvasFile => capture.export_canvas_file,
    ExportCanvasClipboard => capture.export_canvas_clipboard,
    ExportCanvasClipboardAndFile => capture.export_canvas_clipboard_and_file,
    ExportBoardPdfFile => capture.export_board_pdf_file,
    ExportAllBoardsPdfFile => capture.export_all_boards_pdf_file,
    OpenCaptureFolder => capture.open_capture_folder,
    CopyTextFromScreen => capture.copy_text_from_screen,
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
        OpenConfiguratorKeybindings,
        OpenConfiguratorPresets,
        OpenConfiguratorBoards,
        OpenConfiguratorQuickColors,
        OpenConfiguratorOnboardingHints,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configurable_action_identity_and_storage_share_one_registry() {
        let config = KeybindingsConfig::default();

        assert_eq!(
            ConfigurableAction::all().len(),
            KeybindingsConfig::configurable_actions().len()
        );
        for &field in ConfigurableAction::all() {
            let action = field.action();
            assert_eq!(ConfigurableAction::from_action(action), Some(field));
            assert_eq!(
                ConfigurableAction::from_field_key(field.field_key()),
                Some(field)
            );
            assert_eq!(
                KeybindingsConfig::config_key_for_action(action),
                Some(field.field_key())
            );
            assert_eq!(config.bindings_for_action(action), Some(field.get(&config)));
        }
    }

    #[test]
    fn configurable_action_replaces_and_unbinds_its_own_storage() {
        let mut config = KeybindingsConfig::default();
        let field = ConfigurableAction::SelectPenTool;

        field.set(&mut config, vec!["Ctrl+P".into()]);
        assert_eq!(field.get(&config), ["Ctrl+P"]);

        field.set(&mut config, Vec::new());
        assert!(field.get(&config).is_empty());
    }

    #[test]
    fn configurable_action_uses_canonical_action_labels() {
        assert_eq!(
            ConfigurableAction::CaptureRegionInteractive.label(),
            crate::config::action_label(Action::CaptureRegionInteractive)
        );
    }

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
