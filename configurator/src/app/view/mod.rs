mod arrow;
mod boards;
mod capture;
mod drawing;
mod history;
mod keybindings;
mod performance;
mod presets;
mod session;
#[cfg(feature = "tablet-input")]
mod tablet;
mod ui;
mod widgets;

use iced::theme;
use iced::widget::{Column, Row, Space, button, column, container, horizontal_rule, row, text};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::TabId;

use self::widgets::{default_label_color, feedback_text};
use super::state::{ConfiguratorApp, StatusMessage};

impl ConfiguratorApp {
    pub(super) fn view(&self) -> Element<'_, Message> {
        let header = self.header_view();
        let content = self.tab_view();
        let footer = self.footer_view();

        column![header, content, footer]
            .spacing(12)
            .padding(16)
            .into()
    }

    fn header_view(&self) -> Element<'_, Message> {
        let reload_button = button("Reload")
            .style(theme::Button::Secondary)
            .on_press(Message::ReloadRequested);

        let defaults_button = button("Defaults")
            .style(theme::Button::Secondary)
            .on_press(Message::ResetToDefaults);

        let save_button = button("Save")
            .style(theme::Button::Primary)
            .on_press(Message::SaveRequested);

        let mut toolbar = Row::new()
            .spacing(12)
            .align_items(iced::Alignment::Center)
            .push(reload_button)
            .push(defaults_button)
            .push(save_button);

        toolbar = if self.is_saving {
            toolbar.push(text("Saving...").size(16))
        } else if self.is_loading {
            toolbar.push(text("Loading...").size(16))
        } else if self.is_dirty {
            toolbar.push(
                text("Unsaved changes")
                    .style(theme::Text::Color(iced::Color::from_rgb(0.95, 0.72, 0.2))),
            )
        } else {
            toolbar.push(
                text("All changes saved")
                    .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.8, 0.6))),
            )
        };

        let banner: Element<'_, Message> = match &self.status {
            StatusMessage::Idle => Space::new(Length::Shrink, Length::Shrink).into(),
            StatusMessage::Info(message) => container(text(message))
                .padding(8)
                .style(theme::Container::Box)
                .into(),
            StatusMessage::Success(message) => container(
                text(message).style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.9, 0.6))),
            )
            .padding(8)
            .style(theme::Container::Box)
            .into(),
            StatusMessage::Error(message) => container(
                text(message).style(theme::Text::Color(iced::Color::from_rgb(1.0, 0.5, 0.5))),
            )
            .padding(8)
            .style(theme::Container::Box)
            .into(),
        };

        column![toolbar, banner].spacing(8).into()
    }

    fn tab_view(&self) -> Element<'_, Message> {
        let tab_bar = TabId::ALL.iter().fold(
            Row::new().spacing(8).align_items(iced::Alignment::Center),
            |row, tab| {
                let label = tab.title();
                let button = button(label)
                    .padding([6, 12])
                    .style(if *tab == self.active_tab {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
                    })
                    .on_press(Message::TabSelected(*tab));
                row.push(button)
            },
        );

        let content: Element<'_, Message> = match self.active_tab {
            TabId::Drawing => self.drawing_tab(),
            TabId::Presets => self.presets_tab(),
            TabId::Arrow => self.arrow_tab(),
            TabId::History => self.history_tab(),
            TabId::Performance => self.performance_tab(),
            TabId::Ui => self.ui_tab(),
            TabId::Boards => self.boards_tab(),
            TabId::Capture => self.capture_tab(),
            TabId::Session => self.session_tab(),
            TabId::Keybindings => self.keybindings_tab(),
            #[cfg(feature = "tablet-input")]
            TabId::Tablet => self.tablet_tab(),
        };

        let legend = self.defaults_legend();

        column![tab_bar, horizontal_rule(2), legend, content]
            .spacing(12)
            .into()
    }

    fn footer_view(&self) -> Element<'_, Message> {
        let mut info = Column::new().spacing(4);

        if let Some(path) = &self.config_path {
            info = info.push(text(format!("Config path: {}", path.display())).size(14));
        }
        if let Some(path) = &self.last_backup_path {
            info = info.push(text(format!("Last backup: {}", path.display())).size(14));
        }

        info.into()
    }

    fn defaults_legend(&self) -> Element<'_, Message> {
        let legend = row![
            text("Default labels:").size(12),
            text("blue = matches")
                .size(12)
                .style(theme::Text::Color(default_label_color(false))),
            text("yellow = changed")
                .size(12)
                .style(theme::Text::Color(default_label_color(true))),
        ]
        .spacing(12)
        .align_items(iced::Alignment::Center);

        let hint = feedback_text(
            "Tip: use Tab/Shift+Tab to move between fields; Enter activates buttons.",
            false,
        );

        column![legend, hint].spacing(4).into()
    }
}
