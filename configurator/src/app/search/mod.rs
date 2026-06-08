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

    pub(super) fn handle_startup_search_focus_config_fallback(&mut self) -> Task<Message> {
        if !self.startup_search_focus_pending {
            return Task::none();
        }

        self.startup_search_focus_pending = false;
        self.handle_search_focus_requested()
    }

    pub(super) fn handle_search_focus_observed(&mut self, is_focused: bool) -> Task<Message> {
        self.search_input_focus_hint = is_focused;
        Task::none()
    }

    pub(super) fn handle_pointer_pressed(&mut self) -> Task<Message> {
        self.cancel_startup_search_focus();
        self.observe_search_focus()
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
                self.cancel_startup_search_focus();
                self.observe_search_focus()
            }
            _ => content_scroll_action_for_status(&event, status, self.search_input_focus_hint)
                .map_or_else(Task::none, scroll::ContentScrollAction::task),
        }
    }

    fn cancel_startup_search_focus(&mut self) {
        self.startup_search_focus_pending = false;
    }

    fn observe_search_focus(&self) -> Task<Message> {
        iced::widget::operation::is_focused(SEARCH_INPUT_ID).map(Message::SearchFocusObserved)
    }
}

fn content_scroll_action_for_status(
    event: &keyboard::Event,
    status: event::Status,
    allow_captured_edges: bool,
) -> Option<scroll::ContentScrollAction> {
    let action = scroll::content_scroll_action_for_event(event)?;
    if status == event::Status::Ignored
        || (allow_captured_edges && action.can_scroll_when_captured())
    {
        Some(action)
    } else {
        None
    }
}
