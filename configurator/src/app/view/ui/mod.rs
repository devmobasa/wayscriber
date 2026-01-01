mod click_highlight;
mod help_overlay;
mod status_bar;
mod toolbar;

use iced::Element;
use iced::theme;
use iced::widget::{Row, button, column, text};

use crate::app::state::ConfiguratorApp;
use crate::messages::Message;
use crate::models::{TextField, ToggleField, UiTabId};

use super::widgets::{labeled_input, toggle_row};

impl ConfiguratorApp {
    pub(super) fn ui_tab(&self) -> Element<'_, Message> {
        let tab_bar = UiTabId::ALL.iter().fold(
            Row::new().spacing(8).align_items(iced::Alignment::Center),
            |row, tab| {
                let label = tab.title();
                let button = button(label)
                    .padding([4, 10])
                    .style(if *tab == self.active_ui_tab {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
                    })
                    .on_press(Message::UiTabSelected(*tab));
                row.push(button)
            },
        );

        let content = match self.active_ui_tab {
            UiTabId::Toolbar => self.ui_toolbar_tab(),
            UiTabId::StatusBar => self.ui_status_bar_tab(),
            UiTabId::HelpOverlay => self.ui_help_overlay_tab(),
            UiTabId::ClickHighlight => self.ui_click_highlight_tab(),
        };

        let general = column![
            text("General UI").size(18),
            labeled_input(
                "Preferred output (GNOME fallback)",
                &self.draft.ui_preferred_output,
                &self.defaults.ui_preferred_output,
                TextField::UiPreferredOutput,
            ),
            text("Used for the GNOME xdg-shell fallback overlay.")
                .size(12)
                .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.6, 0.6))),
            toggle_row(
                "Use fullscreen xdg fallback",
                self.draft.ui_xdg_fullscreen,
                self.defaults.ui_xdg_fullscreen,
                ToggleField::UiXdgFullscreen,
            ),
            toggle_row(
                "Enable context menu",
                self.draft.ui_context_menu_enabled,
                self.defaults.ui_context_menu_enabled,
                ToggleField::UiContextMenuEnabled,
            )
        ]
        .spacing(12);

        column![text("UI Settings").size(20), general, tab_bar, content]
            .spacing(12)
            .into()
    }
}
