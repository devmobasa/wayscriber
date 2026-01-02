use crate::toolbar_icons;

use super::search::{find_match_range, row_matches};
use super::types::{Badge, Section, row};

pub(crate) struct SectionSets {
    pub(crate) all: Vec<Section>,
    pub(crate) page1: Vec<Section>,
    pub(crate) page2: Vec<Section>,
}

pub(crate) fn build_section_sets(
    frozen_enabled: bool,
    context_filter: bool,
    board_enabled: bool,
    capture_enabled: bool,
    page_prev_label: &str,
    page_next_label: &str,
) -> SectionSets {
    let board_modes_section = (!context_filter || board_enabled).then(|| Section {
        title: "Board Modes",
        rows: vec![
            row("Ctrl+W", "Toggle Whiteboard"),
            row("Ctrl+B", "Toggle Blackboard"),
            row("Ctrl+Shift+T", "Return to Transparent"),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_settings),
    });

    let pages_section = Section {
        title: "Pages",
        rows: vec![
            row(page_prev_label, "Previous page"),
            row(page_next_label, "Next page"),
            row("Ctrl+Alt+N", "New page"),
            row("Ctrl+Alt+D", "Duplicate page"),
            row("Ctrl+Alt+Delete", "Delete page"),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_file),
    };

    let drawing_section = Section {
        title: "Drawing",
        rows: vec![
            row("P", "Freehand pen"),
            row("Shift+Drag", "Line"),
            row("Ctrl+Drag", "Rectangle"),
            row("Tab+Drag", "Circle"),
            row("Ctrl+Shift+Drag", "Arrow"),
            row("Ctrl+Alt+H", "Highlight"),
            row("Ctrl+Alt+M", "Marker"),
            row("Ctrl+Alt+E", "Eraser"),
            row("+ / -", "Adjust thickness"),
        ],
        badges: vec![
            Badge {
                label: "R",
                color: [0.95, 0.41, 0.38],
            },
            Badge {
                label: "G",
                color: [0.46, 0.82, 0.45],
            },
            Badge {
                label: "B",
                color: [0.32, 0.58, 0.92],
            },
            Badge {
                label: "Y",
                color: [0.98, 0.80, 0.10],
            },
            Badge {
                label: "O",
                color: [0.98, 0.55, 0.26],
            },
            Badge {
                label: "P",
                color: [0.78, 0.47, 0.96],
            },
            Badge {
                label: "W",
                color: [0.90, 0.92, 0.96],
            },
            Badge {
                label: "K",
                color: [0.28, 0.30, 0.38],
            },
        ],
        icon: Some(toolbar_icons::draw_icon_pen),
    };

    let selection_section = Section {
        title: "Selection",
        rows: vec![
            row("S", "Selection tool"),
            row("Ctrl+A", "Select all"),
            row("Ctrl+D", "Deselect"),
            row("Ctrl+C", "Copy selection"),
            row("Ctrl+V", "Paste selection"),
            row("Delete", "Delete selection"),
            row("Ctrl+Alt+P", "Selection properties"),
            row("Shift+Scroll", "Adjust font size"),
        ],
        badges: vec![
            Badge {
                label: "F",
                color: [0.98, 0.80, 0.10],
            },
            Badge {
                label: "O",
                color: [0.98, 0.55, 0.26],
            },
            Badge {
                label: "P",
                color: [0.78, 0.47, 0.96],
            },
            Badge {
                label: "W",
                color: [0.90, 0.92, 0.96],
            },
            Badge {
                label: "K",
                color: [0.28, 0.30, 0.38],
            },
        ],
        icon: Some(toolbar_icons::draw_icon_select),
    };

    let pen_text_section = Section {
        title: "Pen & Text",
        rows: vec![
            row("T", "Text tool"),
            row("Shift+T", "Sticky note"),
            row("Shift+Scroll", "Adjust font size"),
            row("Ctrl+Alt+F", "Toggle fill"),
            row("Ctrl+Alt+B", "Text background"),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_text),
    };

    let zoom_section = Section {
        title: "Zoom",
        rows: vec![
            row("Ctrl+Alt+Scroll", "Zoom in/out"),
            row("Ctrl+Alt+0", "Reset zoom"),
            row("Ctrl+Alt+L", "Lock zoom"),
            row("Ctrl+Alt+Arrow", "Pan view"),
            row("Middle drag", "Pan view"),
            row("Ctrl+Alt+R", "Refresh zoom capture"),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_zoom_in),
    };

    let mut action_rows = vec![
        row("E", "Clear frame"),
        row("Ctrl+Z", "Undo"),
        row("Ctrl+Shift+H", "Toggle click highlight"),
        row("Right Click / Shift+F10", "Context menu"),
        row("Escape / Ctrl+Q", "Exit"),
        row("F1 / F10", "Toggle help"),
        row("F2 / F9", "Toggle toolbar"),
        row("Ctrl+Shift+K", "Toggle presenter mode"),
        row("F11", "Open configurator"),
        row("F4 / F12", "Toggle status bar"),
    ];
    if frozen_enabled {
        action_rows.push(row("Ctrl+Shift+F", "Freeze/unfreeze active monitor"));
    }
    let actions_section = Section {
        title: "Actions",
        rows: action_rows,
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_undo),
    };

    let screenshots_section = (!context_filter || capture_enabled).then(|| Section {
        title: "Screenshots",
        rows: vec![
            row("Ctrl+C", "Full screen \u{2192} clipboard"),
            row("Ctrl+S", "Full screen \u{2192} file"),
            row("Ctrl+Shift+C", "Region \u{2192} clipboard"),
            row("Ctrl+Shift+S", "Region \u{2192} file"),
            row("Ctrl+Shift+O", "Active window (Hyprland)"),
            row("Ctrl+Shift+I", "Selection (capture defaults)"),
            row("Ctrl+Alt+O", "Open capture folder"),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_save),
    });

    let mut all_sections = Vec::new();
    if let Some(section) = board_modes_section.clone() {
        all_sections.push(section);
    }
    all_sections.push(actions_section.clone());
    all_sections.push(drawing_section.clone());
    all_sections.push(pen_text_section.clone());
    all_sections.push(zoom_section.clone());
    all_sections.push(selection_section.clone());
    all_sections.push(pages_section.clone());
    if let Some(section) = screenshots_section.clone() {
        all_sections.push(section);
    }

    let mut page1_sections = Vec::new();
    if let Some(section) = board_modes_section {
        page1_sections.push(section);
    }
    page1_sections.push(actions_section);
    page1_sections.push(drawing_section);
    page1_sections.push(pen_text_section);

    let mut page2_sections = vec![pages_section, zoom_section, selection_section];
    if let Some(section) = screenshots_section {
        page2_sections.push(section);
    }

    SectionSets {
        all: all_sections,
        page1: page1_sections,
        page2: page2_sections,
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
