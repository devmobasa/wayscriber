use crate::app::state::ConfiguratorApp;
use crate::models::{KeybindingsTabId, SearchQuery, TabId, UiTabId};
use wayscriber::config::{
    PERFORMANCE_FIELD_METADATA, PerformanceFieldGroup, toolbar_item_definitions,
};

use super::terms::*;
use super::types::{AppSearchSummary, SearchArea, TabSearchSummary};

pub(super) fn build_search_summary(app: &ConfiguratorApp) -> AppSearchSummary {
    if !app.search_query.is_active() {
        return AppSearchSummary::inactive(app.search_query.clone());
    }

    let tabs = TabId::ALL
        .iter()
        .filter_map(|tab| tab_summary(app, *tab))
        .collect();

    AppSearchSummary {
        query: app.search_query.clone(),
        tabs,
    }
}

fn tab_summary(app: &ConfiguratorApp, tab: TabId) -> Option<TabSearchSummary> {
    let query = &app.search_query;
    let direct_title_match = query.matches_text(tab.title());
    let alias_match = tab_aliases(tab)
        .iter()
        .any(|alias| query.matches_text(alias));
    let mut summary = TabSearchSummary::new(tab, direct_title_match, alias_match);

    if !summary.show_all() {
        match tab {
            TabId::Drawing => drawing_matches(query, &mut summary),
            TabId::Presets => preset_matches(app, query, &mut summary),
            TabId::Arrow => add_area_if(query, &mut summary, tab, SearchArea::Arrow, ARROW_TERMS),
            TabId::History => history_matches(query, &mut summary),
            TabId::Performance => performance_matches(query, &mut summary),
            TabId::Ui => ui_matches(query, &mut summary),
            TabId::Boards => board_matches(app, query, &mut summary),
            TabId::RenderProfiles => render_profile_matches(app, query, &mut summary),
            TabId::Capture => capture_matches(query, &mut summary),
            TabId::Daemon => daemon_matches(query, &mut summary),
            TabId::Session => session_matches(app, query, &mut summary),
            TabId::Keybindings => keybinding_matches(app, query, &mut summary),
            #[cfg(feature = "tablet-input")]
            TabId::Tablet => {
                add_area_if(query, &mut summary, tab, SearchArea::Tablet, TABLET_TERMS)
            }
        }
    }

    summary.has_content().then_some(summary)
}

fn drawing_matches(query: &SearchQuery, summary: &mut TabSearchSummary) {
    add_area_if(
        query,
        summary,
        TabId::Drawing,
        SearchArea::DrawingColor,
        DRAWING_COLOR_TERMS,
    );
    add_area_if(
        query,
        summary,
        TabId::Drawing,
        SearchArea::DrawingDefaults,
        DRAWING_DEFAULT_TERMS,
    );
    add_area_if(
        query,
        summary,
        TabId::Drawing,
        SearchArea::DrawingDragTools,
        DRAWING_DRAG_TERMS,
    );
    add_area_if(
        query,
        summary,
        TabId::Drawing,
        SearchArea::DrawingFont,
        DRAWING_FONT_TERMS,
    );
}

fn preset_matches(app: &ConfiguratorApp, query: &SearchQuery, summary: &mut TabSearchSummary) {
    add_area_if(
        query,
        summary,
        TabId::Presets,
        SearchArea::PresetControls,
        PRESET_CONTROL_TERMS,
    );
    for slot_index in 1..=app.draft.presets.slot_count {
        let Some(slot) = app.draft.presets.slot(slot_index) else {
            continue;
        };
        let mut text = format!("preset slot {slot_index} settings enabled");
        if slot.enabled {
            text.push_str(&format!(
                " label tool color named color rgb color fill enabled text background show status bar arrow head at end {} {} {} {} {} {} {} {} {} {} {} {} {:?} {:?} {:?}",
                slot.name,
                slot.tool.label(),
                slot.color.summary(),
                slot.color.rgb.join(" "),
                slot.color.name,
                slot.size,
                slot.marker_opacity,
                slot.font_size,
                slot.eraser_kind.label(),
                slot.eraser_mode.label(),
                slot.arrow_length,
                slot.arrow_angle,
                slot.fill_enabled,
                slot.text_background_enabled,
                slot.arrow_head_at_end,
            ));
        } else {
            text.push_str(" slot disabled enable to configure");
        }
        if query.matches_text(&text) {
            summary.add_preset_slot(slot_index);
        }
    }
}

fn history_matches(query: &SearchQuery, summary: &mut TabSearchSummary) {
    add_area_if(
        query,
        summary,
        TabId::History,
        SearchArea::HistoryMain,
        HISTORY_MAIN_TERMS,
    );
    add_area_if(
        query,
        summary,
        TabId::History,
        SearchArea::HistoryCustom,
        HISTORY_CUSTOM_TERMS,
    );
}

fn performance_matches(query: &SearchQuery, summary: &mut TabSearchSummary) {
    for (group, area, identity) in [
        (
            PerformanceFieldGroup::Rendering,
            SearchArea::PerformanceRendering,
            "rendering",
        ),
        (
            PerformanceFieldGroup::Animations,
            SearchArea::PerformanceAnimations,
            "animations",
        ),
    ] {
        let mut parts = vec![identity];
        for metadata in PERFORMANCE_FIELD_METADATA
            .iter()
            .filter(|metadata| metadata.group == group)
        {
            parts.extend([metadata.path, metadata.label, metadata.help]);
            parts.extend_from_slice(metadata.search_terms);
        }
        if query.matches_parts_scoped_to_tab(TabId::Performance, parts) {
            summary.add_area(area);
        }
    }
}

fn ui_matches(query: &SearchQuery, summary: &mut TabSearchSummary) {
    add_area_if(
        query,
        summary,
        TabId::Ui,
        SearchArea::UiGeneral,
        UI_GENERAL_TERMS,
    );
    for tab in UiTabId::ALL {
        let identity_parts = std::iter::once(tab.title())
            .chain(ui_tab_aliases(tab).iter().copied())
            .collect::<Vec<_>>();
        let field_terms = ui_tab_terms(tab);
        let full_parts = identity_parts
            .iter()
            .copied()
            .chain(field_terms.iter().copied())
            .collect::<Vec<_>>();
        if query.matches_parts_scoped_to_tab(TabId::Ui, identity_parts.iter().copied())
            || query.matches_parts(full_parts.iter().copied())
            || (tab == UiTabId::ToolbarVisibility && toolbar_item_matches(query))
            || (query.matches_any_raw_text(field_terms)
                && query.matches_parts_scoped_to_tab(TabId::Ui, full_parts.iter().copied()))
        {
            summary.add_ui_tab(tab);
        }
    }
}

fn toolbar_item_matches(query: &SearchQuery) -> bool {
    toolbar_item_definitions().iter().any(|definition| {
        query.matches_parts([
            "toolbar item",
            definition.label,
            definition.id.as_str(),
            definition.group.map_or("", |group| group.as_str()),
        ]) || query.matches_parts_scoped_to_tab(
            TabId::Ui,
            [
                "toolbar visibility",
                "toolbar item",
                definition.label,
                definition.id.as_str(),
                definition.group.map_or("", |group| group.as_str()),
            ],
        )
    })
}

fn board_matches(app: &ConfiguratorApp, query: &SearchQuery, summary: &mut TabSearchSummary) {
    add_area_if(
        query,
        summary,
        TabId::Boards,
        SearchArea::BoardsGeneral,
        BOARD_GENERAL_TERMS,
    );
    for (index, item) in app.draft.boards.items.iter().enumerate() {
        let text = format!(
            "board {} board id display name background background color override default pen color pen color auto-adjust pen auto adjust pen persist pinned duplicate remove up down collapse expand {} {} {} background pen persist pinned auto adjust",
            index + 1,
            item.id,
            item.name,
            item.background_kind.label(),
        );
        if query.matches_text(&text) {
            summary.add_board_index(index);
        }
    }
}

fn render_profile_matches(
    app: &ConfiguratorApp,
    query: &SearchQuery,
    summary: &mut TabSearchSummary,
) {
    add_area_if(
        query,
        summary,
        TabId::RenderProfiles,
        SearchArea::RenderProfilesGeneral,
        RENDER_PROFILE_GENERAL_TERMS,
    );
    for (index, profile) in app.draft.render_profiles.profiles.iter().enumerate() {
        let text = format!(
            "render profile {} {} {} id name duplicate delete add mapping",
            index + 1,
            profile.id,
            profile.name
        );
        if query.matches_text(&text) {
            summary.add_render_profile_index(index);
        }
        for (mapping_index, mapping) in profile.mappings.iter().enumerate() {
            let mapping_text = format!(
                "mapping {} color mapping from to {} {} remove pick",
                mapping_index + 1,
                mapping.from,
                mapping.to,
            );
            if query.matches_text(&mapping_text) {
                summary.add_render_profile_mapping_index(index, mapping_index);
            }
        }
    }
}

fn capture_matches(query: &SearchQuery, summary: &mut TabSearchSummary) {
    add_area_if(
        query,
        summary,
        TabId::Capture,
        SearchArea::CaptureFiles,
        CAPTURE_FILE_TERMS,
    );
    if query.matches_parts(CAPTURE_PDF_TERMS.iter().copied())
        || (query.matches_any_raw_text(CAPTURE_PDF_IDENTITY_TERMS)
            && query.matches_parts_scoped_to_tab(TabId::Capture, CAPTURE_PDF_TERMS.iter().copied()))
    {
        summary.add_area(SearchArea::CapturePdf);
    }
}

fn daemon_matches(query: &SearchQuery, summary: &mut TabSearchSummary) {
    add_area_if(
        query,
        summary,
        TabId::Daemon,
        SearchArea::DaemonStatus,
        DAEMON_STATUS_TERMS,
    );
    add_area_if(
        query,
        summary,
        TabId::Daemon,
        SearchArea::DaemonService,
        DAEMON_SERVICE_TERMS,
    );
    add_area_if(
        query,
        summary,
        TabId::Daemon,
        SearchArea::DaemonShortcut,
        DAEMON_SHORTCUT_TERMS,
    );
    add_area_if(
        query,
        summary,
        TabId::Daemon,
        SearchArea::DaemonLightControls,
        DAEMON_LIGHT_TERMS,
    );
}

fn session_matches(app: &ConfiguratorApp, query: &SearchQuery, summary: &mut TabSearchSummary) {
    add_area_if(
        query,
        summary,
        TabId::Session,
        SearchArea::SessionPersistence,
        SESSION_PERSISTENCE_TERMS,
    );
    let catalog_area_matched = query.matches_parts(SESSION_CATALOG_TERMS.iter().copied());
    if catalog_area_matched {
        summary.add_area(SearchArea::SessionCatalog);
        summary.show_all_session_catalog_items();
    }
    add_area_if(
        query,
        summary,
        TabId::Session,
        SearchArea::SessionCatalog,
        SESSION_CATALOG_TERMS,
    );
    for item in &app.session_catalog.items {
        let text = format!(
            "session saved recent catalog {} {} {} {:?}",
            item.display_name, item.path_label, item.created_label, item.path,
        );
        if query.matches_text(&text) {
            summary.add_area(SearchArea::SessionCatalog);
            summary.add_session_item_id(&item.id);
        }
    }
}

fn keybinding_matches(app: &ConfiguratorApp, query: &SearchQuery, summary: &mut TabSearchSummary) {
    for tab in KeybindingsTabId::ALL {
        if query.matches_parts_scoped_to_tab(TabId::Keybindings, [tab.title()]) {
            summary.add_direct_keybinding_tab(tab);
        }
    }
    for entry in &app.draft.keybindings.entries {
        let default_value = app
            .defaults
            .keybindings
            .value_for(entry.field)
            .unwrap_or("");
        let text = format!(
            "keybindings keybinding shortcut hotkey keyboard shortcut list {} {} {} {}",
            entry.field.tab().title(),
            entry.field.label(),
            entry.field.field_key(),
            entry.value,
        );
        if query.matches_text(&text) || query.matches_text(default_value) {
            summary.add_keybinding_field(entry.field);
        }
    }
}

fn add_area_if(
    query: &SearchQuery,
    summary: &mut TabSearchSummary,
    tab: TabId,
    area: SearchArea,
    terms: &[&str],
) {
    if query.matches_parts_scoped_to_tab(tab, terms.iter().copied()) {
        summary.add_area(area);
    }
}

trait ScopedSearchQuery {
    fn matches_parts_scoped_to_tab<'a>(
        &self,
        tab: TabId,
        parts: impl IntoIterator<Item = &'a str>,
    ) -> bool;
    fn matches_any_raw_text(&self, values: &[&str]) -> bool;
}

impl ScopedSearchQuery for SearchQuery {
    fn matches_parts_scoped_to_tab<'a>(
        &self,
        tab: TabId,
        parts: impl IntoIterator<Item = &'a str>,
    ) -> bool {
        let parts = parts.into_iter().collect::<Vec<_>>();
        self.matches_parts(std::iter::once(tab.title()).chain(parts.iter().copied()))
            || tab_scope_aliases(tab).iter().any(|alias| {
                self.matches_parts(std::iter::once(*alias).chain(parts.iter().copied()))
            })
    }

    fn matches_any_raw_text(&self, values: &[&str]) -> bool {
        values
            .iter()
            .any(|value| SearchQuery::new(*value).matches_text(self.raw()))
    }
}
