use super::super::color::{ColorInput, ColorQuadInput, ColorTripletInput};
use super::super::fields::{
    BoardModeOption, EraserModeOption, FontStyleOption, FontWeightOption, SessionCompressionOption,
    SessionStorageModeOption, StatusPositionOption, ToolbarLayoutModeOption,
};
use super::super::keybindings::KeybindingsDraft;
use super::super::util::format_float;
use super::presets::PresetsDraft;
use super::toolbar_overrides::ToolbarModeOverridesDraft;
use wayscriber::config::Config;

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigDraft {
    pub drawing_color: ColorInput,
    pub drawing_default_thickness: String,
    pub drawing_default_eraser_size: String,
    pub drawing_default_eraser_mode: EraserModeOption,
    pub drawing_default_font_size: String,
    pub drawing_marker_opacity: String,
    pub drawing_hit_test_tolerance: String,
    pub drawing_hit_test_linear_threshold: String,
    pub drawing_undo_stack_limit: String,
    pub drawing_font_family: String,
    pub drawing_font_weight: String,
    pub drawing_font_style: String,
    pub drawing_text_background_enabled: bool,
    pub drawing_default_fill_enabled: bool,
    pub drawing_font_style_option: FontStyleOption,
    pub drawing_font_weight_option: FontWeightOption,

    pub arrow_length: String,
    pub arrow_angle: String,
    pub arrow_head_at_end: bool,

    pub history_undo_all_delay_ms: String,
    pub history_redo_all_delay_ms: String,
    pub history_custom_section_enabled: bool,
    pub history_custom_undo_delay_ms: String,
    pub history_custom_redo_delay_ms: String,
    pub history_custom_undo_steps: String,
    pub history_custom_redo_steps: String,

    pub performance_buffer_count: u32,
    pub performance_enable_vsync: bool,
    pub performance_ui_animation_fps: String,

    pub ui_show_status_bar: bool,
    pub ui_show_frozen_badge: bool,
    pub ui_context_menu_enabled: bool,
    pub ui_preferred_output: String,
    pub ui_xdg_fullscreen: bool,
    pub ui_toolbar_top_pinned: bool,
    pub ui_toolbar_side_pinned: bool,
    pub ui_toolbar_use_icons: bool,
    pub ui_toolbar_show_more_colors: bool,
    pub ui_toolbar_show_preset_toasts: bool,
    pub ui_toolbar_layout_mode: ToolbarLayoutModeOption,
    pub ui_toolbar_show_presets: bool,
    pub ui_toolbar_show_actions_section: bool,
    pub ui_toolbar_show_actions_advanced: bool,
    pub ui_toolbar_show_step_section: bool,
    pub ui_toolbar_show_text_controls: bool,
    pub ui_toolbar_show_settings_section: bool,
    pub ui_toolbar_show_delay_sliders: bool,
    pub ui_toolbar_show_marker_opacity_section: bool,
    pub ui_toolbar_show_tool_preview: bool,
    pub ui_toolbar_force_inline: bool,
    pub ui_toolbar_top_offset: String,
    pub ui_toolbar_top_offset_y: String,
    pub ui_toolbar_side_offset: String,
    pub ui_toolbar_side_offset_x: String,
    pub ui_toolbar_mode_overrides: ToolbarModeOverridesDraft,
    pub ui_status_position: StatusPositionOption,
    pub status_font_size: String,
    pub status_padding: String,
    pub status_bar_bg_color: ColorQuadInput,
    pub status_bar_text_color: ColorQuadInput,
    pub status_dot_radius: String,

    pub click_highlight_enabled: bool,
    pub click_highlight_use_pen_color: bool,
    pub click_highlight_radius: String,
    pub click_highlight_outline_thickness: String,
    pub click_highlight_duration_ms: String,
    pub click_highlight_fill_color: ColorQuadInput,
    pub click_highlight_outline_color: ColorQuadInput,

    pub help_font_family: String,
    pub help_font_size: String,
    pub help_line_height: String,
    pub help_padding: String,
    pub help_bg_color: ColorQuadInput,
    pub help_border_color: ColorQuadInput,
    pub help_border_width: String,
    pub help_text_color: ColorQuadInput,
    pub help_context_filter: bool,

    pub board_enabled: bool,
    pub board_default_mode: BoardModeOption,
    pub board_whiteboard_color: ColorTripletInput,
    pub board_blackboard_color: ColorTripletInput,
    pub board_whiteboard_pen: ColorTripletInput,
    pub board_blackboard_pen: ColorTripletInput,
    pub board_auto_adjust_pen: bool,

    pub capture_enabled: bool,
    pub capture_save_directory: String,
    pub capture_filename_template: String,
    pub capture_format: String,
    pub capture_copy_to_clipboard: bool,
    pub capture_exit_after: bool,

    pub session_persist_transparent: bool,
    pub session_persist_whiteboard: bool,
    pub session_persist_blackboard: bool,
    pub session_persist_history: bool,
    pub session_restore_tool_state: bool,
    pub session_per_output: bool,
    pub session_storage_mode: SessionStorageModeOption,
    pub session_custom_directory: String,
    pub session_max_shapes_per_frame: String,
    pub session_max_file_size_mb: String,
    pub session_compression: SessionCompressionOption,
    pub session_auto_compress_threshold_kb: String,
    pub session_max_persisted_undo_depth: String,
    pub session_backup_retention: String,

    #[cfg(feature = "tablet-input")]
    pub tablet_enabled: bool,
    #[cfg(feature = "tablet-input")]
    pub tablet_pressure_enabled: bool,
    #[cfg(feature = "tablet-input")]
    pub tablet_min_thickness: String,
    #[cfg(feature = "tablet-input")]
    pub tablet_max_thickness: String,

    pub presets: PresetsDraft,

    pub keybindings: KeybindingsDraft,
}

impl ConfigDraft {
    pub fn from_config(config: &Config) -> Self {
        let (style_option, style_value) = FontStyleOption::from_value(&config.drawing.font_style);
        let (weight_option, weight_value) =
            FontWeightOption::from_value(&config.drawing.font_weight);
        Self {
            drawing_color: ColorInput::from_color(&config.drawing.default_color),
            drawing_default_thickness: format_float(config.drawing.default_thickness),
            drawing_default_eraser_size: format_float(config.drawing.default_eraser_size),
            drawing_default_eraser_mode: EraserModeOption::from_mode(
                config.drawing.default_eraser_mode,
            ),
            drawing_default_font_size: format_float(config.drawing.default_font_size),
            drawing_marker_opacity: format_float(config.drawing.marker_opacity),
            drawing_hit_test_tolerance: format_float(config.drawing.hit_test_tolerance),
            drawing_hit_test_linear_threshold: config.drawing.hit_test_linear_threshold.to_string(),
            drawing_undo_stack_limit: config.drawing.undo_stack_limit.to_string(),
            drawing_font_family: config.drawing.font_family.clone(),
            drawing_font_weight: weight_value,
            drawing_font_style: style_value,
            drawing_text_background_enabled: config.drawing.text_background_enabled,
            drawing_default_fill_enabled: config.drawing.default_fill_enabled,
            drawing_font_style_option: style_option,
            drawing_font_weight_option: weight_option,

            arrow_length: format_float(config.arrow.length),
            arrow_angle: format_float(config.arrow.angle_degrees),
            arrow_head_at_end: config.arrow.head_at_end,

            history_undo_all_delay_ms: config.history.undo_all_delay_ms.to_string(),
            history_redo_all_delay_ms: config.history.redo_all_delay_ms.to_string(),
            history_custom_section_enabled: config.history.custom_section_enabled,
            history_custom_undo_delay_ms: config.history.custom_undo_delay_ms.to_string(),
            history_custom_redo_delay_ms: config.history.custom_redo_delay_ms.to_string(),
            history_custom_undo_steps: config.history.custom_undo_steps.to_string(),
            history_custom_redo_steps: config.history.custom_redo_steps.to_string(),

            performance_buffer_count: config.performance.buffer_count,
            performance_enable_vsync: config.performance.enable_vsync,
            performance_ui_animation_fps: config.performance.ui_animation_fps.to_string(),

            ui_show_status_bar: config.ui.show_status_bar,
            ui_show_frozen_badge: config.ui.show_frozen_badge,
            ui_context_menu_enabled: config.ui.context_menu.enabled,
            ui_preferred_output: config.ui.preferred_output.clone().unwrap_or_default(),
            ui_xdg_fullscreen: config.ui.xdg_fullscreen,
            ui_toolbar_top_pinned: config.ui.toolbar.top_pinned,
            ui_toolbar_side_pinned: config.ui.toolbar.side_pinned,
            ui_toolbar_use_icons: config.ui.toolbar.use_icons,
            ui_toolbar_show_more_colors: config.ui.toolbar.show_more_colors,
            ui_toolbar_show_preset_toasts: config.ui.toolbar.show_preset_toasts,
            ui_toolbar_layout_mode: ToolbarLayoutModeOption::from_mode(
                config.ui.toolbar.layout_mode,
            ),
            ui_toolbar_show_presets: config.ui.toolbar.show_presets,
            ui_toolbar_show_actions_section: config.ui.toolbar.show_actions_section,
            ui_toolbar_show_actions_advanced: config.ui.toolbar.show_actions_advanced,
            ui_toolbar_show_step_section: config.ui.toolbar.show_step_section,
            ui_toolbar_show_text_controls: config.ui.toolbar.show_text_controls,
            ui_toolbar_show_settings_section: config.ui.toolbar.show_settings_section,
            ui_toolbar_show_delay_sliders: config.ui.toolbar.show_delay_sliders,
            ui_toolbar_show_marker_opacity_section: config.ui.toolbar.show_marker_opacity_section,
            ui_toolbar_show_tool_preview: config.ui.toolbar.show_tool_preview,
            ui_toolbar_force_inline: config.ui.toolbar.force_inline,
            ui_toolbar_top_offset: format_float(config.ui.toolbar.top_offset),
            ui_toolbar_top_offset_y: format_float(config.ui.toolbar.top_offset_y),
            ui_toolbar_side_offset: format_float(config.ui.toolbar.side_offset),
            ui_toolbar_side_offset_x: format_float(config.ui.toolbar.side_offset_x),
            ui_toolbar_mode_overrides: ToolbarModeOverridesDraft::from_config(
                &config.ui.toolbar.mode_overrides,
            ),
            ui_status_position: StatusPositionOption::from_status_position(
                config.ui.status_bar_position,
            ),
            status_font_size: format_float(config.ui.status_bar_style.font_size),
            status_padding: format_float(config.ui.status_bar_style.padding),
            status_bar_bg_color: ColorQuadInput::from(config.ui.status_bar_style.bg_color),
            status_bar_text_color: ColorQuadInput::from(config.ui.status_bar_style.text_color),
            status_dot_radius: format_float(config.ui.status_bar_style.dot_radius),

            click_highlight_enabled: config.ui.click_highlight.enabled,
            click_highlight_use_pen_color: config.ui.click_highlight.use_pen_color,
            click_highlight_radius: format_float(config.ui.click_highlight.radius),
            click_highlight_outline_thickness: format_float(
                config.ui.click_highlight.outline_thickness,
            ),
            click_highlight_duration_ms: config.ui.click_highlight.duration_ms.to_string(),
            click_highlight_fill_color: ColorQuadInput::from(config.ui.click_highlight.fill_color),
            click_highlight_outline_color: ColorQuadInput::from(
                config.ui.click_highlight.outline_color,
            ),

            help_font_family: config.ui.help_overlay_style.font_family.clone(),
            help_font_size: format_float(config.ui.help_overlay_style.font_size),
            help_line_height: format_float(config.ui.help_overlay_style.line_height),
            help_padding: format_float(config.ui.help_overlay_style.padding),
            help_bg_color: ColorQuadInput::from(config.ui.help_overlay_style.bg_color),
            help_border_color: ColorQuadInput::from(config.ui.help_overlay_style.border_color),
            help_border_width: format_float(config.ui.help_overlay_style.border_width),
            help_text_color: ColorQuadInput::from(config.ui.help_overlay_style.text_color),
            help_context_filter: config.ui.help_overlay_context_filter,

            board_enabled: config.board.enabled,
            board_default_mode: BoardModeOption::from_str(&config.board.default_mode)
                .unwrap_or(BoardModeOption::Transparent),
            board_whiteboard_color: ColorTripletInput::from(config.board.whiteboard_color),
            board_blackboard_color: ColorTripletInput::from(config.board.blackboard_color),
            board_whiteboard_pen: ColorTripletInput::from(config.board.whiteboard_pen_color),
            board_blackboard_pen: ColorTripletInput::from(config.board.blackboard_pen_color),
            board_auto_adjust_pen: config.board.auto_adjust_pen,

            capture_enabled: config.capture.enabled,
            capture_save_directory: config.capture.save_directory.clone(),
            capture_filename_template: config.capture.filename_template.clone(),
            capture_format: config.capture.format.clone(),
            capture_copy_to_clipboard: config.capture.copy_to_clipboard,
            capture_exit_after: config.capture.exit_after_capture,

            session_persist_transparent: config.session.persist_transparent,
            session_persist_whiteboard: config.session.persist_whiteboard,
            session_persist_blackboard: config.session.persist_blackboard,
            session_persist_history: config.session.persist_history,
            session_restore_tool_state: config.session.restore_tool_state,
            session_per_output: config.session.per_output,
            session_storage_mode: SessionStorageModeOption::from_mode(
                config.session.storage.clone(),
            ),
            session_custom_directory: config.session.custom_directory.clone().unwrap_or_default(),
            session_max_shapes_per_frame: config.session.max_shapes_per_frame.to_string(),
            session_max_file_size_mb: config.session.max_file_size_mb.to_string(),
            session_compression: SessionCompressionOption::from_compression(
                config.session.compress.clone(),
            ),
            session_auto_compress_threshold_kb: config
                .session
                .auto_compress_threshold_kb
                .to_string(),
            session_max_persisted_undo_depth: config
                .session
                .max_persisted_undo_depth
                .map(|value| value.to_string())
                .unwrap_or_default(),
            session_backup_retention: config.session.backup_retention.to_string(),

            #[cfg(feature = "tablet-input")]
            tablet_enabled: config.tablet.enabled,
            #[cfg(feature = "tablet-input")]
            tablet_pressure_enabled: config.tablet.pressure_enabled,
            #[cfg(feature = "tablet-input")]
            tablet_min_thickness: format_float(config.tablet.min_thickness),
            #[cfg(feature = "tablet-input")]
            tablet_max_thickness: format_float(config.tablet.max_thickness),

            presets: PresetsDraft::from_config(config),

            keybindings: KeybindingsDraft::from_config(&config.keybindings),
        }
    }
}
