use super::*;

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ToolbarItemOrderConfig {
    #[serde(default)]
    pub top_tools: Vec<String>,
    #[serde(default)]
    pub top_controls: Vec<String>,
}

impl ToolbarItemOrderConfig {
    pub fn resolved(&self) -> ResolvedToolbarOrder {
        ResolvedToolbarOrder {
            top_tools: resolve_order_group(ToolbarItemOrderGroup::TopTools, &self.top_tools),
            top_controls: resolve_order_group(
                ToolbarItemOrderGroup::TopControls,
                &self.top_controls,
            ),
        }
    }

    fn group_mut(&mut self, group: ToolbarItemOrderGroup) -> &mut Vec<String> {
        match group {
            ToolbarItemOrderGroup::TopTools => &mut self.top_tools,
            ToolbarItemOrderGroup::TopControls => &mut self.top_controls,
        }
    }

    fn group(&self, group: ToolbarItemOrderGroup) -> &[String] {
        match group {
            ToolbarItemOrderGroup::TopTools => &self.top_tools,
            ToolbarItemOrderGroup::TopControls => &self.top_controls,
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ResolvedToolbarOrderGroup {
    known: Vec<ToolbarItemId>,
    unknown: Vec<String>,
}

/// Live reorder groups for the unified top toolbar.
///
/// Panel-era order lists (`actions`, `pages`, `boards`, `presets`,
/// `tool_options`, `sessions`, `side_sections`) are retired config/runtime
/// keys: authored `config.toml` values stay in the raw document as
/// `RetiredSetting`, while `runtime-ui.toml` recognized-map entries are pruned
/// on rewrite like other unknown order IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolbarItemOrderGroup {
    TopTools,
    TopControls,
}

impl ToolbarItemOrderGroup {
    pub(crate) const ALL: [Self; 2] = [Self::TopTools, Self::TopControls];
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
        _ => None,
    }
}

fn top_control_orderable(id: ToolbarItemId) -> bool {
    DEFAULT_TOP_CONTROLS_ORDER.contains(&id)
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
    match group {
        ToolbarItemOrderGroup::TopTools => DEFAULT_TOP_TOOLS_ORDER.to_vec(),
        ToolbarItemOrderGroup::TopControls => DEFAULT_TOP_CONTROLS_ORDER.to_vec(),
    }
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
