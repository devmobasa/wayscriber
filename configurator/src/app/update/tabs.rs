use iced::Command;

use crate::messages::Message;
use crate::models::{KeybindingsTabId, TabId, UiTabId};

use super::super::state::ConfiguratorApp;

impl ConfiguratorApp {
    pub(super) fn handle_tab_selected(&mut self, tab: TabId) -> Command<Message> {
        self.active_tab = tab;
        Command::none()
    }

    pub(super) fn handle_ui_tab_selected(&mut self, tab: UiTabId) -> Command<Message> {
        self.active_ui_tab = tab;
        Command::none()
    }

    pub(super) fn handle_keybindings_tab_selected(
        &mut self,
        tab: KeybindingsTabId,
    ) -> Command<Message> {
        self.active_keybindings_tab = tab;
        Command::none()
    }
}
