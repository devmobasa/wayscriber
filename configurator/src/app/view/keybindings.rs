use crate::app::view::theme;
use iced::alignment::Horizontal;
use iced::widget::{Column, Row, button, column, container, row, scrollable, text, text_input};
use iced::{Element, Length};

use crate::app::scroll::CONTENT_SCROLL_ID;
use crate::messages::Message;
use crate::models::KeybindingsTabId;

use super::super::search::TabSearchSummary;
use super::super::state::ConfiguratorApp;
use super::widgets::{LABEL_COLUMN_WIDTH, default_value_text};

impl ConfiguratorApp {
    pub(super) fn keybindings_tab(
        &self,
        search: Option<&TabSearchSummary>,
    ) -> Element<'_, Message> {
        let tabs = visible_keybinding_tabs(search);
        let active_tab = active_keybinding_tab(search, self.active_keybindings_tab);
        let tab_bar = tabs.iter().fold(
            Row::new().spacing(8).align_y(iced::Alignment::Center),
            |row, tab| {
                let label = tab.title();
                let button = button(label)
                    .padding([6, 12])
                    .style(if Some(*tab) == active_tab {
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
            .push(text("Keybindings (comma-separated)").size(20));

        if !tabs.is_empty() {
            column = column.push(tab_bar);
        }

        for entry in self
            .draft
            .keybindings
            .entries
            .iter()
            .filter(|entry| keybinding_row_visible(search, active_tab, entry.field))
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
                            .align_y(iced::Alignment::Center)
                    ]
                    .spacing(4)
                    .width(Length::Fill)
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center),
            );
        }

        scrollable(column).id(CONTENT_SCROLL_ID).into()
    }
}

fn visible_keybinding_tabs(search: Option<&TabSearchSummary>) -> Vec<KeybindingsTabId> {
    match search {
        Some(summary) if !summary.show_all() => summary.keybinding_tabs().to_vec(),
        _ => KeybindingsTabId::ALL.to_vec(),
    }
}

fn active_keybinding_tab(
    search: Option<&TabSearchSummary>,
    preferred: KeybindingsTabId,
) -> Option<KeybindingsTabId> {
    match search {
        Some(summary) if summary.show_all() || summary.keybindings_tab_visible(preferred) => {
            Some(preferred)
        }
        Some(summary) => summary.keybinding_tabs().first().copied(),
        None => Some(preferred),
    }
}

fn keybinding_row_visible(
    search: Option<&TabSearchSummary>,
    active_tab: Option<KeybindingsTabId>,
    field: crate::models::KeybindingField,
) -> bool {
    let Some(search) = search else {
        return active_tab == Some(field.tab());
    };
    if search.show_all() {
        return true;
    }
    active_tab == Some(field.tab())
        && (search.keybinding_field_visible(field)
            || search.keybinding_tab_title_visible(field.tab()))
}
