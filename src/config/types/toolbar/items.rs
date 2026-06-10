use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// User-authored item-level toolbar customization.
///
/// The raw strings are intentionally preserved so unknown IDs from future
/// versions survive unrelated toolbar saves.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolbarItemsConfig {
    #[serde(default)]
    pub hidden: Vec<String>,
}

const DEFAULT_HIDDEN_TOOLBAR_ITEM_IDS: &[&str] = &["top.utility.screenshot"];

impl Default for ToolbarItemsConfig {
    fn default() -> Self {
        Self {
            hidden: DEFAULT_HIDDEN_TOOLBAR_ITEM_IDS
                .iter()
                .map(|id| (*id).to_string())
                .collect(),
        }
    }
}

impl ToolbarItemsConfig {
    pub fn resolved(&self) -> ResolvedToolbarItems {
        let mut hidden = BTreeSet::new();
        let mut unknown_hidden = Vec::new();

        for raw in &self.hidden {
            match raw.parse::<ToolbarItemId>() {
                Ok(id) => {
                    hidden.insert(id);
                }
                Err(_) => unknown_hidden.push(raw.clone()),
            }
        }

        ResolvedToolbarItems {
            hidden,
            unknown_hidden,
        }
    }

    #[allow(dead_code)]
    pub fn set_hidden(&mut self, id: ToolbarItemId, hidden: bool) {
        let mut next = Vec::with_capacity(self.hidden.len() + usize::from(hidden));
        let mut seen_known = BTreeSet::new();

        for raw in self.hidden.drain(..) {
            match raw.parse::<ToolbarItemId>() {
                Ok(existing) if existing == id => {}
                Ok(existing) => {
                    if seen_known.insert(existing) {
                        next.push(existing.as_str().to_string());
                    }
                }
                Err(_) => next.push(raw),
            }
        }

        if hidden {
            next.push(id.as_str().to_string());
        }

        self.hidden = next;
    }

    pub fn reset_known_hidden_to_defaults(&mut self) -> bool {
        let original = self.hidden.clone();
        let mut next: Vec<String> = DEFAULT_HIDDEN_TOOLBAR_ITEM_IDS
            .iter()
            .map(|id| (*id).to_string())
            .collect();

        for raw in self.hidden.drain(..) {
            if raw.parse::<ToolbarItemId>().is_err() {
                next.push(raw);
            }
        }

        let changed = next != original;
        self.hidden = next;
        changed
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedToolbarItems {
    pub hidden: BTreeSet<ToolbarItemId>,
    pub unknown_hidden: Vec<String>,
}

impl ResolvedToolbarItems {
    pub fn is_hidden(&self, id: ToolbarItemId) -> bool {
        self.hidden.contains(&id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ToolbarItemId(&'static str);

impl ToolbarItemId {
    pub(crate) const fn from_known(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for ToolbarItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl FromStr for ToolbarItemId {
    type Err = UnknownToolbarItemId;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim();
        toolbar_item_definitions()
            .iter()
            .find(|definition| definition.id.as_str() == normalized)
            .map(|definition| definition.id)
            .ok_or(UnknownToolbarItemId)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownToolbarItemId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolbarItemDefinition {
    pub id: ToolbarItemId,
    pub label: &'static str,
    pub surface: ToolbarItemSurface,
    pub category: ToolbarItemCategory,
    pub group: Option<ToolbarGroupId>,
}

impl ToolbarItemDefinition {
    const fn new(
        id: &'static str,
        label: &'static str,
        surface: ToolbarItemSurface,
        category: ToolbarItemCategory,
        group: Option<ToolbarGroupId>,
    ) -> Self {
        Self {
            id: ToolbarItemId::from_known(id),
            label,
            surface,
            category,
            group,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolbarItemSurface {
    Top,
    Side,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolbarItemCategory {
    Chrome,
    Tool,
    Utility,
    Group,
    Action,
    Page,
    Board,
    Setting,
    Session,
    ToolOption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolbarGroupId {
    Colors,
    Thickness,
    EraserMode,
    PolygonSides,
    ArrowLabels,
    StepMarkers,
    StepUndo,
    MarkerOpacity,
    TextSize,
    Font,
    Actions,
    Pages,
    Boards,
    Presets,
    Settings,
    Session,
}

impl ToolbarGroupId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Colors => "colors",
            Self::Thickness => "thickness",
            Self::EraserMode => "eraser-mode",
            Self::PolygonSides => "polygon-sides",
            Self::ArrowLabels => "arrow-labels",
            Self::StepMarkers => "step-markers",
            Self::StepUndo => "step-undo",
            Self::MarkerOpacity => "marker-opacity",
            Self::TextSize => "text-size",
            Self::Font => "font",
            Self::Actions => "actions",
            Self::Pages => "pages",
            Self::Boards => "boards",
            Self::Presets => "presets",
            Self::Settings => "settings",
            Self::Session => "session",
        }
    }

    pub const fn toolbar_item_id(self) -> ToolbarItemId {
        ToolbarItemId::from_known(match self {
            Self::Colors => "side.group.colors",
            Self::Thickness => "side.group.thickness",
            Self::EraserMode => "side.group.eraser-mode",
            Self::PolygonSides => "side.group.polygon-sides",
            Self::ArrowLabels => "side.group.arrow-labels",
            Self::StepMarkers => "side.group.step-markers",
            Self::StepUndo => "side.group.step-undo",
            Self::MarkerOpacity => "side.group.marker-opacity",
            Self::TextSize => "side.group.text-size",
            Self::Font => "side.group.font",
            Self::Actions => "side.group.actions",
            Self::Pages => "side.group.pages",
            Self::Boards => "side.group.boards",
            Self::Presets => "side.group.presets",
            Self::Settings => "side.group.settings",
            Self::Session => "side.group.session",
        })
    }
}

impl fmt::Display for ToolbarGroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ToolbarGroupId {
    type Err = UnknownToolbarGroupId;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "colors" => Ok(Self::Colors),
            "thickness" => Ok(Self::Thickness),
            "eraser-mode" => Ok(Self::EraserMode),
            "polygon-sides" => Ok(Self::PolygonSides),
            "arrow-labels" => Ok(Self::ArrowLabels),
            "step-markers" => Ok(Self::StepMarkers),
            "step-undo" => Ok(Self::StepUndo),
            "marker-opacity" => Ok(Self::MarkerOpacity),
            "text-size" => Ok(Self::TextSize),
            "font" => Ok(Self::Font),
            "actions" => Ok(Self::Actions),
            "pages" => Ok(Self::Pages),
            "boards" => Ok(Self::Boards),
            "presets" => Ok(Self::Presets),
            "settings" => Ok(Self::Settings),
            "session" => Ok(Self::Session),
            _ => Err(UnknownToolbarGroupId),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownToolbarGroupId;

pub fn toolbar_item_definitions() -> &'static [ToolbarItemDefinition] {
    TOOLBAR_ITEM_DEFINITIONS
}

const TOOLBAR_ITEM_DEFINITIONS: &[ToolbarItemDefinition] = &[
    item("top.chrome.drag", "Move top toolbar", Top, Chrome, None),
    item("top.chrome.pin", "Pin top toolbar", Top, Chrome, None),
    item("top.chrome.close", "Close top toolbar", Top, Chrome, None),
    item("top.tool.select", "Select", Top, Tool, None),
    item("top.tool.pen", "Pen", Top, Tool, None),
    item("top.tool.marker", "Marker", Top, Tool, None),
    item("top.tool.step-marker", "Step marker", Top, Tool, None),
    item("top.tool.eraser", "Eraser", Top, Tool, None),
    item("top.tool.line", "Line", Top, Tool, None),
    item("top.tool.rect", "Rectangle", Top, Tool, None),
    item("top.tool.ellipse", "Ellipse", Top, Tool, None),
    item("top.tool.arrow", "Arrow", Top, Tool, None),
    item("top.tool.blur", "Blur", Top, Tool, None),
    item("top.tool.triangle", "Triangle", Top, Tool, None),
    item("top.tool.parallelogram", "Parallelogram", Top, Tool, None),
    item("top.tool.rhombus", "Rhombus", Top, Tool, None),
    item(
        "top.tool.regular-polygon",
        "Regular polygon",
        Top,
        Tool,
        None,
    ),
    item(
        "top.tool.freeform-polygon",
        "Freeform polygon",
        Top,
        Tool,
        None,
    ),
    item(
        "top.utility.shape-picker",
        "Shape picker",
        Top,
        Utility,
        None,
    ),
    item("top.utility.fill", "Fill", Top, Utility, None),
    item("top.utility.text", "Text", Top, Utility, None),
    item("top.utility.sticky-note", "Sticky note", Top, Utility, None),
    item(
        "top.utility.clear-canvas",
        "Clear canvas",
        Top,
        Utility,
        None,
    ),
    item("top.utility.screenshot", "Screenshot", Top, Utility, None),
    item("top.utility.highlight", "Highlight", Top, Utility, None),
    item(
        "top.utility.highlight-ring",
        "Highlight ring",
        Top,
        Utility,
        None,
    ),
    item(
        "top.utility.icon-mode-icons",
        "Use icons",
        Top,
        Utility,
        None,
    ),
    item(
        "top.utility.icon-mode-text",
        "Use text labels",
        Top,
        Utility,
        None,
    ),
    item(
        "side.group.colors",
        "Colors",
        Side,
        Group,
        Some(ToolbarGroupId::Colors),
    ),
    item(
        "side.group.thickness",
        "Thickness",
        Side,
        Group,
        Some(ToolbarGroupId::Thickness),
    ),
    item(
        "side.group.eraser-mode",
        "Eraser mode",
        Side,
        Group,
        Some(ToolbarGroupId::EraserMode),
    ),
    item(
        "side.group.polygon-sides",
        "Polygon sides",
        Side,
        Group,
        Some(ToolbarGroupId::PolygonSides),
    ),
    item(
        "side.group.arrow-labels",
        "Arrow labels",
        Side,
        Group,
        Some(ToolbarGroupId::ArrowLabels),
    ),
    item(
        "side.group.step-markers",
        "Step markers",
        Side,
        Group,
        Some(ToolbarGroupId::StepMarkers),
    ),
    item(
        "side.group.step-undo",
        "Step Undo/Redo",
        Side,
        Group,
        Some(ToolbarGroupId::StepUndo),
    ),
    item(
        "side.group.marker-opacity",
        "Marker opacity",
        Side,
        Group,
        Some(ToolbarGroupId::MarkerOpacity),
    ),
    item(
        "side.group.text-size",
        "Text size",
        Side,
        Group,
        Some(ToolbarGroupId::TextSize),
    ),
    item(
        "side.group.font",
        "Font",
        Side,
        Group,
        Some(ToolbarGroupId::Font),
    ),
    item(
        "side.group.actions",
        "Actions",
        Side,
        Group,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        "side.group.pages",
        "Pages",
        Side,
        Group,
        Some(ToolbarGroupId::Pages),
    ),
    item(
        "side.group.boards",
        "Boards",
        Side,
        Group,
        Some(ToolbarGroupId::Boards),
    ),
    item(
        "side.group.presets",
        "Presets",
        Side,
        Group,
        Some(ToolbarGroupId::Presets),
    ),
    item(
        "side.group.settings",
        "Settings",
        Side,
        Group,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.group.session",
        "Session",
        Side,
        Group,
        Some(ToolbarGroupId::Session),
    ),
    item(
        "side.actions.undo",
        "Undo",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        "side.actions.redo",
        "Redo",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        "side.actions.clear-canvas",
        "Clear canvas",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        "side.actions.zoom-in",
        "Zoom in",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        "side.actions.zoom-out",
        "Zoom out",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        "side.actions.reset-zoom",
        "Reset zoom",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        "side.actions.toggle-zoom-lock",
        "Toggle zoom lock",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        "side.actions.undo-all",
        "Undo all",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        "side.actions.redo-all",
        "Redo all",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        "side.actions.undo-all-delayed",
        "Delayed undo all",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        "side.actions.redo-all-delayed",
        "Delayed redo all",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        "side.actions.freeze",
        "Freeze",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        "side.pages.previous",
        "Previous page",
        Side,
        Page,
        Some(ToolbarGroupId::Pages),
    ),
    item(
        "side.pages.next",
        "Next page",
        Side,
        Page,
        Some(ToolbarGroupId::Pages),
    ),
    item(
        "side.pages.new",
        "New page",
        Side,
        Page,
        Some(ToolbarGroupId::Pages),
    ),
    item(
        "side.pages.duplicate",
        "Duplicate page",
        Side,
        Page,
        Some(ToolbarGroupId::Pages),
    ),
    item(
        "side.pages.delete",
        "Delete page",
        Side,
        Page,
        Some(ToolbarGroupId::Pages),
    ),
    item(
        "side.boards.previous",
        "Previous board",
        Side,
        Board,
        Some(ToolbarGroupId::Boards),
    ),
    item(
        "side.boards.next",
        "Next board",
        Side,
        Board,
        Some(ToolbarGroupId::Boards),
    ),
    item(
        "side.boards.new",
        "New board",
        Side,
        Board,
        Some(ToolbarGroupId::Boards),
    ),
    item(
        "side.boards.duplicate",
        "Duplicate board",
        Side,
        Board,
        Some(ToolbarGroupId::Boards),
    ),
    item(
        "side.boards.delete",
        "Delete board",
        Side,
        Board,
        Some(ToolbarGroupId::Boards),
    ),
    item(
        "side.boards.rename",
        "Rename board",
        Side,
        Board,
        Some(ToolbarGroupId::Boards),
    ),
    item(
        "side.settings.context-aware-ui",
        "Context UI",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.settings.text-controls",
        "Text controls",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.settings.status-bar",
        "Status bar",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.settings.status-board-badge",
        "Status board badge",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.settings.status-page-badge",
        "Status page badge",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.settings.floating-badge-always",
        "Floating board/page badge",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.settings.preset-toasts",
        "Preset toasts",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.settings.presets",
        "Presets toggle",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.settings.actions",
        "Actions toggle",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.settings.zoom-actions",
        "Zoom actions toggle",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.settings.advanced-actions",
        "Advanced actions toggle",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.settings.boards",
        "Boards toggle",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.settings.pages",
        "Pages toggle",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.settings.step-controls",
        "Step controls toggle",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.settings.configurator",
        "Open configurator",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.settings.config-file",
        "Open config file",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        "side.session.open",
        "Open session",
        Side,
        Session,
        Some(ToolbarGroupId::Session),
    ),
    item(
        "side.session.save-as",
        "Save session as",
        Side,
        Session,
        Some(ToolbarGroupId::Session),
    ),
    item(
        "side.session.info",
        "Session info",
        Side,
        Session,
        Some(ToolbarGroupId::Session),
    ),
    item(
        "side.session.clear",
        "Clear session",
        Side,
        Session,
        Some(ToolbarGroupId::Session),
    ),
    item(
        "side.session.manager",
        "Session manager",
        Side,
        Session,
        Some(ToolbarGroupId::Session),
    ),
    item(
        "side.tool-options.color",
        "Color",
        Side,
        ToolOption,
        Some(ToolbarGroupId::Colors),
    ),
    item(
        "side.tool-options.thickness",
        "Thickness",
        Side,
        ToolOption,
        Some(ToolbarGroupId::Thickness),
    ),
    item(
        "side.tool-options.marker-opacity",
        "Marker opacity",
        Side,
        ToolOption,
        Some(ToolbarGroupId::MarkerOpacity),
    ),
    item(
        "side.tool-options.eraser-mode",
        "Eraser mode",
        Side,
        ToolOption,
        Some(ToolbarGroupId::EraserMode),
    ),
    item(
        "side.tool-options.font-size",
        "Font size",
        Side,
        ToolOption,
        Some(ToolbarGroupId::TextSize),
    ),
    item(
        "side.tool-options.font-family",
        "Font family",
        Side,
        ToolOption,
        Some(ToolbarGroupId::Font),
    ),
    item(
        "side.tool-options.polygon-sides",
        "Polygon sides",
        Side,
        ToolOption,
        Some(ToolbarGroupId::PolygonSides),
    ),
    item(
        "side.tool-options.arrow-labels",
        "Arrow labels",
        Side,
        ToolOption,
        Some(ToolbarGroupId::ArrowLabels),
    ),
    item(
        "side.tool-options.step-marker-reset",
        "Reset step marker",
        Side,
        ToolOption,
        Some(ToolbarGroupId::StepMarkers),
    ),
];

const fn item(
    id: &'static str,
    label: &'static str,
    surface: ToolbarItemSurface,
    category: ToolbarItemCategory,
    group: Option<ToolbarGroupId>,
) -> ToolbarItemDefinition {
    ToolbarItemDefinition::new(id, label, surface, category, group)
}

use ToolbarItemCategory::{
    Action, Board, Chrome, Group, Page, Session, Setting, Tool, ToolOption, Utility,
};
use ToolbarItemSurface::{Side, Top};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_hidden_ids_resolve_and_unknown_ids_round_trip() {
        let config = ToolbarItemsConfig {
            hidden: vec![
                "side.actions.undo-all".to_string(),
                "future.toolbar.item".to_string(),
            ],
        };

        let resolved = config.resolved();

        assert!(resolved.is_hidden("side.actions.undo-all".parse().expect("known id")));
        assert_eq!(resolved.unknown_hidden, vec!["future.toolbar.item"]);
    }

    #[test]
    fn default_hidden_items_hide_screenshot_tool() {
        let resolved = ToolbarItemsConfig::default().resolved();

        assert!(resolved.is_hidden("top.utility.screenshot".parse().expect("known id")));
    }

    #[test]
    fn set_hidden_preserves_unknown_ids_while_mutating_known_ids() {
        let mut config = ToolbarItemsConfig {
            hidden: vec![
                "future.toolbar.item".to_string(),
                "side.actions.undo-all".to_string(),
                "side.actions.undo-all".to_string(),
                "side.pages.duplicate".to_string(),
            ],
        };

        config.set_hidden("side.actions.undo-all".parse().expect("known id"), false);
        config.set_hidden("top.tool.pen".parse().expect("known id"), true);

        assert_eq!(
            config.hidden,
            vec![
                "future.toolbar.item",
                "side.pages.duplicate",
                "top.tool.pen"
            ]
        );
    }

    #[test]
    fn reset_known_hidden_restores_defaults_and_preserves_unknown_ids() {
        let mut config = ToolbarItemsConfig {
            hidden: vec![
                "future.toolbar.item".to_string(),
                "side.actions.undo-all".to_string(),
            ],
        };

        assert!(config.reset_known_hidden_to_defaults());
        assert_eq!(
            config.hidden,
            vec!["top.utility.screenshot", "future.toolbar.item"]
        );
        assert!(!config.reset_known_hidden_to_defaults());
    }

    #[test]
    fn toolbar_group_ids_include_step_markers_and_step_undo() {
        assert_eq!(
            "step-markers".parse::<ToolbarGroupId>(),
            Ok(ToolbarGroupId::StepMarkers)
        );
        assert_eq!(
            "step-undo".parse::<ToolbarGroupId>(),
            Ok(ToolbarGroupId::StepUndo)
        );
    }

    #[test]
    fn toolbar_item_definitions_are_unique_parseable_and_labeled() {
        let mut seen = BTreeSet::new();

        for definition in toolbar_item_definitions() {
            assert!(
                seen.insert(definition.id.as_str()),
                "duplicate toolbar item id: {}",
                definition.id
            );
            assert_eq!(
                definition.id.as_str().parse::<ToolbarItemId>(),
                Ok(definition.id)
            );
            assert!(
                !definition.label.is_empty(),
                "missing toolbar item label: {}",
                definition.id
            );
        }
    }
}
