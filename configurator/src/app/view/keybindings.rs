use iced::alignment::Horizontal;
use iced::theme;
use iced::widget::{Column, Row, button, column, container, row, scrollable, text, text_input};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::KeybindingsTabId;

use super::super::state::ConfiguratorApp;
use super::widgets::{LABEL_COLUMN_WIDTH, default_value_text};

impl ConfiguratorApp {
    pub(super) fn keybindings_tab(&self) -> Element<'_, Message> {
        let tab_bar = KeybindingsTabId::ALL.iter().fold(
            Row::new().spacing(8).align_items(iced::Alignment::Center),
            |row, tab| {
                let label = tab.title();
                let button = button(label)
                    .padding([6, 12])
                    .style(if *tab == self.active_keybindings_tab {
                        theme::Button::Primary
                    } else {
                        theme::Button::Secondary
                    })
                    .on_press(Message::KeybindingsTabSelected(*tab));
                row.push(button)
            },
        );

        let mut column = Column::new()
            .spacing(8)
            .push(text("Keybindings (comma-separated)").size(20))
            .push(tab_bar);

        for entry in self
            .draft
            .keybindings
            .entries
            .iter()
            .filter(|entry| entry.field.tab() == self.active_keybindings_tab)
        {
            let default_value = self
                .defaults
                .keybindings
                .value_for(entry.field)
                .unwrap_or("");
            let changed = entry.value.trim() != default_value.trim();
            column = column.push(
                row![
                    container(text(entry.field.label()).size(16))
                        .width(Length::Fixed(LABEL_COLUMN_WIDTH))
                        .align_x(Horizontal::Right),
                    column![
                        text_input("Shortcut list", &entry.value)
                            .on_input({
                                let field = entry.field;
                                move |value| Message::KeybindingChanged(field, value)
                            })
                            .width(Length::Fill),
                        row![default_value_text(default_value.to_string(), changed)]
                            .align_items(iced::Alignment::Center)
                    ]
                    .spacing(4)
                    .width(Length::Fill)
                ]
                .spacing(12)
                .align_items(iced::Alignment::Center),
            );
        }

        scrollable(column).into()
    }
}
