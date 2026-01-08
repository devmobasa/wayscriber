use std::collections::{HashMap, HashSet};

use crate::config::{Action, action_label, action_meta_iter};
use crate::input::InputState;
use crate::label_format::{NOT_BOUND_LABEL, format_binding_labels_or, join_binding_labels};
use crate::toolbar_icons;

use super::search::{find_match_range, row_matches};
use super::types::{Badge, Section, row};

pub(crate) struct SectionSets {
    pub(crate) all: Vec<Section>,
    pub(crate) page1: Vec<Section>,
    pub(crate) page2: Vec<Section>,
}

pub struct HelpOverlayBindings {
    labels: HashMap<Action, Vec<String>>,
    cache_key: String,
}

impl HelpOverlayBindings {
    pub fn from_input_state(state: &InputState) -> Self {
        let mut labels = HashMap::new();
        for meta in action_meta_iter().filter(|meta| meta.in_help) {
            let bindings = state.action_binding_labels(meta.action);
            if !bindings.is_empty() {
                labels.insert(meta.action, bindings);
            }
        }

        let mut cache_parts = Vec::new();
        for meta in action_meta_iter().filter(|meta| meta.in_help) {
            if let Some(values) = labels.get(&meta.action) {
                cache_parts.push(format!("{:?}={}", meta.action, values.join("/")));
            }
        }

        Self {
            labels,
            cache_key: cache_parts.join("|"),
        }
    }

    pub(crate) fn labels_for(&self, action: Action) -> Option<&[String]> {
        self.labels.get(&action).map(|values| values.as_slice())
    }

    pub(crate) fn cache_key(&self) -> &str {
        self.cache_key.as_str()
    }
}

fn collect_labels(bindings: &HelpOverlayBindings, actions: &[Action]) -> Vec<String> {
    let mut labels = Vec::new();
    let mut seen = HashSet::new();
    for action in actions {
        if let Some(values) = bindings.labels_for(*action) {
            for value in values {
                if seen.insert(value.clone()) {
                    labels.push(value.clone());
                }
            }
        }
    }
    labels
}

fn joined_labels(bindings: &HelpOverlayBindings, actions: &[Action]) -> Option<String> {
    join_binding_labels(&collect_labels(bindings, actions))
}

fn binding_or_fallback(bindings: &HelpOverlayBindings, action: Action, fallback: &str) -> String {
    format_binding_labels_or(&collect_labels(bindings, &[action]), fallback)
}

fn bindings_or_fallback(
    bindings: &HelpOverlayBindings,
    actions: &[Action],
    fallback: &str,
) -> String {
    format_binding_labels_or(&collect_labels(bindings, actions), fallback)
}

fn primary_or_fallback(bindings: &HelpOverlayBindings, action: Action, fallback: &str) -> String {
    bindings
        .labels_for(action)
        .and_then(|values| values.first())
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

fn color_badge(
    bindings: &HelpOverlayBindings,
    action: Action,
    fallback: &str,
    color: [f64; 3],
) -> Badge {
    Badge {
        label: primary_or_fallback(bindings, action, fallback),
        color,
    }
}

pub(crate) fn build_section_sets(
    bindings: &HelpOverlayBindings,
    frozen_enabled: bool,
    context_filter: bool,
    board_enabled: bool,
    capture_enabled: bool,
) -> SectionSets {
    let board_modes_section = (!context_filter || board_enabled).then(|| Section {
        title: "Board Modes",
        rows: vec![
            row(
                binding_or_fallback(bindings, Action::ToggleWhiteboard, NOT_BOUND_LABEL),
                action_label(Action::ToggleWhiteboard),
            ),
            row(
                binding_or_fallback(bindings, Action::ToggleBlackboard, NOT_BOUND_LABEL),
                action_label(Action::ToggleBlackboard),
            ),
            row(
                binding_or_fallback(bindings, Action::ReturnToTransparent, NOT_BOUND_LABEL),
                action_label(Action::ReturnToTransparent),
            ),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_settings),
    });

    let pages_section = Section {
        title: "Pages",
        rows: vec![
            row(
                binding_or_fallback(bindings, Action::PagePrev, NOT_BOUND_LABEL),
                action_label(Action::PagePrev),
            ),
            row(
                binding_or_fallback(bindings, Action::PageNext, NOT_BOUND_LABEL),
                action_label(Action::PageNext),
            ),
            row(
                binding_or_fallback(bindings, Action::PageNew, NOT_BOUND_LABEL),
                action_label(Action::PageNew),
            ),
            row(
                binding_or_fallback(bindings, Action::PageDuplicate, NOT_BOUND_LABEL),
                action_label(Action::PageDuplicate),
            ),
            row(
                binding_or_fallback(bindings, Action::PageDelete, NOT_BOUND_LABEL),
                action_label(Action::PageDelete),
            ),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_file),
    };

    let color_badges = vec![
        color_badge(bindings, Action::SetColorRed, "R", [0.95, 0.41, 0.38]),
        color_badge(bindings, Action::SetColorGreen, "G", [0.46, 0.82, 0.45]),
        color_badge(bindings, Action::SetColorBlue, "B", [0.32, 0.58, 0.92]),
        color_badge(bindings, Action::SetColorYellow, "Y", [0.98, 0.80, 0.10]),
        color_badge(bindings, Action::SetColorOrange, "O", [0.98, 0.55, 0.26]),
        color_badge(bindings, Action::SetColorPink, "P", [0.78, 0.47, 0.96]),
        color_badge(bindings, Action::SetColorWhite, "W", [0.90, 0.92, 0.96]),
        color_badge(bindings, Action::SetColorBlack, "K", [0.28, 0.30, 0.38]),
    ];

    let drawing_section = Section {
        title: "Drawing",
        rows: vec![
            row(
                binding_or_fallback(bindings, Action::SelectPenTool, NOT_BOUND_LABEL),
                action_label(Action::SelectPenTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectLineTool, "Shift+Drag"),
                action_label(Action::SelectLineTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectRectTool, "Ctrl+Drag"),
                action_label(Action::SelectRectTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectEllipseTool, "Tab+Drag"),
                action_label(Action::SelectEllipseTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectArrowTool, "Ctrl+Shift+Drag"),
                action_label(Action::SelectArrowTool),
            ),
            row(
                binding_or_fallback(bindings, Action::ToggleHighlightTool, NOT_BOUND_LABEL),
                action_label(Action::ToggleHighlightTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectMarkerTool, NOT_BOUND_LABEL),
                action_label(Action::SelectMarkerTool),
            ),
            row(
                binding_or_fallback(bindings, Action::SelectEraserTool, NOT_BOUND_LABEL),
                action_label(Action::SelectEraserTool),
            ),
            row(
                bindings_or_fallback(
                    bindings,
                    &[Action::IncreaseThickness, Action::DecreaseThickness],
                    NOT_BOUND_LABEL,
                ),
                "Adjust thickness",
            ),
        ],
        badges: color_badges.clone(),
        icon: Some(toolbar_icons::draw_icon_pen),
    };

    let selection_section = Section {
        title: "Selection",
        rows: vec![
            row("Drag", "Selection tool"),
            row(
                binding_or_fallback(bindings, Action::SelectAll, NOT_BOUND_LABEL),
                action_label(Action::SelectAll),
            ),
            row(
                binding_or_fallback(bindings, Action::DuplicateSelection, NOT_BOUND_LABEL),
                action_label(Action::DuplicateSelection),
            ),
            row(
                binding_or_fallback(bindings, Action::CopySelection, NOT_BOUND_LABEL),
                action_label(Action::CopySelection),
            ),
            row(
                binding_or_fallback(bindings, Action::PasteSelection, NOT_BOUND_LABEL),
                action_label(Action::PasteSelection),
            ),
            row(
                binding_or_fallback(bindings, Action::DeleteSelection, NOT_BOUND_LABEL),
                action_label(Action::DeleteSelection),
            ),
            row(
                binding_or_fallback(bindings, Action::ToggleSelectionProperties, NOT_BOUND_LABEL),
                action_label(Action::ToggleSelectionProperties),
            ),
            row(
                bindings_or_fallback(
                    bindings,
                    &[Action::IncreaseFontSize, Action::DecreaseFontSize],
                    NOT_BOUND_LABEL,
                ),
                "Adjust font size",
            ),
        ],
        badges: color_badges.clone(),
        icon: Some(toolbar_icons::draw_icon_select),
    };

    let pen_text_section = Section {
        title: "Pen & Text",
        rows: vec![
            row(
                binding_or_fallback(bindings, Action::EnterTextMode, NOT_BOUND_LABEL),
                action_label(Action::EnterTextMode),
            ),
            row(
                binding_or_fallback(bindings, Action::EnterStickyNoteMode, NOT_BOUND_LABEL),
                action_label(Action::EnterStickyNoteMode),
            ),
            row(
                bindings_or_fallback(
                    bindings,
                    &[Action::IncreaseFontSize, Action::DecreaseFontSize],
                    NOT_BOUND_LABEL,
                ),
                "Adjust font size",
            ),
            row(
                binding_or_fallback(bindings, Action::ToggleFill, NOT_BOUND_LABEL),
                action_label(Action::ToggleFill),
            ),
            row("Selection properties panel", "Text background"),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_text),
    };

    let zoom_section = Section {
        title: "Zoom",
        rows: vec![
            row(
                bindings_or_fallback(
                    bindings,
                    &[Action::ZoomIn, Action::ZoomOut],
                    NOT_BOUND_LABEL,
                ),
                "Zoom in/out",
            ),
            row(
                binding_or_fallback(bindings, Action::ResetZoom, NOT_BOUND_LABEL),
                action_label(Action::ResetZoom),
            ),
            row(
                binding_or_fallback(bindings, Action::ToggleZoomLock, NOT_BOUND_LABEL),
                action_label(Action::ToggleZoomLock),
            ),
            row("Middle drag / arrow keys", "Pan view"),
            row(
                binding_or_fallback(bindings, Action::RefreshZoomCapture, NOT_BOUND_LABEL),
                action_label(Action::RefreshZoomCapture),
            ),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_zoom_in),
    };

    let mut action_rows = vec![
        row(
            binding_or_fallback(bindings, Action::ClearCanvas, NOT_BOUND_LABEL),
            action_label(Action::ClearCanvas),
        ),
        row(
            binding_or_fallback(bindings, Action::Undo, NOT_BOUND_LABEL),
            action_label(Action::Undo),
        ),
        row(
            binding_or_fallback(bindings, Action::ToggleClickHighlight, NOT_BOUND_LABEL),
            action_label(Action::ToggleClickHighlight),
        ),
        row(
            match joined_labels(bindings, &[Action::OpenContextMenu]) {
                Some(label) => format!("Right Click / {label}"),
                None => "Right Click".to_string(),
            },
            action_label(Action::OpenContextMenu),
        ),
        row(
            binding_or_fallback(bindings, Action::Exit, NOT_BOUND_LABEL),
            action_label(Action::Exit),
        ),
        row(
            binding_or_fallback(bindings, Action::ToggleHelp, NOT_BOUND_LABEL),
            action_label(Action::ToggleHelp),
        ),
        row(
            binding_or_fallback(bindings, Action::ToggleToolbar, NOT_BOUND_LABEL),
            action_label(Action::ToggleToolbar),
        ),
        row(
            binding_or_fallback(bindings, Action::TogglePresenterMode, NOT_BOUND_LABEL),
            action_label(Action::TogglePresenterMode),
        ),
        row(
            binding_or_fallback(bindings, Action::OpenConfigurator, NOT_BOUND_LABEL),
            action_label(Action::OpenConfigurator),
        ),
        row(
            binding_or_fallback(bindings, Action::ToggleStatusBar, NOT_BOUND_LABEL),
            action_label(Action::ToggleStatusBar),
        ),
    ];
    if frozen_enabled {
        action_rows.push(row(
            binding_or_fallback(bindings, Action::ToggleFrozenMode, NOT_BOUND_LABEL),
            action_label(Action::ToggleFrozenMode),
        ));
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
            row(
                binding_or_fallback(bindings, Action::CaptureClipboardFull, NOT_BOUND_LABEL),
                "Full screen \u{2192} clipboard",
            ),
            row(
                binding_or_fallback(bindings, Action::CaptureFileFull, NOT_BOUND_LABEL),
                "Full screen \u{2192} file",
            ),
            row(
                binding_or_fallback(bindings, Action::CaptureClipboardSelection, NOT_BOUND_LABEL),
                "Region \u{2192} clipboard",
            ),
            row(
                binding_or_fallback(bindings, Action::CaptureFileSelection, NOT_BOUND_LABEL),
                "Region \u{2192} file",
            ),
            row(
                binding_or_fallback(bindings, Action::CaptureActiveWindow, NOT_BOUND_LABEL),
                "Active window (Hyprland)",
            ),
            row(
                binding_or_fallback(bindings, Action::CaptureSelection, NOT_BOUND_LABEL),
                "Selection (capture defaults)",
            ),
            row(
                binding_or_fallback(bindings, Action::OpenCaptureFolder, NOT_BOUND_LABEL),
                action_label(Action::OpenCaptureFolder),
            ),
        ],
        badges: Vec::new(),
        icon: Some(toolbar_icons::draw_icon_save),
    });

    let mut all_sections = Vec::new();
    all_sections.push(drawing_section.clone());
    all_sections.push(actions_section.clone());
    if let Some(section) = board_modes_section.clone() {
        all_sections.push(section);
    }
    all_sections.push(pen_text_section.clone());
    all_sections.push(zoom_section.clone());
    all_sections.push(selection_section.clone());
    all_sections.push(pages_section.clone());
    if let Some(section) = screenshots_section.clone() {
        all_sections.push(section);
    }

    let mut page1_sections = Vec::new();
    page1_sections.push(drawing_section);
    page1_sections.push(actions_section);
    if let Some(section) = board_modes_section {
        page1_sections.push(section);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty_bindings() -> HelpOverlayBindings {
        HelpOverlayBindings {
            labels: HashMap::new(),
            cache_key: String::new(),
        }
    }

    #[test]
    fn gesture_hints_remain_present() {
        let bindings = empty_bindings();
        let sections = build_section_sets(&bindings, false, false, true, true).all;
        let rows: Vec<(&str, &str)> = sections
            .iter()
            .flat_map(|section| section.rows.iter())
            .map(|row| (row.key.as_str(), row.action))
            .collect();

        let expected = [
            ("Shift+Drag", action_label(Action::SelectLineTool)),
            ("Ctrl+Drag", action_label(Action::SelectRectTool)),
            ("Tab+Drag", action_label(Action::SelectEllipseTool)),
            ("Ctrl+Shift+Drag", action_label(Action::SelectArrowTool)),
            ("Drag", "Selection tool"),
            ("Selection properties panel", "Text background"),
            ("Middle drag / arrow keys", "Pan view"),
        ];

        for (key, action) in expected {
            assert!(
                rows.iter()
                    .any(|(row_key, row_action)| *row_key == key && *row_action == action),
                "Missing gesture hint row: '{key}' -> '{action}'"
            );
        }
    }
}
