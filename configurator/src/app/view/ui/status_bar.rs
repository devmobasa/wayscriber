use iced::widget::{column, pick_list, row, scrollable, text};
use iced::{Element, Length};

use crate::app::state::ConfiguratorApp;
use crate::messages::Message;
use crate::models::{QuadField, StatusPositionOption, TextField, ToggleField};

use super::super::widgets::{color_quad_editor, labeled_control, labeled_input, toggle_row};

impl ConfiguratorApp {
    pub(super) fn ui_status_bar_tab(&self) -> Element<'_, Message> {
        let status_position = pick_list(
            StatusPositionOption::list(),
            Some(self.draft.ui_status_position),
            Message::StatusPositionChanged,
        );

        let column = column![
            text("Status Bar").size(18),
            toggle_row(
                "Show status bar",
                self.draft.ui_show_status_bar,
                self.defaults.ui_show_status_bar,
                ToggleField::UiShowStatusBar,
            ),
            toggle_row(
                "Show board label",
                self.draft.ui_show_status_board_badge,
                self.defaults.ui_show_status_board_badge,
                ToggleField::UiShowStatusBoardBadge,
            ),
            toggle_row(
                "Show page counter",
                self.draft.ui_show_status_page_badge,
                self.defaults.ui_show_status_page_badge,
                ToggleField::UiShowStatusPageBadge,
            ),
            toggle_row(
                "Show overlay badge with status bar",
                self.draft.ui_show_page_badge_with_status_bar,
                self.defaults.ui_show_page_badge_with_status_bar,
                ToggleField::UiShowPageBadgeWithStatusBar,
            ),
            toggle_row(
                "Show frozen badge",
                self.draft.ui_show_frozen_badge,
                self.defaults.ui_show_frozen_badge,
                ToggleField::UiShowFrozenBadge,
            ),
            labeled_control(
                "Status bar position",
                status_position.width(Length::Fill).into(),
                self.defaults.ui_status_position.label().to_string(),
                self.draft.ui_status_position != self.defaults.ui_status_position,
            ),
            text("Status Bar Style").size(18),
            color_quad_editor(
                "Background RGBA (0-1)",
                &self.draft.status_bar_bg_color,
                &self.defaults.status_bar_bg_color,
                QuadField::StatusBarBg,
            ),
            color_quad_editor(
                "Text RGBA (0-1)",
                &self.draft.status_bar_text_color,
                &self.defaults.status_bar_text_color,
                QuadField::StatusBarText,
            ),
            row![
                labeled_input(
                    "Font size",
                    &self.draft.status_font_size,
                    &self.defaults.status_font_size,
                    TextField::StatusFontSize,
                ),
                labeled_input(
                    "Padding",
                    &self.draft.status_padding,
                    &self.defaults.status_padding,
                    TextField::StatusPadding,
                ),
                labeled_input(
                    "Dot radius",
                    &self.draft.status_dot_radius,
                    &self.defaults.status_dot_radius,
                    TextField::StatusDotRadius,
                )
            ]
            .spacing(12),
        ]
        .spacing(12);

        scrollable(column).into()
    }
}
