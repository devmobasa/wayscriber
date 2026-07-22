use super::*;

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

    pub(super) fn set_known_group_order(
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

    pub(super) fn reset_known_group_to_defaults(&mut self, group: ToolbarItemOrderGroup) -> bool {
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

impl ToolbarItemOrderGroup {
    pub(crate) const ALL: [Self; 9] = [
        Self::TopTools,
        Self::TopControls,
        Self::SideSections,
        Self::Actions,
        Self::Pages,
        Self::Boards,
        Self::Presets,
        Self::ToolOptions,
        Self::Sessions,
    ];
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
