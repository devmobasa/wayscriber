use crate::models::{KeybindingField, KeybindingsTabId, SearchQuery, TabId, UiTabId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SearchArea {
    DrawingColor,
    DrawingDefaults,
    DrawingDragTools,
    DrawingFont,
    PresetControls,
    Arrow,
    HistoryMain,
    HistoryCustom,
    PerformanceRendering,
    PerformanceAnimations,
    UiGeneral,
    BoardsGeneral,
    RenderProfilesGeneral,
    CaptureFiles,
    CapturePdf,
    DaemonStatus,
    DaemonService,
    DaemonShortcut,
    DaemonLightControls,
    SessionPersistence,
    SessionCatalog,
    #[cfg(feature = "tablet-input")]
    Tablet,
}

#[derive(Debug, Clone)]
pub(crate) struct AppSearchSummary {
    pub(super) query: SearchQuery,
    pub(super) tabs: Vec<TabSearchSummary>,
}

impl AppSearchSummary {
    pub(super) fn inactive(query: SearchQuery) -> Self {
        Self {
            query,
            tabs: Vec::new(),
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.query.is_active()
    }

    pub(crate) fn raw_query(&self) -> &str {
        self.query.raw()
    }

    pub(crate) fn has_raw_input(&self) -> bool {
        self.query.has_raw_input()
    }

    pub(crate) fn total_matches(&self) -> usize {
        if !self.is_active() {
            return 0;
        }
        self.tabs.iter().map(TabSearchSummary::match_count).sum()
    }

    pub(crate) fn tabs(&self) -> &[TabSearchSummary] {
        &self.tabs
    }

    pub(crate) fn tab(&self, tab: TabId) -> Option<&TabSearchSummary> {
        self.tabs.iter().find(|summary| summary.tab == tab)
    }

    pub(crate) fn tab_is_visible(&self, tab: TabId) -> bool {
        !self.is_active() || self.tab(tab).is_some()
    }

    pub(crate) fn active_tab_or_first(&self, preferred: TabId) -> Option<TabId> {
        if !self.is_active() || self.tab_is_visible(preferred) {
            return Some(preferred);
        }
        self.tabs.first().map(|summary| summary.tab)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TabSearchSummary {
    tab: TabId,
    direct_title_match: bool,
    alias_match: bool,
    areas: Vec<SearchArea>,
    ui_tabs: Vec<UiTabId>,
    keybinding_tabs: Vec<KeybindingsTabId>,
    direct_keybinding_tabs: Vec<KeybindingsTabId>,
    board_indices: Vec<usize>,
    preset_slots: Vec<usize>,
    render_profile_indices: Vec<usize>,
    render_profile_control_indices: Vec<usize>,
    render_profile_mapping_indices: Vec<(usize, usize)>,
    session_catalog_all_items: bool,
    session_item_ids: Vec<String>,
    keybinding_fields: Vec<KeybindingField>,
}

impl TabSearchSummary {
    pub(super) fn new(tab: TabId, direct_title_match: bool, alias_match: bool) -> Self {
        Self {
            tab,
            direct_title_match,
            alias_match,
            areas: Vec::new(),
            ui_tabs: Vec::new(),
            keybinding_tabs: Vec::new(),
            direct_keybinding_tabs: Vec::new(),
            board_indices: Vec::new(),
            preset_slots: Vec::new(),
            render_profile_indices: Vec::new(),
            render_profile_control_indices: Vec::new(),
            render_profile_mapping_indices: Vec::new(),
            session_catalog_all_items: false,
            session_item_ids: Vec::new(),
            keybinding_fields: Vec::new(),
        }
    }

    pub(crate) fn tab(&self) -> TabId {
        self.tab
    }

    pub(crate) fn show_all(&self) -> bool {
        self.direct_title_match || self.alias_match
    }

    pub(crate) fn area_matches(&self, area: SearchArea) -> bool {
        self.show_all() || self.areas.contains(&area)
    }

    pub(crate) fn ui_tab_visible(&self, tab: UiTabId) -> bool {
        self.show_all() || self.ui_tabs.contains(&tab)
    }

    pub(crate) fn keybindings_tab_visible(&self, tab: KeybindingsTabId) -> bool {
        self.show_all() || self.keybinding_tabs.contains(&tab)
    }

    pub(crate) fn ui_tabs(&self) -> &[UiTabId] {
        &self.ui_tabs
    }

    pub(crate) fn keybinding_tabs(&self) -> &[KeybindingsTabId] {
        &self.keybinding_tabs
    }

    pub(crate) fn keybinding_tab_title_visible(&self, tab: KeybindingsTabId) -> bool {
        self.direct_keybinding_tabs.contains(&tab)
    }

    pub(crate) fn board_indices(&self) -> &[usize] {
        &self.board_indices
    }

    pub(crate) fn preset_slots(&self) -> &[usize] {
        &self.preset_slots
    }

    pub(crate) fn render_profile_indices(&self) -> &[usize] {
        &self.render_profile_indices
    }

    pub(crate) fn render_profile_controls_visible(&self, index: usize) -> bool {
        self.show_all() || self.render_profile_control_indices.contains(&index)
    }

    pub(crate) fn render_profile_mapping_indices(&self) -> &[(usize, usize)] {
        &self.render_profile_mapping_indices
    }

    pub(crate) fn session_item_visible(&self, id: &str) -> bool {
        self.show_all()
            || self.session_catalog_all_items
            || self.session_item_ids.iter().any(|item_id| item_id == id)
    }

    pub(crate) fn keybinding_field_visible(&self, field: KeybindingField) -> bool {
        self.show_all() || self.keybinding_fields.contains(&field)
    }

    pub(super) fn has_content(&self) -> bool {
        self.show_all()
            || !self.areas.is_empty()
            || !self.ui_tabs.is_empty()
            || !self.keybinding_tabs.is_empty()
            || !self.direct_keybinding_tabs.is_empty()
            || !self.board_indices.is_empty()
            || !self.preset_slots.is_empty()
            || !self.render_profile_indices.is_empty()
            || !self.render_profile_control_indices.is_empty()
            || !self.render_profile_mapping_indices.is_empty()
            || self.session_catalog_all_items
            || !self.session_item_ids.is_empty()
            || !self.keybinding_fields.is_empty()
    }

    fn match_count(&self) -> usize {
        let count = self.areas.len()
            + self.ui_tabs.len()
            + self.keybinding_tabs.len()
            + self.direct_keybinding_tabs.len()
            + self.board_indices.len()
            + self.preset_slots.len()
            + self.render_profile_indices.len()
            + self.render_profile_mapping_indices.len()
            + usize::from(self.session_catalog_all_items)
            + self.session_item_ids.len()
            + self.keybinding_fields.len();
        if self.show_all() && count == 0 {
            1
        } else {
            count.max(1)
        }
    }

    pub(super) fn add_area(&mut self, area: SearchArea) {
        push_unique(&mut self.areas, area);
    }

    pub(super) fn add_ui_tab(&mut self, tab: UiTabId) {
        push_unique(&mut self.ui_tabs, tab);
    }

    pub(super) fn add_direct_keybinding_tab(&mut self, tab: KeybindingsTabId) {
        self.add_keybinding_tab(tab);
        push_unique(&mut self.direct_keybinding_tabs, tab);
    }

    pub(super) fn add_board_index(&mut self, index: usize) {
        push_unique(&mut self.board_indices, index);
    }

    pub(super) fn add_preset_slot(&mut self, slot: usize) {
        push_unique(&mut self.preset_slots, slot);
    }

    pub(super) fn add_render_profile_index(&mut self, index: usize) {
        push_unique(&mut self.render_profile_indices, index);
        push_unique(&mut self.render_profile_control_indices, index);
    }

    pub(super) fn add_render_profile_mapping_index(
        &mut self,
        profile_index: usize,
        mapping_index: usize,
    ) {
        push_unique(&mut self.render_profile_indices, profile_index);
        push_unique(
            &mut self.render_profile_mapping_indices,
            (profile_index, mapping_index),
        );
    }

    pub(super) fn show_all_session_catalog_items(&mut self) {
        self.session_catalog_all_items = true;
    }

    pub(super) fn add_session_item_id(&mut self, id: &str) {
        if !self.session_item_ids.iter().any(|item_id| item_id == id) {
            self.session_item_ids.push(id.to_string());
        }
    }

    pub(super) fn add_keybinding_field(&mut self, field: KeybindingField) {
        push_unique(&mut self.keybinding_fields, field);
        self.add_keybinding_tab(field.tab());
    }

    fn add_keybinding_tab(&mut self, tab: KeybindingsTabId) {
        push_unique(&mut self.keybinding_tabs, tab);
    }
}

fn push_unique<T: PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}
