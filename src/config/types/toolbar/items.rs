use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::ids;

/// User-authored item-level toolbar customization.
///
/// The raw strings are intentionally preserved so unknown IDs from future
/// versions survive unrelated toolbar saves.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolbarItemsConfig {
    #[serde(default)]
    pub hidden: Vec<String>,
    #[serde(default)]
    pub order: ToolbarItemOrderConfig,
}

const DEFAULT_HIDDEN_TOOLBAR_ITEM_IDS: &[ToolbarItemId] = &[ids::TOP_UTILITY_SCREENSHOT];

const DEFAULT_TOP_TOOLS_ORDER: &[ToolbarItemId] = &[
    ids::TOP_TOOL_SELECT,
    ids::TOP_TOOL_PEN,
    ids::TOP_TOOL_MARKER,
    ids::TOP_TOOL_STEP_MARKER,
    ids::TOP_TOOL_ERASER,
    ids::TOP_TOOL_LINE,
    ids::TOP_TOOL_RECT,
    ids::TOP_TOOL_ELLIPSE,
    ids::TOP_TOOL_ARROW,
    ids::TOP_TOOL_BLUR,
    ids::TOP_TOOL_TRIANGLE,
    ids::TOP_TOOL_PARALLELOGRAM,
    ids::TOP_TOOL_RHOMBUS,
    ids::TOP_TOOL_REGULAR_POLYGON,
    ids::TOP_TOOL_FREEFORM_POLYGON,
];

const DEFAULT_TOP_CONTROLS_ORDER: &[ToolbarItemId] = &[
    ids::TOP_UTILITY_TEXT,
    ids::TOP_UTILITY_STICKY_NOTE,
    ids::TOP_UTILITY_SCREENSHOT,
    ids::TOP_UTILITY_CLEAR_CANVAS,
    ids::TOP_UTILITY_HIGHLIGHT,
];

const DEFAULT_SIDE_SECTIONS_ORDER: &[ToolbarItemId] = &[
    ids::SIDE_GROUP_COLORS,
    ids::SIDE_GROUP_PRESETS,
    ids::SIDE_GROUP_THICKNESS,
    ids::SIDE_GROUP_ARROW_LABELS,
    ids::SIDE_GROUP_STEP_MARKERS,
    ids::SIDE_GROUP_MARKER_OPACITY,
    ids::SIDE_GROUP_TEXT_SIZE,
    ids::SIDE_GROUP_ACTIONS,
    ids::SIDE_GROUP_BOARDS,
    ids::SIDE_GROUP_PAGES,
    ids::SIDE_GROUP_STEP_UNDO,
    ids::SIDE_GROUP_SESSION,
    ids::SIDE_GROUP_SETTINGS,
];

impl Default for ToolbarItemsConfig {
    fn default() -> Self {
        Self {
            hidden: DEFAULT_HIDDEN_TOOLBAR_ITEM_IDS
                .iter()
                .map(|id| id.as_str().to_string())
                .collect(),
            order: ToolbarItemOrderConfig::default(),
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
            order: self.order.resolved(),
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
            .map(|id| id.as_str().to_string())
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

    pub fn move_item_by(
        &mut self,
        group: ToolbarItemOrderGroup,
        id: ToolbarItemId,
        delta: isize,
    ) -> bool {
        let order = self.order.resolved().ordered_ids(group).to_vec();
        let Some(current) = order.iter().position(|candidate| *candidate == id) else {
            return false;
        };
        let target = current
            .saturating_add_signed(delta)
            .min(order.len().saturating_sub(1));
        self.move_item_to_index(group, id, target)
    }

    pub fn move_item_to_index(
        &mut self,
        group: ToolbarItemOrderGroup,
        id: ToolbarItemId,
        target_index: usize,
    ) -> bool {
        if !toolbar_item_id_in_order_group(id, group) {
            return false;
        }

        let mut order = self.order.resolved().ordered_ids(group).to_vec();
        let Some(current) = order.iter().position(|candidate| *candidate == id) else {
            return false;
        };
        let item = order.remove(current);
        let target = target_index.min(order.len());
        order.insert(target, item);
        self.set_known_order(group, &order)
    }

    pub fn reset_known_order_to_defaults(&mut self, group: ToolbarItemOrderGroup) -> bool {
        self.order.reset_known_group_to_defaults(group)
    }

    fn set_known_order(&mut self, group: ToolbarItemOrderGroup, ids: &[ToolbarItemId]) -> bool {
        self.order.set_known_group_order(group, ids)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedToolbarItems {
    pub hidden: BTreeSet<ToolbarItemId>,
    pub unknown_hidden: Vec<String>,
    pub order: ResolvedToolbarOrder,
}

impl ResolvedToolbarItems {
    pub fn is_hidden(&self, id: ToolbarItemId) -> bool {
        self.hidden.contains(&id)
    }
}

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ToolbarItemOrderConfig {
    #[serde(default)]
    pub top_tools: Vec<String>,
    #[serde(default)]
    pub top_controls: Vec<String>,
    #[serde(default)]
    pub side_sections: Vec<String>,
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub pages: Vec<String>,
    #[serde(default)]
    pub boards: Vec<String>,
    #[serde(default)]
    pub presets: Vec<String>,
    #[serde(default)]
    pub tool_options: Vec<String>,
    #[serde(default)]
    pub sessions: Vec<String>,
}

impl ToolbarItemOrderConfig {
    pub fn resolved(&self) -> ResolvedToolbarOrder {
        ResolvedToolbarOrder {
            top_tools: resolve_order_group(ToolbarItemOrderGroup::TopTools, &self.top_tools),
            top_controls: resolve_order_group(
                ToolbarItemOrderGroup::TopControls,
                &self.top_controls,
            ),
            side_sections: resolve_order_group(
                ToolbarItemOrderGroup::SideSections,
                &self.side_sections,
            ),
            actions: resolve_order_group(ToolbarItemOrderGroup::Actions, &self.actions),
            pages: resolve_order_group(ToolbarItemOrderGroup::Pages, &self.pages),
            boards: resolve_order_group(ToolbarItemOrderGroup::Boards, &self.boards),
            presets: resolve_order_group(ToolbarItemOrderGroup::Presets, &self.presets),
            tool_options: resolve_order_group(
                ToolbarItemOrderGroup::ToolOptions,
                &self.tool_options,
            ),
            sessions: resolve_order_group(ToolbarItemOrderGroup::Sessions, &self.sessions),
        }
    }

    fn group_mut(&mut self, group: ToolbarItemOrderGroup) -> &mut Vec<String> {
        match group {
            ToolbarItemOrderGroup::TopTools => &mut self.top_tools,
            ToolbarItemOrderGroup::TopControls => &mut self.top_controls,
            ToolbarItemOrderGroup::SideSections => &mut self.side_sections,
            ToolbarItemOrderGroup::Actions => &mut self.actions,
            ToolbarItemOrderGroup::Pages => &mut self.pages,
            ToolbarItemOrderGroup::Boards => &mut self.boards,
            ToolbarItemOrderGroup::Presets => &mut self.presets,
            ToolbarItemOrderGroup::ToolOptions => &mut self.tool_options,
            ToolbarItemOrderGroup::Sessions => &mut self.sessions,
        }
    }

    fn group(&self, group: ToolbarItemOrderGroup) -> &[String] {
        match group {
            ToolbarItemOrderGroup::TopTools => &self.top_tools,
            ToolbarItemOrderGroup::TopControls => &self.top_controls,
            ToolbarItemOrderGroup::SideSections => &self.side_sections,
            ToolbarItemOrderGroup::Actions => &self.actions,
            ToolbarItemOrderGroup::Pages => &self.pages,
            ToolbarItemOrderGroup::Boards => &self.boards,
            ToolbarItemOrderGroup::Presets => &self.presets,
            ToolbarItemOrderGroup::ToolOptions => &self.tool_options,
            ToolbarItemOrderGroup::Sessions => &self.sessions,
        }
    }

    fn set_known_group_order(
        &mut self,
        group: ToolbarItemOrderGroup,
        ids: &[ToolbarItemId],
    ) -> bool {
        let original = self.group(group).to_vec();
        let mut next: Vec<String> = ids
            .iter()
            .copied()
            .filter(|id| toolbar_item_id_in_order_group(*id, group))
            .map(|id| id.as_str().to_string())
            .collect();
        append_preserved_order_strings(&original, group, &mut next);
        let changed = next != original;
        *self.group_mut(group) = next;
        changed
    }

    fn reset_known_group_to_defaults(&mut self, group: ToolbarItemOrderGroup) -> bool {
        let original = self.group(group).to_vec();
        let mut next = Vec::new();
        append_preserved_order_strings(&original, group, &mut next);
        let changed = next != original;
        *self.group_mut(group) = next;
        changed
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedToolbarOrder {
    top_tools: ResolvedToolbarOrderGroup,
    top_controls: ResolvedToolbarOrderGroup,
    side_sections: ResolvedToolbarOrderGroup,
    actions: ResolvedToolbarOrderGroup,
    pages: ResolvedToolbarOrderGroup,
    boards: ResolvedToolbarOrderGroup,
    presets: ResolvedToolbarOrderGroup,
    tool_options: ResolvedToolbarOrderGroup,
    sessions: ResolvedToolbarOrderGroup,
}

impl ResolvedToolbarOrder {
    pub fn ordered_ids(&self, group: ToolbarItemOrderGroup) -> &[ToolbarItemId] {
        &self.group(group).known
    }

    pub fn index_of(&self, group: ToolbarItemOrderGroup, id: ToolbarItemId) -> Option<usize> {
        self.ordered_ids(group)
            .iter()
            .position(|candidate| *candidate == id)
    }

    fn group(&self, group: ToolbarItemOrderGroup) -> &ResolvedToolbarOrderGroup {
        match group {
            ToolbarItemOrderGroup::TopTools => &self.top_tools,
            ToolbarItemOrderGroup::TopControls => &self.top_controls,
            ToolbarItemOrderGroup::SideSections => &self.side_sections,
            ToolbarItemOrderGroup::Actions => &self.actions,
            ToolbarItemOrderGroup::Pages => &self.pages,
            ToolbarItemOrderGroup::Boards => &self.boards,
            ToolbarItemOrderGroup::Presets => &self.presets,
            ToolbarItemOrderGroup::ToolOptions => &self.tool_options,
            ToolbarItemOrderGroup::Sessions => &self.sessions,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ResolvedToolbarOrderGroup {
    known: Vec<ToolbarItemId>,
    unknown: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolbarItemOrderGroup {
    TopTools,
    TopControls,
    SideSections,
    Actions,
    Pages,
    Boards,
    Presets,
    ToolOptions,
    Sessions,
}

pub fn toolbar_item_order_group(
    definition: &ToolbarItemDefinition,
) -> Option<ToolbarItemOrderGroup> {
    match (definition.surface, definition.category, definition.group) {
        (ToolbarItemSurface::Top, ToolbarItemCategory::Tool, _) => {
            Some(ToolbarItemOrderGroup::TopTools)
        }
        (ToolbarItemSurface::Top, ToolbarItemCategory::Utility, _)
            if top_control_orderable(definition.id) =>
        {
            Some(ToolbarItemOrderGroup::TopControls)
        }
        (_, ToolbarItemCategory::Group, Some(group)) if side_section_orderable(group) => {
            Some(ToolbarItemOrderGroup::SideSections)
        }
        (_, ToolbarItemCategory::Action, _) => Some(ToolbarItemOrderGroup::Actions),
        (_, ToolbarItemCategory::Page, _) => Some(ToolbarItemOrderGroup::Pages),
        (_, ToolbarItemCategory::Board, _) => Some(ToolbarItemOrderGroup::Boards),
        (_, ToolbarItemCategory::ToolOption, _) => Some(ToolbarItemOrderGroup::ToolOptions),
        (_, ToolbarItemCategory::Session, _) => Some(ToolbarItemOrderGroup::Sessions),
        (_, _, Some(ToolbarGroupId::Presets)) => Some(ToolbarItemOrderGroup::Presets),
        _ => None,
    }
}

fn top_control_orderable(id: ToolbarItemId) -> bool {
    DEFAULT_TOP_CONTROLS_ORDER.contains(&id)
}

fn side_section_orderable(group: ToolbarGroupId) -> bool {
    matches!(
        group,
        ToolbarGroupId::Colors
            | ToolbarGroupId::Thickness
            | ToolbarGroupId::ArrowLabels
            | ToolbarGroupId::StepMarkers
            | ToolbarGroupId::MarkerOpacity
            | ToolbarGroupId::TextSize
            | ToolbarGroupId::Actions
            | ToolbarGroupId::Pages
            | ToolbarGroupId::Boards
            | ToolbarGroupId::Presets
            | ToolbarGroupId::StepUndo
            | ToolbarGroupId::Session
            | ToolbarGroupId::Settings
    )
}

pub fn toolbar_item_id_in_order_group(id: ToolbarItemId, group: ToolbarItemOrderGroup) -> bool {
    toolbar_item_definitions()
        .iter()
        .find(|definition| definition.id == id)
        .and_then(toolbar_item_order_group)
        == Some(group)
}

fn resolve_order_group(group: ToolbarItemOrderGroup, raw: &[String]) -> ResolvedToolbarOrderGroup {
    let defaults = default_order_for_group(group);
    if raw.is_empty() {
        return ResolvedToolbarOrderGroup {
            known: defaults,
            unknown: Vec::new(),
        };
    }

    let mut known = Vec::with_capacity(defaults.len());
    let mut seen = BTreeSet::new();
    let mut unknown = Vec::new();
    for value in raw {
        match value.parse::<ToolbarItemId>() {
            Ok(id) if toolbar_item_id_in_order_group(id, group) => {
                if seen.insert(id) {
                    known.push(id);
                }
            }
            _ => unknown.push(value.clone()),
        }
    }
    for id in defaults {
        if seen.insert(id) {
            known.push(id);
        }
    }

    ResolvedToolbarOrderGroup { known, unknown }
}

fn default_order_for_group(group: ToolbarItemOrderGroup) -> Vec<ToolbarItemId> {
    let default_visual_order = match group {
        ToolbarItemOrderGroup::TopTools => Some(DEFAULT_TOP_TOOLS_ORDER),
        ToolbarItemOrderGroup::TopControls => Some(DEFAULT_TOP_CONTROLS_ORDER),
        ToolbarItemOrderGroup::SideSections => Some(DEFAULT_SIDE_SECTIONS_ORDER),
        _ => None,
    };
    if let Some(order) = default_visual_order {
        return order.to_vec();
    }

    toolbar_item_definitions()
        .iter()
        .filter(|definition| toolbar_item_order_group(definition) == Some(group))
        .map(|definition| definition.id)
        .collect()
}

fn append_preserved_order_strings(
    original: &[String],
    group: ToolbarItemOrderGroup,
    next: &mut Vec<String>,
) {
    for raw in original {
        if raw
            .parse::<ToolbarItemId>()
            .is_ok_and(|id| toolbar_item_id_in_order_group(id, group))
        {
            continue;
        }
        next.push(raw.clone());
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
        id: ToolbarItemId,
        label: &'static str,
        surface: ToolbarItemSurface,
        category: ToolbarItemCategory,
        group: Option<ToolbarGroupId>,
    ) -> Self {
        Self {
            id,
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
        match self {
            Self::Colors => ids::SIDE_GROUP_COLORS,
            Self::Thickness => ids::SIDE_GROUP_THICKNESS,
            Self::EraserMode => ids::SIDE_GROUP_ERASER_MODE,
            Self::PolygonSides => ids::SIDE_GROUP_POLYGON_SIDES,
            Self::ArrowLabels => ids::SIDE_GROUP_ARROW_LABELS,
            Self::StepMarkers => ids::SIDE_GROUP_STEP_MARKERS,
            Self::StepUndo => ids::SIDE_GROUP_STEP_UNDO,
            Self::MarkerOpacity => ids::SIDE_GROUP_MARKER_OPACITY,
            Self::TextSize => ids::SIDE_GROUP_TEXT_SIZE,
            Self::Font => ids::SIDE_GROUP_FONT,
            Self::Actions => ids::SIDE_GROUP_ACTIONS,
            Self::Pages => ids::SIDE_GROUP_PAGES,
            Self::Boards => ids::SIDE_GROUP_BOARDS,
            Self::Presets => ids::SIDE_GROUP_PRESETS,
            Self::Settings => ids::SIDE_GROUP_SETTINGS,
            Self::Session => ids::SIDE_GROUP_SESSION,
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

pub fn toolbar_item_definitions() -> &'static [ToolbarItemDefinition] {
    TOOLBAR_ITEM_DEFINITIONS
}

const TOOLBAR_ITEM_DEFINITIONS: &[ToolbarItemDefinition] = &[
    item(ids::TOP_CHROME_DRAG, "Move top toolbar", Top, Chrome, None),
    item(ids::TOP_CHROME_PIN, "Pin top toolbar", Top, Chrome, None),
    item(
        ids::TOP_CHROME_CLOSE,
        "Close top toolbar",
        Top,
        Chrome,
        None,
    ),
    item(ids::TOP_TOOL_SELECT, "Select", Top, Tool, None),
    item(ids::TOP_TOOL_PEN, "Pen", Top, Tool, None),
    item(ids::TOP_TOOL_MARKER, "Marker", Top, Tool, None),
    item(ids::TOP_TOOL_STEP_MARKER, "Step marker", Top, Tool, None),
    item(ids::TOP_TOOL_ERASER, "Eraser", Top, Tool, None),
    item(ids::TOP_TOOL_LINE, "Line", Top, Tool, None),
    item(ids::TOP_TOOL_RECT, "Rectangle", Top, Tool, None),
    item(ids::TOP_TOOL_ELLIPSE, "Ellipse", Top, Tool, None),
    item(ids::TOP_TOOL_ARROW, "Arrow", Top, Tool, None),
    item(ids::TOP_TOOL_BLUR, "Blur", Top, Tool, None),
    item(ids::TOP_TOOL_TRIANGLE, "Triangle", Top, Tool, None),
    item(
        ids::TOP_TOOL_PARALLELOGRAM,
        "Parallelogram",
        Top,
        Tool,
        None,
    ),
    item(ids::TOP_TOOL_RHOMBUS, "Rhombus", Top, Tool, None),
    item(
        ids::TOP_TOOL_REGULAR_POLYGON,
        "Regular polygon",
        Top,
        Tool,
        None,
    ),
    item(
        ids::TOP_TOOL_FREEFORM_POLYGON,
        "Freeform polygon",
        Top,
        Tool,
        None,
    ),
    item(
        ids::TOP_UTILITY_SHAPE_PICKER,
        "Shape picker",
        Top,
        Utility,
        None,
    ),
    item(ids::TOP_UTILITY_FILL, "Fill", Top, Utility, None),
    item(ids::TOP_UTILITY_TEXT, "Text", Top, Utility, None),
    item(
        ids::TOP_UTILITY_STICKY_NOTE,
        "Sticky note",
        Top,
        Utility,
        None,
    ),
    item(
        ids::TOP_UTILITY_CLEAR_CANVAS,
        "Clear canvas",
        Top,
        Utility,
        None,
    ),
    item(
        ids::TOP_UTILITY_SCREENSHOT,
        "Screenshot",
        Top,
        Utility,
        None,
    ),
    item(ids::TOP_UTILITY_HIGHLIGHT, "Highlight", Top, Utility, None),
    item(
        ids::TOP_UTILITY_HIGHLIGHT_RING,
        "Highlight ring",
        Top,
        Utility,
        None,
    ),
    item(
        ids::TOP_UTILITY_ICON_MODE_ICONS,
        "Use icons",
        Top,
        Utility,
        None,
    ),
    item(
        ids::TOP_UTILITY_ICON_MODE_TEXT,
        "Use text labels",
        Top,
        Utility,
        None,
    ),
    item(
        ids::SIDE_GROUP_COLORS,
        "Colors",
        Side,
        Group,
        Some(ToolbarGroupId::Colors),
    ),
    item(
        ids::SIDE_GROUP_THICKNESS,
        "Thickness",
        Side,
        Group,
        Some(ToolbarGroupId::Thickness),
    ),
    item(
        ids::SIDE_GROUP_ERASER_MODE,
        "Eraser mode",
        Side,
        Group,
        Some(ToolbarGroupId::EraserMode),
    ),
    item(
        ids::SIDE_GROUP_POLYGON_SIDES,
        "Polygon sides",
        Side,
        Group,
        Some(ToolbarGroupId::PolygonSides),
    ),
    item(
        ids::SIDE_GROUP_ARROW_LABELS,
        "Arrow labels",
        Side,
        Group,
        Some(ToolbarGroupId::ArrowLabels),
    ),
    item(
        ids::SIDE_GROUP_STEP_MARKERS,
        "Step markers",
        Side,
        Group,
        Some(ToolbarGroupId::StepMarkers),
    ),
    item(
        ids::SIDE_GROUP_STEP_UNDO,
        "Step Undo/Redo",
        Side,
        Group,
        Some(ToolbarGroupId::StepUndo),
    ),
    item(
        ids::SIDE_GROUP_MARKER_OPACITY,
        "Marker opacity",
        Side,
        Group,
        Some(ToolbarGroupId::MarkerOpacity),
    ),
    item(
        ids::SIDE_GROUP_TEXT_SIZE,
        "Text size",
        Side,
        Group,
        Some(ToolbarGroupId::TextSize),
    ),
    item(
        ids::SIDE_GROUP_FONT,
        "Font",
        Side,
        Group,
        Some(ToolbarGroupId::Font),
    ),
    item(
        ids::SIDE_GROUP_ACTIONS,
        "Actions",
        Side,
        Group,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        ids::SIDE_GROUP_PAGES,
        "Pages",
        Side,
        Group,
        Some(ToolbarGroupId::Pages),
    ),
    item(
        ids::SIDE_GROUP_BOARDS,
        "Boards",
        Side,
        Group,
        Some(ToolbarGroupId::Boards),
    ),
    item(
        ids::SIDE_GROUP_PRESETS,
        "Presets",
        Side,
        Group,
        Some(ToolbarGroupId::Presets),
    ),
    item(
        ids::SIDE_GROUP_SETTINGS,
        "Settings",
        Side,
        Group,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_GROUP_SESSION,
        "Session",
        Side,
        Group,
        Some(ToolbarGroupId::Session),
    ),
    item(
        ids::SIDE_ACTIONS_UNDO,
        "Undo",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        ids::SIDE_ACTIONS_REDO,
        "Redo",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        ids::SIDE_ACTIONS_CLEAR_CANVAS,
        "Clear canvas",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        ids::SIDE_ACTIONS_ZOOM_IN,
        "Zoom in",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        ids::SIDE_ACTIONS_ZOOM_OUT,
        "Zoom out",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        ids::SIDE_ACTIONS_RESET_ZOOM,
        "Reset zoom",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        ids::SIDE_ACTIONS_TOGGLE_ZOOM_LOCK,
        "Toggle zoom lock",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        ids::SIDE_ACTIONS_UNDO_ALL,
        "Undo all",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        ids::SIDE_ACTIONS_REDO_ALL,
        "Redo all",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        ids::SIDE_ACTIONS_UNDO_ALL_DELAYED,
        "Delayed undo all",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        ids::SIDE_ACTIONS_REDO_ALL_DELAYED,
        "Delayed redo all",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        ids::SIDE_ACTIONS_FREEZE,
        "Freeze",
        Side,
        Action,
        Some(ToolbarGroupId::Actions),
    ),
    item(
        ids::SIDE_PAGES_PREVIOUS,
        "Previous page",
        Side,
        Page,
        Some(ToolbarGroupId::Pages),
    ),
    item(
        ids::SIDE_PAGES_NEXT,
        "Next page",
        Side,
        Page,
        Some(ToolbarGroupId::Pages),
    ),
    item(
        ids::SIDE_PAGES_NEW,
        "New page",
        Side,
        Page,
        Some(ToolbarGroupId::Pages),
    ),
    item(
        ids::SIDE_PAGES_DUPLICATE,
        "Duplicate page",
        Side,
        Page,
        Some(ToolbarGroupId::Pages),
    ),
    item(
        ids::SIDE_PAGES_DELETE,
        "Delete page",
        Side,
        Page,
        Some(ToolbarGroupId::Pages),
    ),
    item(
        ids::SIDE_BOARDS_PREVIOUS,
        "Previous board",
        Side,
        Board,
        Some(ToolbarGroupId::Boards),
    ),
    item(
        ids::SIDE_BOARDS_NEXT,
        "Next board",
        Side,
        Board,
        Some(ToolbarGroupId::Boards),
    ),
    item(
        ids::SIDE_BOARDS_NEW,
        "New board",
        Side,
        Board,
        Some(ToolbarGroupId::Boards),
    ),
    item(
        ids::SIDE_BOARDS_DUPLICATE,
        "Duplicate board",
        Side,
        Board,
        Some(ToolbarGroupId::Boards),
    ),
    item(
        ids::SIDE_BOARDS_DELETE,
        "Delete board",
        Side,
        Board,
        Some(ToolbarGroupId::Boards),
    ),
    item(
        ids::SIDE_BOARDS_RENAME,
        "Rename board",
        Side,
        Board,
        Some(ToolbarGroupId::Boards),
    ),
    item(
        ids::SIDE_SETTINGS_CONTEXT_AWARE_UI,
        "Context UI",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SETTINGS_TEXT_CONTROLS,
        "Text controls",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SETTINGS_STATUS_BAR,
        "Status bar",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SETTINGS_STATUS_BOARD_BADGE,
        "Status board badge",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SETTINGS_STATUS_PAGE_BADGE,
        "Status page badge",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SETTINGS_FLOATING_BADGE_ALWAYS,
        "Floating board/page badge",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SETTINGS_PRESET_TOASTS,
        "Preset toasts",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SETTINGS_PRESETS,
        "Presets toggle",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SETTINGS_ACTIONS,
        "Actions toggle",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SETTINGS_ZOOM_ACTIONS,
        "Zoom actions toggle",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SETTINGS_ADVANCED_ACTIONS,
        "Advanced actions toggle",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SETTINGS_BOARDS,
        "Boards toggle",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SETTINGS_PAGES,
        "Pages toggle",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SETTINGS_STEP_CONTROLS,
        "Step controls toggle",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SETTINGS_CONFIGURATOR,
        "Open configurator",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SETTINGS_CONFIG_FILE,
        "Open config file",
        Side,
        Setting,
        Some(ToolbarGroupId::Settings),
    ),
    item(
        ids::SIDE_SESSION_OPEN,
        "Open session",
        Side,
        Session,
        Some(ToolbarGroupId::Session),
    ),
    item(
        ids::SIDE_SESSION_SAVE_AS,
        "Save session as",
        Side,
        Session,
        Some(ToolbarGroupId::Session),
    ),
    item(
        ids::SIDE_SESSION_INFO,
        "Session info",
        Side,
        Session,
        Some(ToolbarGroupId::Session),
    ),
    item(
        ids::SIDE_SESSION_CLEAR,
        "Clear session",
        Side,
        Session,
        Some(ToolbarGroupId::Session),
    ),
    item(
        ids::SIDE_SESSION_MANAGER,
        "Session manager",
        Side,
        Session,
        Some(ToolbarGroupId::Session),
    ),
    item(
        ids::SIDE_TOOL_OPTIONS_COLOR,
        "Color",
        Side,
        ToolOption,
        Some(ToolbarGroupId::Colors),
    ),
    item(
        ids::SIDE_TOOL_OPTIONS_THICKNESS,
        "Thickness",
        Side,
        ToolOption,
        Some(ToolbarGroupId::Thickness),
    ),
    item(
        ids::SIDE_TOOL_OPTIONS_MARKER_OPACITY,
        "Marker opacity",
        Side,
        ToolOption,
        Some(ToolbarGroupId::MarkerOpacity),
    ),
    item(
        ids::SIDE_TOOL_OPTIONS_ERASER_MODE,
        "Eraser mode",
        Side,
        ToolOption,
        Some(ToolbarGroupId::EraserMode),
    ),
    item(
        ids::SIDE_TOOL_OPTIONS_FONT_SIZE,
        "Font size",
        Side,
        ToolOption,
        Some(ToolbarGroupId::TextSize),
    ),
    item(
        ids::SIDE_TOOL_OPTIONS_FONT_FAMILY,
        "Font family",
        Side,
        ToolOption,
        Some(ToolbarGroupId::Font),
    ),
    item(
        ids::SIDE_TOOL_OPTIONS_POLYGON_SIDES,
        "Polygon sides",
        Side,
        ToolOption,
        Some(ToolbarGroupId::PolygonSides),
    ),
    item(
        ids::SIDE_TOOL_OPTIONS_ARROW_LABELS,
        "Arrow labels",
        Side,
        ToolOption,
        Some(ToolbarGroupId::ArrowLabels),
    ),
    item(
        ids::SIDE_TOOL_OPTIONS_STEP_MARKER_RESET,
        "Reset step marker",
        Side,
        ToolOption,
        Some(ToolbarGroupId::StepMarkers),
    ),
];

const fn item(
    id: ToolbarItemId,
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
                ids::SIDE_ACTIONS_UNDO_ALL.as_str().to_string(),
                "future.toolbar.item".to_string(),
            ],
            order: ToolbarItemOrderConfig::default(),
        };

        let resolved = config.resolved();

        assert!(resolved.is_hidden(ids::SIDE_ACTIONS_UNDO_ALL));
        assert_eq!(resolved.unknown_hidden, vec!["future.toolbar.item"]);
    }

    #[test]
    fn default_hidden_items_hide_screenshot_tool() {
        let resolved = ToolbarItemsConfig::default().resolved();

        assert!(resolved.is_hidden(ids::TOP_UTILITY_SCREENSHOT));
    }

    #[test]
    fn set_hidden_preserves_unknown_ids_while_mutating_known_ids() {
        let mut config = ToolbarItemsConfig {
            hidden: vec![
                "future.toolbar.item".to_string(),
                ids::SIDE_ACTIONS_UNDO_ALL.as_str().to_string(),
                ids::SIDE_ACTIONS_UNDO_ALL.as_str().to_string(),
                ids::SIDE_PAGES_DUPLICATE.as_str().to_string(),
            ],
            order: ToolbarItemOrderConfig::default(),
        };

        config.set_hidden(ids::SIDE_ACTIONS_UNDO_ALL, false);
        config.set_hidden(ids::TOP_TOOL_PEN, true);

        assert_eq!(
            config.hidden,
            vec![
                "future.toolbar.item".to_string(),
                ids::SIDE_PAGES_DUPLICATE.as_str().to_string(),
                ids::TOP_TOOL_PEN.as_str().to_string()
            ]
        );
    }

    #[test]
    fn reset_known_hidden_restores_defaults_and_preserves_unknown_ids() {
        let mut config = ToolbarItemsConfig {
            hidden: vec![
                "future.toolbar.item".to_string(),
                ids::SIDE_ACTIONS_UNDO_ALL.as_str().to_string(),
            ],
            order: ToolbarItemOrderConfig::default(),
        };

        assert!(config.reset_known_hidden_to_defaults());
        assert_eq!(
            config.hidden,
            vec![
                ids::TOP_UTILITY_SCREENSHOT.as_str().to_string(),
                "future.toolbar.item".to_string()
            ]
        );
        assert!(!config.reset_known_hidden_to_defaults());
    }

    #[test]
    fn default_order_matches_visual_toolbar_defaults() {
        let resolved = ToolbarItemsConfig::default().resolved();

        assert_eq!(
            resolved.order.ordered_ids(ToolbarItemOrderGroup::TopTools),
            DEFAULT_TOP_TOOLS_ORDER
        );
        assert_eq!(
            resolved
                .order
                .ordered_ids(ToolbarItemOrderGroup::TopControls),
            DEFAULT_TOP_CONTROLS_ORDER
        );
        assert_eq!(
            resolved
                .order
                .ordered_ids(ToolbarItemOrderGroup::SideSections),
            DEFAULT_SIDE_SECTIONS_ORDER
        );
    }

    #[test]
    fn item_order_moves_known_ids_and_preserves_unknown_ids() {
        let mut config = ToolbarItemsConfig {
            hidden: Vec::new(),
            order: ToolbarItemOrderConfig {
                top_tools: vec![
                    "future.toolbar.item".to_string(),
                    ids::TOP_TOOL_PEN.as_str().to_string(),
                    ids::TOP_TOOL_SELECT.as_str().to_string(),
                ],
                ..ToolbarItemOrderConfig::default()
            },
        };

        assert!(config.move_item_by(ToolbarItemOrderGroup::TopTools, ids::TOP_TOOL_PEN, 1,));

        assert_eq!(
            config.order.top_tools.last(),
            Some(&"future.toolbar.item".to_string())
        );
        assert_eq!(
            config
                .resolved()
                .order
                .ordered_ids(ToolbarItemOrderGroup::TopTools)[1],
            ids::TOP_TOOL_PEN
        );
    }

    #[test]
    fn top_control_order_excludes_visibility_only_utilities() {
        let config = ToolbarItemsConfig {
            hidden: Vec::new(),
            order: ToolbarItemOrderConfig {
                top_controls: vec![
                    ids::TOP_UTILITY_SHAPE_PICKER.as_str().to_string(),
                    ids::TOP_UTILITY_TEXT.as_str().to_string(),
                    ids::TOP_UTILITY_FILL.as_str().to_string(),
                ],
                ..ToolbarItemOrderConfig::default()
            },
        };

        let resolved = config.resolved();
        let ordered = resolved
            .order
            .ordered_ids(ToolbarItemOrderGroup::TopControls);
        assert_eq!(ordered[0], ids::TOP_UTILITY_TEXT);
        assert!(!ordered.contains(&ids::TOP_UTILITY_SHAPE_PICKER));
        assert!(!ordered.contains(&ids::TOP_UTILITY_FILL));
    }

    #[test]
    fn side_section_order_uses_runtime_representable_blocks() {
        let config = ToolbarItemsConfig {
            hidden: Vec::new(),
            order: ToolbarItemOrderConfig {
                side_sections: vec![
                    ids::SIDE_GROUP_FONT.as_str().to_string(),
                    ids::SIDE_GROUP_THICKNESS.as_str().to_string(),
                    ids::SIDE_GROUP_POLYGON_SIDES.as_str().to_string(),
                ],
                ..ToolbarItemOrderConfig::default()
            },
        };

        let resolved = config.resolved();
        let ordered = resolved
            .order
            .ordered_ids(ToolbarItemOrderGroup::SideSections);
        assert_eq!(ordered[0], ids::SIDE_GROUP_THICKNESS);
        assert!(!ordered.contains(&ids::SIDE_GROUP_FONT));
        assert!(!ordered.contains(&ids::SIDE_GROUP_POLYGON_SIDES));
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
