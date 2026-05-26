use iced::Task;

use crate::messages::Message;
use crate::models::{DragMouseButton, KeybindingsTabId, TabId, UiTabId};

use super::super::state::ConfiguratorApp;

impl ConfiguratorApp {
    pub(super) fn handle_tab_selected(&mut self, tab: TabId) -> Task<Message> {
        self.active_tab = tab;
        Task::none()
    }

    pub(super) fn handle_ui_tab_selected(&mut self, tab: UiTabId) -> Task<Message> {
        self.active_ui_tab = tab;
        Task::none()
    }

    pub(super) fn handle_keybindings_tab_selected(
        &mut self,
        tab: KeybindingsTabId,
    ) -> Task<Message> {
        self.active_keybindings_tab = tab;
        Task::none()
    }

    pub(super) fn handle_drawing_drag_mapping_section_toggled(
        &mut self,
        button: DragMouseButton,
    ) -> Task<Message> {
        self.active_drawing_drag_button = if self.active_drawing_drag_button == Some(button) {
            None
        } else {
            Some(button)
        };
        Task::none()
    }
}
