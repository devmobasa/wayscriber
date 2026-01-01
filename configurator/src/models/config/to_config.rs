use super::super::error::FormError;
use super::draft::ConfigDraft;
use super::parse::{
    parse_field, parse_optional_usize_field, parse_u32_field, parse_u64_field, parse_usize_field,
};
use wayscriber::config::Config;

impl ConfigDraft {
    pub fn to_config(&self, base: &Config) -> Result<Config, Vec<FormError>> {
        let mut errors = Vec::new();
        let mut config = base.clone();

        match self.drawing_color.to_color_spec() {
            Ok(color) => config.drawing.default_color = color,
            Err(err) => errors.push(err),
        }
        parse_field(
            &self.drawing_default_thickness,
            "drawing.default_thickness",
            &mut errors,
            |value| config.drawing.default_thickness = value,
        );
        parse_field(
            &self.drawing_default_eraser_size,
            "drawing.default_eraser_size",
            &mut errors,
            |value| config.drawing.default_eraser_size = value,
        );
        config.drawing.default_eraser_mode = self.drawing_default_eraser_mode.to_mode();
        parse_field(
            &self.drawing_default_font_size,
            "drawing.default_font_size",
            &mut errors,
            |value| config.drawing.default_font_size = value,
        );
        parse_field(
            &self.drawing_marker_opacity,
            "drawing.marker_opacity",
            &mut errors,
            |value| config.drawing.marker_opacity = value,
        );
        config.drawing.font_family = self.drawing_font_family.clone();
        config.drawing.font_weight = self.drawing_font_weight.clone();
        config.drawing.font_style = self.drawing_font_style.clone();
        config.drawing.text_background_enabled = self.drawing_text_background_enabled;
        config.drawing.default_fill_enabled = self.drawing_default_fill_enabled;
        parse_field(
            &self.drawing_hit_test_tolerance,
            "drawing.hit_test_tolerance",
            &mut errors,
            |value| config.drawing.hit_test_tolerance = value,
        );
        parse_usize_field(
            &self.drawing_hit_test_linear_threshold,
            "drawing.hit_test_linear_threshold",
            &mut errors,
            |value| config.drawing.hit_test_linear_threshold = value,
        );
        parse_usize_field(
            &self.drawing_undo_stack_limit,
            "drawing.undo_stack_limit",
            &mut errors,
            |value| config.drawing.undo_stack_limit = value,
        );

        parse_field(&self.arrow_length, "arrow.length", &mut errors, |value| {
            config.arrow.length = value
        });
        parse_field(
            &self.arrow_angle,
            "arrow.angle_degrees",
            &mut errors,
            |value| config.arrow.angle_degrees = value,
        );
        config.arrow.head_at_end = self.arrow_head_at_end;

        parse_u64_field(
            &self.history_undo_all_delay_ms,
            "history.undo_all_delay_ms",
            &mut errors,
            |value| config.history.undo_all_delay_ms = value,
        );
        parse_u64_field(
            &self.history_redo_all_delay_ms,
            "history.redo_all_delay_ms",
            &mut errors,
            |value| config.history.redo_all_delay_ms = value,
        );
        config.history.custom_section_enabled = self.history_custom_section_enabled;
        parse_u64_field(
            &self.history_custom_undo_delay_ms,
            "history.custom_undo_delay_ms",
            &mut errors,
            |value| config.history.custom_undo_delay_ms = value,
        );
        parse_u64_field(
            &self.history_custom_redo_delay_ms,
            "history.custom_redo_delay_ms",
            &mut errors,
            |value| config.history.custom_redo_delay_ms = value,
        );
        parse_usize_field(
            &self.history_custom_undo_steps,
            "history.custom_undo_steps",
            &mut errors,
            |value| config.history.custom_undo_steps = value,
        );
        parse_usize_field(
            &self.history_custom_redo_steps,
            "history.custom_redo_steps",
            &mut errors,
            |value| config.history.custom_redo_steps = value,
        );

        config.performance.buffer_count = self.performance_buffer_count;
        config.performance.enable_vsync = self.performance_enable_vsync;
        parse_u32_field(
            &self.performance_ui_animation_fps,
            "performance.ui_animation_fps",
            &mut errors,
            |value| config.performance.ui_animation_fps = value,
        );

        config.ui.show_status_bar = self.ui_show_status_bar;
        config.ui.show_frozen_badge = self.ui_show_frozen_badge;
        config.ui.context_menu.enabled = self.ui_context_menu_enabled;
        let preferred_output = self.ui_preferred_output.trim();
        config.ui.preferred_output = if preferred_output.is_empty() {
            None
        } else {
            Some(preferred_output.to_string())
        };
        config.ui.xdg_fullscreen = self.ui_xdg_fullscreen;
        config.ui.toolbar.top_pinned = self.ui_toolbar_top_pinned;
        config.ui.toolbar.side_pinned = self.ui_toolbar_side_pinned;
        config.ui.toolbar.use_icons = self.ui_toolbar_use_icons;
        config.ui.toolbar.show_more_colors = self.ui_toolbar_show_more_colors;
        config.ui.toolbar.show_preset_toasts = self.ui_toolbar_show_preset_toasts;
        config.ui.toolbar.layout_mode = self.ui_toolbar_layout_mode.to_mode();
        config.ui.toolbar.mode_overrides = self.ui_toolbar_mode_overrides.to_config();
        config.ui.toolbar.show_presets = self.ui_toolbar_show_presets;
        config.ui.toolbar.show_actions_section = self.ui_toolbar_show_actions_section;
        config.ui.toolbar.show_actions_advanced = self.ui_toolbar_show_actions_advanced;
        config.ui.toolbar.show_pages_section = self.ui_toolbar_show_pages_section;
        config.ui.toolbar.show_step_section = self.ui_toolbar_show_step_section;
        config.ui.toolbar.show_text_controls = self.ui_toolbar_show_text_controls;
        config.ui.toolbar.show_settings_section = self.ui_toolbar_show_settings_section;
        config.ui.toolbar.show_delay_sliders = self.ui_toolbar_show_delay_sliders;
        config.ui.toolbar.show_marker_opacity_section = self.ui_toolbar_show_marker_opacity_section;
        config.ui.toolbar.show_tool_preview = self.ui_toolbar_show_tool_preview;
        config.ui.toolbar.force_inline = self.ui_toolbar_force_inline;
        parse_field(
            &self.ui_toolbar_top_offset,
            "ui.toolbar.top_offset",
            &mut errors,
            |value| config.ui.toolbar.top_offset = value,
        );
        parse_field(
            &self.ui_toolbar_top_offset_y,
            "ui.toolbar.top_offset_y",
            &mut errors,
            |value| config.ui.toolbar.top_offset_y = value,
        );
        parse_field(
            &self.ui_toolbar_side_offset,
            "ui.toolbar.side_offset",
            &mut errors,
            |value| config.ui.toolbar.side_offset = value,
        );
        parse_field(
            &self.ui_toolbar_side_offset_x,
            "ui.toolbar.side_offset_x",
            &mut errors,
            |value| config.ui.toolbar.side_offset_x = value,
        );
        config.ui.status_bar_position = self.ui_status_position.to_status_position();
        parse_field(
            &self.status_font_size,
            "ui.status_bar_style.font_size",
            &mut errors,
            |value| config.ui.status_bar_style.font_size = value,
        );
        parse_field(
            &self.status_padding,
            "ui.status_bar_style.padding",
            &mut errors,
            |value| config.ui.status_bar_style.padding = value,
        );
        match self
            .status_bar_bg_color
            .to_array("ui.status_bar_style.bg_color")
        {
            Ok(values) => config.ui.status_bar_style.bg_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .status_bar_text_color
            .to_array("ui.status_bar_style.text_color")
        {
            Ok(values) => config.ui.status_bar_style.text_color = values,
            Err(err) => errors.push(err),
        }
        parse_field(
            &self.status_dot_radius,
            "ui.status_bar_style.dot_radius",
            &mut errors,
            |value| config.ui.status_bar_style.dot_radius = value,
        );

        config.ui.click_highlight.enabled = self.click_highlight_enabled;
        config.ui.click_highlight.use_pen_color = self.click_highlight_use_pen_color;
        parse_field(
            &self.click_highlight_radius,
            "ui.click_highlight.radius",
            &mut errors,
            |value| config.ui.click_highlight.radius = value,
        );
        parse_field(
            &self.click_highlight_outline_thickness,
            "ui.click_highlight.outline_thickness",
            &mut errors,
            |value| config.ui.click_highlight.outline_thickness = value,
        );
        parse_u64_field(
            &self.click_highlight_duration_ms,
            "ui.click_highlight.duration_ms",
            &mut errors,
            |value| config.ui.click_highlight.duration_ms = value,
        );
        match self
            .click_highlight_fill_color
            .to_array("ui.click_highlight.fill_color")
        {
            Ok(values) => config.ui.click_highlight.fill_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .click_highlight_outline_color
            .to_array("ui.click_highlight.outline_color")
        {
            Ok(values) => config.ui.click_highlight.outline_color = values,
            Err(err) => errors.push(err),
        }

        config.ui.help_overlay_style.font_family = self.help_font_family.trim().to_string();
        parse_field(
            &self.help_font_size,
            "ui.help_overlay_style.font_size",
            &mut errors,
            |value| config.ui.help_overlay_style.font_size = value,
        );
        parse_field(
            &self.help_line_height,
            "ui.help_overlay_style.line_height",
            &mut errors,
            |value| config.ui.help_overlay_style.line_height = value,
        );
        parse_field(
            &self.help_padding,
            "ui.help_overlay_style.padding",
            &mut errors,
            |value| config.ui.help_overlay_style.padding = value,
        );
        match self
            .help_bg_color
            .to_array("ui.help_overlay_style.bg_color")
        {
            Ok(values) => config.ui.help_overlay_style.bg_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .help_border_color
            .to_array("ui.help_overlay_style.border_color")
        {
            Ok(values) => config.ui.help_overlay_style.border_color = values,
            Err(err) => errors.push(err),
        }
        parse_field(
            &self.help_border_width,
            "ui.help_overlay_style.border_width",
            &mut errors,
            |value| config.ui.help_overlay_style.border_width = value,
        );
        match self
            .help_text_color
            .to_array("ui.help_overlay_style.text_color")
        {
            Ok(values) => config.ui.help_overlay_style.text_color = values,
            Err(err) => errors.push(err),
        }
        config.ui.help_overlay_context_filter = self.help_context_filter;

        config.board.enabled = self.board_enabled;
        config.board.default_mode = self.board_default_mode.as_str().to_string();
        match self
            .board_whiteboard_color
            .to_array("board.whiteboard_color")
        {
            Ok(values) => config.board.whiteboard_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .board_blackboard_color
            .to_array("board.blackboard_color")
        {
            Ok(values) => config.board.blackboard_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .board_whiteboard_pen
            .to_array("board.whiteboard_pen_color")
        {
            Ok(values) => config.board.whiteboard_pen_color = values,
            Err(err) => errors.push(err),
        }
        match self
            .board_blackboard_pen
            .to_array("board.blackboard_pen_color")
        {
            Ok(values) => config.board.blackboard_pen_color = values,
            Err(err) => errors.push(err),
        }
        config.board.auto_adjust_pen = self.board_auto_adjust_pen;

        config.capture.enabled = self.capture_enabled;
        config.capture.save_directory = self.capture_save_directory.clone();
        config.capture.filename_template = self.capture_filename_template.clone();
        config.capture.format = self.capture_format.clone();
        config.capture.copy_to_clipboard = self.capture_copy_to_clipboard;
        config.capture.exit_after_capture = self.capture_exit_after;

        config.session.persist_transparent = self.session_persist_transparent;
        config.session.persist_whiteboard = self.session_persist_whiteboard;
        config.session.persist_blackboard = self.session_persist_blackboard;
        config.session.persist_history = self.session_persist_history;
        config.session.restore_tool_state = self.session_restore_tool_state;
        config.session.per_output = self.session_per_output;
        config.session.storage = self.session_storage_mode.to_mode();
        let custom_dir = self.session_custom_directory.trim();
        config.session.custom_directory = if custom_dir.is_empty() {
            None
        } else {
            Some(custom_dir.to_string())
        };
        parse_usize_field(
            &self.session_max_shapes_per_frame,
            "session.max_shapes_per_frame",
            &mut errors,
            |value| config.session.max_shapes_per_frame = value,
        );
        parse_u64_field(
            &self.session_max_file_size_mb,
            "session.max_file_size_mb",
            &mut errors,
            |value| config.session.max_file_size_mb = value,
        );
        config.session.compress = self.session_compression.to_compression();
        parse_u64_field(
            &self.session_auto_compress_threshold_kb,
            "session.auto_compress_threshold_kb",
            &mut errors,
            |value| config.session.auto_compress_threshold_kb = value,
        );
        parse_optional_usize_field(
            &self.session_max_persisted_undo_depth,
            "session.max_persisted_undo_depth",
            &mut errors,
            |value| config.session.max_persisted_undo_depth = value,
        );
        parse_usize_field(
            &self.session_backup_retention,
            "session.backup_retention",
            &mut errors,
            |value| config.session.backup_retention = value,
        );

        #[cfg(feature = "tablet-input")]
        {
            config.tablet.enabled = self.tablet_enabled;
            config.tablet.pressure_enabled = self.tablet_pressure_enabled;
            parse_field(
                &self.tablet_min_thickness,
                "tablet.min_thickness",
                &mut errors,
                |value| config.tablet.min_thickness = value,
            );
            parse_field(
                &self.tablet_max_thickness,
                "tablet.max_thickness",
                &mut errors,
                |value| config.tablet.max_thickness = value,
            );
        }

        config.presets = self.presets.to_config(&mut errors);

        match self.keybindings.to_config() {
            Ok(cfg) => config.keybindings = cfg,
            Err(errs) => errors.extend(errs),
        }

        if errors.is_empty() {
            Ok(config)
        } else {
            Err(errors)
        }
    }
}
