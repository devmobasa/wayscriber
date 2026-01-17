use super::super::draft::ConfigDraft;
use super::super::parse::{parse_field, parse_u64_field};
use crate::models::error::FormError;
use wayscriber::config::Config;

impl ConfigDraft {
    pub(super) fn apply_ui(&self, config: &mut Config, errors: &mut Vec<FormError>) {
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
        config.ui.toolbar.show_zoom_actions = self.ui_toolbar_show_zoom_actions;
        config.ui.toolbar.show_pages_section = self.ui_toolbar_show_pages_section;
        config.ui.toolbar.show_boards_section = self.ui_toolbar_show_boards_section;
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
            errors,
            |value| config.ui.toolbar.top_offset = value,
        );
        parse_field(
            &self.ui_toolbar_top_offset_y,
            "ui.toolbar.top_offset_y",
            errors,
            |value| config.ui.toolbar.top_offset_y = value,
        );
        parse_field(
            &self.ui_toolbar_side_offset,
            "ui.toolbar.side_offset",
            errors,
            |value| config.ui.toolbar.side_offset = value,
        );
        parse_field(
            &self.ui_toolbar_side_offset_x,
            "ui.toolbar.side_offset_x",
            errors,
            |value| config.ui.toolbar.side_offset_x = value,
        );
        config.ui.status_bar_position = self.ui_status_position.to_status_position();
        parse_field(
            &self.status_font_size,
            "ui.status_bar_style.font_size",
            errors,
            |value| config.ui.status_bar_style.font_size = value,
        );
        parse_field(
            &self.status_padding,
            "ui.status_bar_style.padding",
            errors,
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
            errors,
            |value| config.ui.status_bar_style.dot_radius = value,
        );

        config.ui.click_highlight.enabled = self.click_highlight_enabled;
        config.ui.click_highlight.use_pen_color = self.click_highlight_use_pen_color;
        parse_field(
            &self.click_highlight_radius,
            "ui.click_highlight.radius",
            errors,
            |value| config.ui.click_highlight.radius = value,
        );
        parse_field(
            &self.click_highlight_outline_thickness,
            "ui.click_highlight.outline_thickness",
            errors,
            |value| config.ui.click_highlight.outline_thickness = value,
        );
        parse_u64_field(
            &self.click_highlight_duration_ms,
            "ui.click_highlight.duration_ms",
            errors,
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
            errors,
            |value| config.ui.help_overlay_style.font_size = value,
        );
        parse_field(
            &self.help_line_height,
            "ui.help_overlay_style.line_height",
            errors,
            |value| config.ui.help_overlay_style.line_height = value,
        );
        parse_field(
            &self.help_padding,
            "ui.help_overlay_style.padding",
            errors,
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
            errors,
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
    }
}
