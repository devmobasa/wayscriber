mod summary;
mod terms;
#[cfg(test)]
mod tests;
mod types;

use iced::keyboard::{self, Key, key};
use iced::{Task, event};

use crate::messages::Message;
use crate::models::{SearchQuery, TabId};

use super::scroll;
use super::state::ConfiguratorApp;

pub(crate) use types::{AppSearchSummary, SearchArea, TabSearchSummary};

pub(crate) const SEARCH_INPUT_ID: &str = "configurator-search-input";

impl ConfiguratorApp {
    pub(crate) fn search_summary(&self) -> AppSearchSummary {
        summary::build_search_summary(self)
    }

    pub(crate) fn align_active_tabs_for_search(&mut self) {
        let search = self.search_summary();
        if !search.is_active() {
            return;
        }

        if let Some(tab) = search.active_tab_or_first(self.active_tab) {
            self.active_tab = tab;
        }

        match self.active_tab {
            TabId::Ui => self.align_active_ui_tab_for_search(&search),
            TabId::Keybindings => self.align_active_keybindings_tab_for_search(&search),
            _ => {}
        }
    }

    fn align_active_ui_tab_for_search(&mut self, search: &AppSearchSummary) {
        let Some(tab) = search.tab(TabId::Ui) else {
            return;
        };
        if tab.ui_tab_visible(self.active_ui_tab) {
            return;
        }
        if let Some(first) = tab.ui_tabs().first().copied() {
            self.active_ui_tab = first;
        }
    }

    fn align_active_keybindings_tab_for_search(&mut self, search: &AppSearchSummary) {
        let Some(tab) = search.tab(TabId::Keybindings) else {
            return;
        };
        if tab.keybindings_tab_visible(self.active_keybindings_tab) {
            return;
        }
        if let Some(first) = tab.keybinding_tabs().first().copied() {
            self.active_keybindings_tab = first;
        }
    }

    pub(super) fn handle_search_changed(&mut self, value: String) -> Task<Message> {
        self.search_input_focus_hint = true;
        self.search_query = SearchQuery::new(value);
        self.align_active_tabs_for_search();
        Task::none()
    }

    pub(super) fn handle_search_cleared(&mut self) -> Task<Message> {
        self.search_query = SearchQuery::default();
        Task::none()
    }

    pub(super) fn handle_search_focus_requested(&mut self) -> Task<Message> {
        self.search_input_focus_hint = true;
        iced::widget::operation::focus(SEARCH_INPUT_ID)
    }

    pub(super) fn handle_pointer_pressed(&mut self) -> Task<Message> {
        self.search_input_focus_hint = false;
        Task::none()
    }

    pub(super) fn handle_keyboard_event(
        &mut self,
        event: keyboard::Event,
        status: event::Status,
    ) -> Task<Message> {
        let keyboard::Event::KeyPressed { key, modifiers, .. } = &event else {
            return Task::none();
        };

        match key.as_ref() {
            Key::Character("f") | Key::Character("F") if modifiers.command() => {
                self.handle_search_focus_requested()
            }
            Key::Named(key::Named::Escape) if self.search_query.has_raw_input() => {
                let should_refocus_search = self.search_input_focus_hint;
                self.search_query = SearchQuery::default();
                if should_refocus_search {
                    self.handle_search_focus_requested()
                } else {
                    Task::none()
                }
            }
            Key::Named(key::Named::Tab) => {
                self.search_input_focus_hint = false;
                Task::none()
            }
            _ => content_scroll_action_for_status(&event, status)
                .map_or_else(Task::none, scroll::ContentScrollAction::task),
        }
    }
}

fn content_scroll_action_for_status(
    event: &keyboard::Event,
    status: event::Status,
) -> Option<scroll::ContentScrollAction> {
    (status == event::Status::Ignored)
        .then(|| scroll::content_scroll_action_for_event(event))
        .flatten()
}
