use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// User-authored item-level toolbar customization.
///
/// The raw strings are intentionally preserved so unknown IDs from future
/// versions survive unrelated toolbar saves.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolbarItemsConfig {
    #[serde(default)]
    pub hidden: Vec<String>,
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

    pub fn is_hidden(&self, id: ToolbarItemId) -> bool {
        self.resolved().is_hidden(id)
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
        KNOWN_TOOLBAR_ITEM_IDS
            .iter()
            .copied()
            .find(|known| *known == normalized)
            .map(Self)
            .ok_or(UnknownToolbarItemId)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownToolbarItemId;

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

const KNOWN_TOOLBAR_ITEM_IDS: &[&str] = &[
    "top.chrome.drag",
    "top.chrome.pin",
    "top.chrome.close",
    "top.tool.select",
    "top.tool.pen",
    "top.tool.marker",
    "top.tool.step-marker",
    "top.tool.eraser",
    "top.tool.line",
    "top.tool.rect",
    "top.tool.ellipse",
    "top.tool.arrow",
    "top.tool.blur",
    "top.tool.triangle",
    "top.tool.parallelogram",
    "top.tool.rhombus",
    "top.tool.regular-polygon",
    "top.tool.freeform-polygon",
    "top.utility.shape-picker",
    "top.utility.fill",
    "top.utility.text",
    "top.utility.sticky-note",
    "top.utility.clear-canvas",
    "top.utility.highlight",
    "top.utility.highlight-ring",
    "top.utility.icon-mode-icons",
    "top.utility.icon-mode-text",
    "side.group.colors",
    "side.group.thickness",
    "side.group.eraser-mode",
    "side.group.polygon-sides",
    "side.group.arrow-labels",
    "side.group.step-markers",
    "side.group.step-undo",
    "side.group.marker-opacity",
    "side.group.text-size",
    "side.group.font",
    "side.group.actions",
    "side.group.pages",
    "side.group.boards",
    "side.group.presets",
    "side.group.settings",
    "side.group.session",
    "side.actions.undo",
    "side.actions.redo",
    "side.actions.clear-canvas",
    "side.actions.zoom-in",
    "side.actions.zoom-out",
    "side.actions.reset-zoom",
    "side.actions.toggle-zoom-lock",
    "side.actions.undo-all",
    "side.actions.redo-all",
    "side.actions.undo-all-delayed",
    "side.actions.redo-all-delayed",
    "side.actions.freeze",
    "side.pages.previous",
    "side.pages.next",
    "side.pages.new",
    "side.pages.duplicate",
    "side.pages.delete",
    "side.boards.previous",
    "side.boards.next",
    "side.boards.new",
    "side.boards.duplicate",
    "side.boards.delete",
    "side.boards.rename",
    "side.settings.context-aware-ui",
    "side.settings.text-controls",
    "side.settings.status-bar",
    "side.settings.status-board-badge",
    "side.settings.status-page-badge",
    "side.settings.floating-badge-always",
    "side.settings.preset-toasts",
    "side.settings.presets",
    "side.settings.actions",
    "side.settings.zoom-actions",
    "side.settings.advanced-actions",
    "side.settings.boards",
    "side.settings.pages",
    "side.settings.step-controls",
    "side.settings.configurator",
    "side.settings.config-file",
    "side.session.open",
    "side.session.save-as",
    "side.session.info",
    "side.session.clear",
    "side.session.manager",
    "side.tool-options.color",
    "side.tool-options.thickness",
    "side.tool-options.marker-opacity",
    "side.tool-options.eraser-mode",
    "side.tool-options.font-size",
    "side.tool-options.font-family",
    "side.tool-options.polygon-sides",
    "side.tool-options.arrow-labels",
    "side.tool-options.step-marker-reset",
];

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
}
