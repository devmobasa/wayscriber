use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use super::ids;

mod definitions;
mod order;
pub use definitions::toolbar_item_definitions;
pub use order::{
    ResolvedToolbarOrder, ToolbarItemOrderConfig, ToolbarItemOrderGroup,
    toolbar_item_id_in_order_group, toolbar_item_order_group,
};

/// User-authored item-level toolbar customization.
///
/// The raw strings are intentionally preserved so unknown IDs from future
/// versions survive unrelated toolbar saves.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolbarItemsConfig {
    #[serde(default)]
    pub hidden: Vec<String>,
    /// IDs explicitly shown, overriding the layout-mode baseline (e.g.
    /// presets kept visible in simple mode). Raw strings preserve unknown
    /// future IDs, mirroring `hidden`.
    #[serde(default)]
    pub shown: Vec<String>,
    #[serde(default)]
    pub order: ToolbarItemOrderConfig,
}

/// Rebuild a raw-id list without `id`, deduping known ids and preserving
/// unknown raw strings.
fn drain_without(raw_ids: Vec<String>, id: ToolbarItemId) -> Vec<String> {
    let mut next = Vec::with_capacity(raw_ids.len());
    let mut seen_known = BTreeSet::new();
    for raw in raw_ids {
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
    next
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
            shown: Vec::new(),
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

        let mut shown = BTreeSet::new();
        let mut unknown_shown = Vec::new();
        for raw in &self.shown {
            match raw.parse::<ToolbarItemId>() {
                Ok(id) => {
                    shown.insert(id);
                }
                Err(_) => unknown_shown.push(raw.clone()),
            }
        }

        ResolvedToolbarItems {
            hidden,
            shown,
            unknown_hidden,
            unknown_shown,
            order: self.order.resolved(),
        }
    }

    pub fn set_hidden(&mut self, id: ToolbarItemId, hidden: bool) {
        self.hidden = drain_without(std::mem::take(&mut self.hidden), id);
        self.shown = drain_without(std::mem::take(&mut self.shown), id);
        if hidden {
            self.hidden.push(id.as_str().to_string());
        } else if super::visibility::section_flag_for_item(id).is_some() {
            // Section-level ids have a layout-mode baseline that may hide
            // them again; an explicit "shown" entry pins them visible.
            self.shown.push(id.as_str().to_string());
        }
    }

    /// Replace the explicit visibility setting for one known item while
    /// preserving unknown/future ids in both raw lists.
    pub(crate) fn set_visibility_setting(
        &mut self,
        id: ToolbarItemId,
        setting: ToolbarItemVisibilitySetting,
    ) -> bool {
        let before_hidden = self.hidden.clone();
        let before_shown = self.shown.clone();
        self.hidden = drain_without(std::mem::take(&mut self.hidden), id);
        self.shown = drain_without(std::mem::take(&mut self.shown), id);
        match setting {
            ToolbarItemVisibilitySetting::Default => {}
            ToolbarItemVisibilitySetting::Hidden => {
                self.hidden.push(id.as_str().to_string());
            }
            ToolbarItemVisibilitySetting::Shown => {
                self.shown.push(id.as_str().to_string());
            }
        }
        self.hidden != before_hidden || self.shown != before_shown
    }

    pub fn reset_known_hidden_to_defaults(&mut self) -> bool {
        let original_hidden = self.hidden.clone();
        let original_shown = self.shown.clone();
        let mut next: Vec<String> = DEFAULT_HIDDEN_TOOLBAR_ITEM_IDS
            .iter()
            .map(|id| id.as_str().to_string())
            .collect();

        for raw in self.hidden.drain(..) {
            if raw.parse::<ToolbarItemId>().is_err() {
                next.push(raw);
            }
        }
        let mut next_shown = Vec::new();
        for raw in self.shown.drain(..) {
            if raw.parse::<ToolbarItemId>().is_err() {
                next_shown.push(raw);
            }
        }

        let changed = next != original_hidden || next_shown != original_shown;
        self.hidden = next;
        self.shown = next_shown;
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

    pub(crate) fn set_known_order(
        &mut self,
        group: ToolbarItemOrderGroup,
        ids: &[ToolbarItemId],
    ) -> bool {
        self.order.set_known_group_order(group, ids)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedToolbarItems {
    pub hidden: BTreeSet<ToolbarItemId>,
    /// Explicit "keep visible" overrides that beat the layout-mode baseline.
    pub shown: BTreeSet<ToolbarItemId>,
    pub unknown_hidden: Vec<String>,
    pub unknown_shown: Vec<String>,
    pub order: ResolvedToolbarOrder,
}

impl ResolvedToolbarItems {
    pub fn is_hidden(&self, id: ToolbarItemId) -> bool {
        self.hidden.contains(&id)
    }
}

/// The explicit replacement setting for one toolbar item. `Default` means
/// the authored/factory baseline decides visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ToolbarItemVisibilitySetting {
    Default,
    Hidden,
    Shown,
}

pub(crate) fn item_visibility_setting(
    resolved: &ResolvedToolbarItems,
    id: ToolbarItemId,
) -> ToolbarItemVisibilitySetting {
    if resolved.hidden.contains(&id) {
        ToolbarItemVisibilitySetting::Hidden
    } else if resolved.shown.contains(&id) {
        ToolbarItemVisibilitySetting::Shown
    } else {
        ToolbarItemVisibilitySetting::Default
    }
}

static FACTORY_INDIVIDUAL_VISIBILITY_SETTINGS: LazyLock<
    BTreeMap<ToolbarItemId, ToolbarItemVisibilitySetting>,
> = LazyLock::new(|| {
    let factory = ToolbarItemsConfig::default().resolved();
    resettable_individual_toolbar_item_ids()
        .map(|id| (id, item_visibility_setting(&factory, id)))
        .collect()
});

/// Built-in visibility settings for the exact individual-item reset batch.
/// This is initialized once and shared by availability, live mutation, and
/// runtime persistence so those layers cannot drift.
pub(crate) fn factory_individual_toolbar_item_visibility_settings()
-> &'static BTreeMap<ToolbarItemId, ToolbarItemVisibilitySetting> {
    &FACTORY_INDIVIDUAL_VISIBILITY_SETTINGS
}

/// Canonical visibility-customization predicate shared by the settings UI,
/// runtime-state seed builder, and factory reset implementation.
pub(crate) fn toolbar_item_visibility_override_allowed(definition: &ToolbarItemDefinition) -> bool {
    definition.group != Some(ToolbarGroupId::Settings)
        && definition.id != ids::SIDE_GROUP_SETTINGS
        && definition.id != ids::TOP_CHROME_OVERFLOW
}

/// Individual toolbar items affected by "Restore built-in visibility".
/// Named section settings intentionally remain outside this reset.
pub(crate) fn resettable_individual_toolbar_item_ids()
-> impl Iterator<Item = ToolbarItemId> + 'static {
    toolbar_item_definitions()
        .iter()
        .filter(|definition| toolbar_item_visibility_override_allowed(definition))
        .filter(|definition| super::visibility::section_flag_for_item(definition.id).is_none())
        .map(|definition| definition.id)
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

#[cfg(test)]
mod tests;
