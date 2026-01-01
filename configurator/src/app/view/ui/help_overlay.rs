use iced::Element;
use iced::widget::{column, row, scrollable, text};

use crate::app::state::ConfiguratorApp;
use crate::messages::Message;
use crate::models::{QuadField, TextField, ToggleField};

use super::super::widgets::{color_quad_editor, labeled_input, toggle_row};

impl ConfiguratorApp {
    pub(super) fn ui_help_overlay_tab(&self) -> Element<'_, Message> {
        let column = column![
            text("Help Overlay Style").size(18),
            toggle_row(
                "Filter sections by enabled features",
                self.draft.help_context_filter,
                self.defaults.help_context_filter,
                ToggleField::UiHelpOverlayContextFilter,
            ),
            color_quad_editor(
                "Background RGBA (0-1)",
                &self.draft.help_bg_color,
                &self.defaults.help_bg_color,
                QuadField::HelpBg,
            ),
            color_quad_editor(
                "Border RGBA (0-1)",
                &self.draft.help_border_color,
                &self.defaults.help_border_color,
                QuadField::HelpBorder,
            ),
            color_quad_editor(
                "Text RGBA (0-1)",
                &self.draft.help_text_color,
                &self.defaults.help_text_color,
                QuadField::HelpText,
            ),
            labeled_input(
                "Font family",
                &self.draft.help_font_family,
                &self.defaults.help_font_family,
                TextField::HelpFontFamily,
            ),
            row![
                labeled_input(
                    "Font size",
                    &self.draft.help_font_size,
                    &self.defaults.help_font_size,
                    TextField::HelpFontSize,
                ),
                labeled_input(
                    "Line height",
                    &self.draft.help_line_height,
                    &self.defaults.help_line_height,
                    TextField::HelpLineHeight,
                ),
                labeled_input(
                    "Padding",
                    &self.draft.help_padding,
                    &self.defaults.help_padding,
                    TextField::HelpPadding,
                ),
                labeled_input(
                    "Border width",
                    &self.draft.help_border_width,
                    &self.defaults.help_border_width,
                    TextField::HelpBorderWidth,
                )
            ]
            .spacing(12),
        ]
        .spacing(12);

        scrollable(column).into()
    }
}
