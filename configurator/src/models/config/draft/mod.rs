mod from_config;

use super::super::color::{ColorInput, ColorQuadInput, ColorTripletInput};
use super::super::fields::{
    BoardModeOption, EraserModeOption, FontStyleOption, FontWeightOption, SessionCompressionOption,
    SessionStorageModeOption, StatusPositionOption, ToolbarLayoutModeOption,
};
use super::super::keybindings::KeybindingsDraft;
use super::presets::PresetsDraft;
use super::toolbar_overrides::ToolbarModeOverridesDraft;

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
    pub ui_toolbar_show_pages_section: bool,
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
