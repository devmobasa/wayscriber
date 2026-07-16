mod arrow;
mod boards;
mod capture;
mod daemon;
mod drawing;
mod history;
mod keybindings;
mod performance;
mod presets;
mod render_profiles;
mod session;
#[cfg(feature = "tablet-input")]
mod tablet;
pub(crate) mod theme;
mod ui;
mod widgets;

use iced::widget::{Column, Row, Space, button, column, container, row, rule, text, text_input};
use iced::{Element, Length};
use wayscriber::config::Config;

use crate::messages::Message;
use crate::models::TabId;

use self::widgets::{default_label_color, feedback_text};
use super::search::{AppSearchSummary, SEARCH_INPUT_ID, TabSearchSummary};
use super::state::{ConfiguratorApp, StatusMessage};

const SEARCH_INPUT_WIDTH: f32 = 324.0;

impl ConfiguratorApp {
    pub(crate) fn view(&self) -> Element<'_, Message> {
        let search = self.search_summary();
        let header = self.header_view(&search);
        let content = self.tab_view(&search);
        let footer = self.footer_view();

        column![header, content, footer]
            .spacing(12)
            .padding(16)
            .into()
    }

    fn header_view(&self, search: &AppSearchSummary) -> Element<'_, Message> {
        let reload_button = button("Reload")
            .style(theme::Button::Secondary)
            .on_press(Message::ReloadRequested);

        let defaults_button = if self.defaults_reset_pending {
            button("Confirm Defaults")
                .style(theme::Button::Warning)
                .on_press(Message::ResetToDefaultsConfirmed)
        } else {
            button("Defaults")
                .style(theme::Button::Warning)
                .on_press(Message::ResetToDefaultsRequested)
        };

        let save_button = button("Save")
            .style(theme::Button::Primary)
            .on_press(Message::SaveRequested);

        let search_input = text_input("Search settings", search.raw_query())
            .id(SEARCH_INPUT_ID)
            .on_input(Message::SearchChanged)
            .padding(8)
            .width(Length::Fixed(SEARCH_INPUT_WIDTH));

        let mut toolbar = Row::new()
            .spacing(12)
            .align_y(iced::Alignment::Center)
            .push(reload_button)
            .push(defaults_button)
            .push(save_button)
            .push(search_input);

        if search.has_raw_input() {
            if search.is_active() {
                toolbar =
                    toolbar.push(text(format!("{} matches", search.total_matches())).size(14));
            }
            toolbar = toolbar.push(
                button("Clear")
                    .style(theme::Button::Subtle)
                    .on_press(Message::SearchCleared),
            );
        } else {
            toolbar = toolbar.push(
                button("Find")
                    .style(theme::Button::Subtle)
                    .on_press(Message::SearchFocusRequested),
            );
        }

        if self.defaults_reset_pending {
            toolbar = toolbar.push(
                button("Cancel")
                    .style(theme::Button::Subtle)
                    .on_press(Message::ResetToDefaultsCanceled),
            );
        }

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
                text("No unsaved changes")
                    .style(theme::Text::Color(iced::Color::from_rgb(0.6, 0.8, 0.6))),
            )
        };

        let toolbar = container(toolbar)
            .padding([6, 8])
            .width(Length::Fill)
            .style(theme::Container::ActionBar);

        let banner: Element<'_, Message> = match &self.status {
            StatusMessage::Idle => Space::new()
                .width(Length::Shrink)
                .height(Length::Shrink)
                .into(),
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
            StatusMessage::Warning(message) => container(text(message))
                .padding(8)
                .style(theme::Container::Warning)
                .into(),
        };

        column![toolbar, banner].spacing(8).into()
    }

    fn tab_view(&self, search: &AppSearchSummary) -> Element<'_, Message> {
        let tabs = visible_tabs(search);
        let active_tab = search.active_tab_or_first(self.active_tab);
        let tab_bar = tabs.iter().fold(
            Row::new().spacing(8).align_y(iced::Alignment::Center),
            |row, tab| {
                let label = tab.title();
                let button = button(label)
                    .padding([6, 12])
                    .style(if Some(*tab) == active_tab {
                        theme::Button::TabActive
                    } else {
                        theme::Button::TabInactive
                    })
                    .on_press(Message::TabSelected(*tab));
                row.push(button)
            },
        );

        let content: Element<'_, Message> = match active_tab {
            Some(TabId::Drawing) => self.drawing_tab(search.tab(TabId::Drawing)),
            Some(TabId::Presets) => self.presets_tab(search.tab(TabId::Presets)),
            Some(TabId::Arrow) => self.arrow_tab(search.tab(TabId::Arrow)),
            Some(TabId::History) => self.history_tab(search.tab(TabId::History)),
            Some(TabId::Performance) => self.performance_tab(search.tab(TabId::Performance)),
            Some(TabId::Ui) => self.ui_tab(search.tab(TabId::Ui)),
            Some(TabId::Boards) => self.boards_tab(search.tab(TabId::Boards)),
            Some(TabId::RenderProfiles) => {
                self.render_profiles_tab(search.tab(TabId::RenderProfiles))
            }
            Some(TabId::Capture) => self.capture_tab(search.tab(TabId::Capture)),
            Some(TabId::Daemon) => self.daemon_tab(search.tab(TabId::Daemon)),
            Some(TabId::Session) => self.session_tab(search.tab(TabId::Session)),
            Some(TabId::Keybindings) => self.keybindings_tab(search.tab(TabId::Keybindings)),
            #[cfg(feature = "tablet-input")]
            Some(TabId::Tablet) => self.tablet_tab(search.tab(TabId::Tablet)),
            None => empty_search_view(),
        };

        let legend = self.defaults_legend();

        column![tab_bar, rule::horizontal(2), legend, content]
            .spacing(12)
            .into()
    }

    fn footer_view(&self) -> Element<'_, Message> {
        let mut info = Column::new().spacing(4);

        let config_path = self
            .base_document
            .as_ref()
            .map(|document| document.source_path().to_path_buf())
            .or_else(|| Config::get_config_path().ok());
        if let Some(path) = config_path {
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
        .align_y(iced::Alignment::Center);

        let hint = feedback_text(
            "Tip: use Tab/Shift+Tab to move between fields; Enter activates buttons.",
            false,
        );

        column![legend, hint].spacing(4).into()
    }
}

fn visible_tabs(search: &AppSearchSummary) -> Vec<TabId> {
    if search.is_active() {
        search.tabs().iter().map(TabSearchSummary::tab).collect()
    } else {
        TabId::ALL.to_vec()
    }
}

fn empty_search_view<'a>() -> Element<'a, Message> {
    container(
        column![
            text("No settings match this search.").size(20),
            text("Try a field label, tab name, shortcut, or session path.").size(14),
        ]
        .spacing(8),
    )
    .padding(16)
    .style(theme::Container::Box)
    .into()
}
