use super::super::{Config, float::finite_or_default};

impl Config {
    /// These styling fields have no load-time finite bounds. Preserve authored
    /// finite values, but keep NaN/infinity out of layout and Cairo consumers.
    pub(super) fn validate_ui_styles(&mut self) {
        let defaults = crate::config::UiConfig::default();
        for (field, value, fallback) in [
            (
                "ui.toolbar.top_offset",
                &mut self.ui.toolbar.top_offset,
                defaults.toolbar.top_offset,
            ),
            (
                "ui.toolbar.top_offset_y",
                &mut self.ui.toolbar.top_offset_y,
                defaults.toolbar.top_offset_y,
            ),
            (
                "ui.status_bar_style.font_size",
                &mut self.ui.status_bar_style.font_size,
                defaults.status_bar_style.font_size,
            ),
            (
                "ui.status_bar_style.padding",
                &mut self.ui.status_bar_style.padding,
                defaults.status_bar_style.padding,
            ),
            (
                "ui.status_bar_style.dot_radius",
                &mut self.ui.status_bar_style.dot_radius,
                defaults.status_bar_style.dot_radius,
            ),
            (
                "ui.help_overlay_style.font_size",
                &mut self.ui.help_overlay_style.font_size,
                defaults.help_overlay_style.font_size,
            ),
            (
                "ui.help_overlay_style.line_height",
                &mut self.ui.help_overlay_style.line_height,
                defaults.help_overlay_style.line_height,
            ),
            (
                "ui.help_overlay_style.padding",
                &mut self.ui.help_overlay_style.padding,
                defaults.help_overlay_style.padding,
            ),
            (
                "ui.help_overlay_style.border_width",
                &mut self.ui.help_overlay_style.border_width,
                defaults.help_overlay_style.border_width,
            ),
        ] {
            *value = finite_or_default(*value, fallback, field);
        }

        for (field, color, fallback) in [
            (
                "ui.status_bar_style.bg_color",
                &mut self.ui.status_bar_style.bg_color,
                defaults.status_bar_style.bg_color,
            ),
            (
                "ui.status_bar_style.text_color",
                &mut self.ui.status_bar_style.text_color,
                defaults.status_bar_style.text_color,
            ),
            (
                "ui.help_overlay_style.bg_color",
                &mut self.ui.help_overlay_style.bg_color,
                defaults.help_overlay_style.bg_color,
            ),
            (
                "ui.help_overlay_style.border_color",
                &mut self.ui.help_overlay_style.border_color,
                defaults.help_overlay_style.border_color,
            ),
            (
                "ui.help_overlay_style.text_color",
                &mut self.ui.help_overlay_style.text_color,
                defaults.help_overlay_style.text_color,
            ),
        ] {
            for (index, (component, fallback)) in color.iter_mut().zip(fallback).enumerate() {
                *component = finite_or_default(*component, fallback, &format!("{field}[{index}]"));
            }
        }
    }
}
