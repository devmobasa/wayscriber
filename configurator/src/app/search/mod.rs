mod summary;
mod terms;
#[cfg(test)]
mod tests;
mod types;

use crate::models::{SearchQuery, TabId};

use super::effects::Effect;
use super::state::ConfiguratorApp;

pub(crate) use types::{AppSearchSummary, SearchArea, TabSearchSummary};

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

    pub(super) fn handle_search_changed(&mut self, value: String) -> Vec<Effect> {
        self.search_query = SearchQuery::new(value);
        self.align_active_tabs_for_search();
        Vec::new()
    }

    pub(super) fn handle_search_cleared(&mut self) -> Vec<Effect> {
        self.search_query = SearchQuery::default();
        Vec::new()
    }

    /// Asks the shell to put the caret in the search box once.
    ///
    /// The request is a serial rather than a task: the model cannot reach a
    /// widget, and the shell honors each new serial exactly once.
    pub(super) fn handle_search_focus_requested(&mut self) -> Vec<Effect> {
        self.search_focus_serial = self.search_focus_serial.saturating_add(1);
        Vec::new()
    }

    /// A click, tap, or Tab press observed before the initial config load
    /// finished: the deferred startup focus offer is answered by the user's
    /// own navigation and must not fire later.
    pub(super) fn handle_startup_interaction_observed(&mut self) -> Vec<Effect> {
        self.startup_search_focus_pending = false;
        Vec::new()
    }

    pub(super) fn handle_startup_search_focus_config_fallback(&mut self) -> Vec<Effect> {
        if !self.startup_search_focus_pending {
            return Vec::new();
        }

        self.startup_search_focus_pending = false;
        self.handle_search_focus_requested()
    }
}
