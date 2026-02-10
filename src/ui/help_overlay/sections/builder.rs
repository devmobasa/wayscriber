mod quick;
mod sections;

use super::super::search::{find_match_range, row_matches};
use super::super::types::{Section, row};
use super::bindings::HelpOverlayBindings;
use quick::build_quick_sections;
use sections::build_main_sections;

pub(crate) struct SectionSets {
    pub(crate) all: Vec<Section>,
    pub(crate) page1: Vec<Section>,
    pub(crate) page2: Vec<Section>,
    pub(crate) quick: Vec<Section>,
}

pub(crate) fn build_section_sets(
    bindings: &HelpOverlayBindings,
    frozen_enabled: bool,
    context_filter: bool,
    board_enabled: bool,
    capture_enabled: bool,
) -> SectionSets {
    let mut sections = build_main_sections(
        bindings,
        frozen_enabled,
        context_filter,
        board_enabled,
        capture_enabled,
    );

    let mut all_sections = vec![
        sections.drawing.clone(),
        sections.actions.clone(),
        sections.pen_text.clone(),
        sections.zoom.clone(),
        sections.selection.clone(),
        sections.pages.clone(),
    ];
    if let Some(section) = sections.board_modes.clone() {
        all_sections.insert(2, section);
    }
    if let Some(section) = sections.screenshots.clone() {
        all_sections.push(section);
    }

    let mut page1_sections = vec![
        sections.drawing.clone(),
        sections.actions.clone(),
        sections.pen_text.clone(),
    ];
    if let Some(section) = sections.board_modes.take() {
        page1_sections.insert(2, section);
    }

    let mut page2_sections = vec![sections.pages, sections.zoom, sections.selection];
    if let Some(section) = sections.screenshots.take() {
        page2_sections.push(section);
    }

    SectionSets {
        all: all_sections,
        page1: page1_sections,
        page2: page2_sections,
        quick: build_quick_sections(bindings),
    }
}

pub(crate) fn filter_sections_for_search(
    all_sections: Vec<Section>,
    search_lower: &str,
) -> Vec<Section> {
    let mut filtered = Vec::new();
    for mut section in all_sections {
        let title_match = find_match_range(section.title, search_lower).is_some();
        if !title_match {
            section.rows.retain(|row| row_matches(row, search_lower));
        }
        if !section.rows.is_empty() {
            filtered.push(section);
        }
    }

    if filtered.is_empty() {
        filtered.push(Section {
            title: "No results",
            rows: vec![
                row("", "Try: zoom, page, selection, capture"),
                row("", "Tip: search by key or action name"),
            ],
            badges: Vec::new(),
            icon: None,
        });
    }

    filtered
}
