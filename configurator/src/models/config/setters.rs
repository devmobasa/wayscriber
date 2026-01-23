use super::super::fields::{
    FontStyleOption, FontWeightOption, OverrideOption, QuadField, TextField, ToggleField,
    ToolbarLayoutModeOption, ToolbarOverrideField, TripletField,
};
use super::draft::ConfigDraft;

impl ConfigDraft {
    pub fn apply_toolbar_layout_mode(&mut self, mode: ToolbarLayoutModeOption) {
        self.ui_toolbar_layout_mode = mode;
        let defaults = mode.to_mode().section_defaults();
        self.ui_toolbar_show_actions_section = defaults.show_actions_section;
        self.ui_toolbar_show_actions_advanced = defaults.show_actions_advanced;
        self.ui_toolbar_show_zoom_actions = defaults.show_zoom_actions;
        self.ui_toolbar_show_pages_section = defaults.show_pages_section;
        self.ui_toolbar_show_boards_section = defaults.show_boards_section;
        self.ui_toolbar_show_presets = defaults.show_presets;
        self.ui_toolbar_show_step_section = defaults.show_step_section;
        self.ui_toolbar_show_text_controls = defaults.show_text_controls;
        self.ui_toolbar_show_settings_section = defaults.show_settings_section;
    }

    pub fn set_toolbar_override(
        &mut self,
        mode: ToolbarLayoutModeOption,
        field: ToolbarOverrideField,
        value: OverrideOption,
    ) {
        self.ui_toolbar_mode_overrides
            .for_mode_mut(mode)
            .set(field, value);
    }

    pub fn set_toggle(&mut self, field: ToggleField, value: bool) {
        match field {
            ToggleField::DrawingTextBackground => {
                self.drawing_text_background_enabled = value;
            }
            ToggleField::DrawingFillEnabled => {
                self.drawing_default_fill_enabled = value;
            }
            ToggleField::PerformanceVsync => self.performance_enable_vsync = value,
            ToggleField::UiShowStatusBar => self.ui_show_status_bar = value,
            ToggleField::UiShowFrozenBadge => self.ui_show_frozen_badge = value,
            ToggleField::UiShowStatusBoardBadge => self.ui_show_status_board_badge = value,
            ToggleField::UiShowStatusPageBadge => self.ui_show_status_page_badge = value,
            ToggleField::UiShowPageBadgeWithStatusBar => {
                self.ui_show_page_badge_with_status_bar = value;
            }
            ToggleField::UiHelpOverlayContextFilter => self.help_context_filter = value,
            ToggleField::UiContextMenuEnabled => self.ui_context_menu_enabled = value,
            ToggleField::UiXdgFullscreen => self.ui_xdg_fullscreen = value,
            ToggleField::UiToolbarTopPinned => self.ui_toolbar_top_pinned = value,
            ToggleField::UiToolbarSidePinned => self.ui_toolbar_side_pinned = value,
            ToggleField::UiToolbarUseIcons => self.ui_toolbar_use_icons = value,
            ToggleField::UiToolbarShowMoreColors => self.ui_toolbar_show_more_colors = value,
            ToggleField::UiToolbarPresetToasts => self.ui_toolbar_show_preset_toasts = value,
            ToggleField::UiToolbarShowPresets => self.ui_toolbar_show_presets = value,
            ToggleField::UiToolbarShowActionsSection => {
                self.ui_toolbar_show_actions_section = value;
            }
            ToggleField::UiToolbarShowActionsAdvanced => {
                self.ui_toolbar_show_actions_advanced = value;
            }
            ToggleField::UiToolbarShowZoomActions => {
                self.ui_toolbar_show_zoom_actions = value;
            }
            ToggleField::UiToolbarShowPagesSection => {
                self.ui_toolbar_show_pages_section = value;
            }
            ToggleField::UiToolbarShowBoardsSection => {
                self.ui_toolbar_show_boards_section = value;
            }
            ToggleField::UiToolbarShowStepSection => self.ui_toolbar_show_step_section = value,
            ToggleField::UiToolbarShowTextControls => self.ui_toolbar_show_text_controls = value,
            ToggleField::UiToolbarShowSettingsSection => {
                self.ui_toolbar_show_settings_section = value;
            }
            ToggleField::UiToolbarShowDelaySliders => {
                self.ui_toolbar_show_delay_sliders = value;
            }
            ToggleField::UiToolbarShowMarkerOpacitySection => {
                self.ui_toolbar_show_marker_opacity_section = value;
            }
            ToggleField::UiToolbarShowToolPreview => {
                self.ui_toolbar_show_tool_preview = value;
            }
            ToggleField::UiToolbarForceInline => {
                self.ui_toolbar_force_inline = value;
            }
            ToggleField::UiClickHighlightEnabled => self.click_highlight_enabled = value,
            ToggleField::UiClickHighlightUsePenColor => self.click_highlight_use_pen_color = value,
            ToggleField::PresenterHideStatusBar => self.presenter_hide_status_bar = value,
            ToggleField::PresenterHideToolbars => self.presenter_hide_toolbars = value,
            ToggleField::PresenterHideToolPreview => self.presenter_hide_tool_preview = value,
            ToggleField::PresenterCloseHelpOverlay => self.presenter_close_help_overlay = value,
            ToggleField::PresenterEnableClickHighlight => {
                self.presenter_enable_click_highlight = value;
            }
            ToggleField::PresenterShowToast => self.presenter_show_toast = value,
            ToggleField::BoardsAutoCreate => self.boards.auto_create = value,
            ToggleField::BoardsShowBadge => self.boards.show_board_badge = value,
            ToggleField::BoardsPersistCustomizations => {
                self.boards.persist_customizations = value;
            }
            ToggleField::CaptureEnabled => self.capture_enabled = value,
            ToggleField::CaptureCopyToClipboard => self.capture_copy_to_clipboard = value,
            ToggleField::CaptureExitAfter => self.capture_exit_after = value,
            ToggleField::SessionPersistTransparent => {
                self.session_persist_transparent = value;
            }
            ToggleField::SessionPersistWhiteboard => {
                self.session_persist_whiteboard = value;
            }
            ToggleField::SessionPersistBlackboard => {
                self.session_persist_blackboard = value;
            }
            ToggleField::SessionPersistHistory => {
                self.session_persist_history = value;
            }
            ToggleField::SessionRestoreToolState => {
                self.session_restore_tool_state = value;
            }
            ToggleField::SessionPerOutput => {
                self.session_per_output = value;
            }
            ToggleField::SessionAutosaveEnabled => {
                self.session_autosave_enabled = value;
            }
            ToggleField::HistoryCustomSectionEnabled => {
                self.history_custom_section_enabled = value;
            }
            ToggleField::ArrowHeadAtEnd => {
                self.arrow_head_at_end = value;
            }
            #[cfg(feature = "tablet-input")]
            ToggleField::TabletEnabled => self.tablet_enabled = value,
            #[cfg(feature = "tablet-input")]
            ToggleField::TabletPressureEnabled => self.tablet_pressure_enabled = value,
            #[cfg(feature = "tablet-input")]
            ToggleField::TabletAutoEraserSwitch => self.tablet_auto_eraser_switch = value,
        }
    }

    pub fn set_text(&mut self, field: TextField, value: String) {
        match field {
            TextField::DrawingColorName => {
                self.drawing_color.name = value;
                self.drawing_color.update_named_from_current();
            }
            TextField::DrawingThickness => self.drawing_default_thickness = value,
            TextField::DrawingEraserSize => self.drawing_default_eraser_size = value,
            TextField::DrawingFontSize => self.drawing_default_font_size = value,
            TextField::DrawingMarkerOpacity => self.drawing_marker_opacity = value,
            TextField::DrawingFontFamily => self.drawing_font_family = value,
            TextField::DrawingFontWeight => {
                self.drawing_font_weight = value;
                self.drawing_font_weight_option = FontWeightOption::Custom;
            }
            TextField::DrawingFontStyle => {
                self.drawing_font_style = value;
                self.drawing_font_style_option = FontStyleOption::Custom;
            }
            TextField::DrawingHitTestTolerance => self.drawing_hit_test_tolerance = value,
            TextField::DrawingHitTestThreshold => self.drawing_hit_test_linear_threshold = value,
            TextField::DrawingUndoStackLimit => self.drawing_undo_stack_limit = value,
            TextField::ArrowLength => self.arrow_length = value,
            TextField::ArrowAngle => self.arrow_angle = value,
            TextField::PerformanceMaxFpsNoVsync => self.performance_max_fps_no_vsync = value,
            TextField::PerformanceUiAnimationFps => self.performance_ui_animation_fps = value,
            TextField::HistoryUndoAllDelayMs => self.history_undo_all_delay_ms = value,
            TextField::HistoryRedoAllDelayMs => self.history_redo_all_delay_ms = value,
            TextField::HistoryCustomUndoDelayMs => self.history_custom_undo_delay_ms = value,
            TextField::HistoryCustomRedoDelayMs => self.history_custom_redo_delay_ms = value,
            TextField::HistoryCustomUndoSteps => self.history_custom_undo_steps = value,
            TextField::HistoryCustomRedoSteps => self.history_custom_redo_steps = value,
            TextField::UiPreferredOutput => self.ui_preferred_output = value,
            TextField::StatusFontSize => self.status_font_size = value,
            TextField::StatusPadding => self.status_padding = value,
            TextField::StatusDotRadius => self.status_dot_radius = value,
            TextField::HighlightRadius => self.click_highlight_radius = value,
            TextField::HighlightOutlineThickness => self.click_highlight_outline_thickness = value,
            TextField::HighlightDurationMs => self.click_highlight_duration_ms = value,
            TextField::HelpFontFamily => self.help_font_family = value,
            TextField::HelpFontSize => self.help_font_size = value,
            TextField::HelpLineHeight => self.help_line_height = value,
            TextField::HelpPadding => self.help_padding = value,
            TextField::HelpBorderWidth => self.help_border_width = value,
            TextField::CaptureSaveDirectory => self.capture_save_directory = value,
            TextField::CaptureFilename => self.capture_filename_template = value,
            TextField::CaptureFormat => self.capture_format = value,
            TextField::ToolbarTopOffset => self.ui_toolbar_top_offset = value,
            TextField::ToolbarTopOffsetY => self.ui_toolbar_top_offset_y = value,
            TextField::ToolbarSideOffset => self.ui_toolbar_side_offset = value,
            TextField::ToolbarSideOffsetX => self.ui_toolbar_side_offset_x = value,
            TextField::BoardsMaxCount => self.boards.max_count = value,
            TextField::SessionCustomDirectory => self.session_custom_directory = value,
            TextField::SessionMaxShapesPerFrame => self.session_max_shapes_per_frame = value,
            TextField::SessionMaxFileSizeMb => self.session_max_file_size_mb = value,
            TextField::SessionAutoCompressThresholdKb => {
                self.session_auto_compress_threshold_kb = value
            }
            TextField::SessionMaxPersistedUndoDepth => {
                self.session_max_persisted_undo_depth = value
            }
            TextField::SessionBackupRetention => self.session_backup_retention = value,
            TextField::SessionAutosaveIdleMs => self.session_autosave_idle_ms = value,
            TextField::SessionAutosaveIntervalMs => self.session_autosave_interval_ms = value,
            TextField::SessionAutosaveFailureBackoffMs => {
                self.session_autosave_failure_backoff_ms = value
            }
            #[cfg(feature = "tablet-input")]
            TextField::TabletMinThickness => self.tablet_min_thickness = value,
            #[cfg(feature = "tablet-input")]
            TextField::TabletMaxThickness => self.tablet_max_thickness = value,
            #[cfg(feature = "tablet-input")]
            TextField::TabletPressureVariationThreshold => {
                self.tablet_pressure_variation_threshold = value
            }
            #[cfg(feature = "tablet-input")]
            TextField::TabletPressureScaleStep => self.tablet_pressure_thickness_scale_step = value,
        }
    }

    pub fn set_triplet(&mut self, field: TripletField, index: usize, value: String) {
        match field {
            TripletField::DrawingColorRgb => {
                if let Some(slot) = self.drawing_color.rgb.get_mut(index) {
                    *slot = value;
                }
            }
        }
    }

    pub fn set_quad(&mut self, field: QuadField, index: usize, value: String) {
        match field {
            QuadField::StatusBarBg => self.status_bar_bg_color.set_component(index, value),
            QuadField::StatusBarText => self.status_bar_text_color.set_component(index, value),
            QuadField::HelpBg => self.help_bg_color.set_component(index, value),
            QuadField::HelpBorder => self.help_border_color.set_component(index, value),
            QuadField::HelpText => self.help_text_color.set_component(index, value),
            QuadField::HighlightFill => self.click_highlight_fill_color.set_component(index, value),
            QuadField::HighlightOutline => self
                .click_highlight_outline_color
                .set_component(index, value),
        }
    }
}
