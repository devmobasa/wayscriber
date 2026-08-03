use crate::models::{DragMouseButton, KeybindingsTabId, TabId, UiTabId};

use super::super::effects::Effect;
use super::super::state::ConfiguratorApp;

impl ConfiguratorApp {
    pub(super) fn handle_tab_selected(&mut self, tab: TabId) -> Vec<Effect> {
        self.active_tab = tab;
        self.align_active_tabs_for_search();
        Vec::new()
    }

    pub(super) fn handle_ui_tab_selected(&mut self, tab: UiTabId) -> Vec<Effect> {
        self.active_ui_tab = tab;
        self.align_active_tabs_for_search();
        Vec::new()
    }

    pub(super) fn handle_keybindings_tab_selected(&mut self, tab: KeybindingsTabId) -> Vec<Effect> {
        self.active_keybindings_tab = tab;
        self.align_active_tabs_for_search();
        Vec::new()
    }

    pub(super) fn handle_drawing_drag_mapping_section_toggled(
        &mut self,
        button: DragMouseButton,
    ) -> Vec<Effect> {
        self.active_drawing_drag_button = if self.active_drawing_drag_button == Some(button) {
            None
        } else {
            Some(button)
        };
        Vec::new()
    }
}
