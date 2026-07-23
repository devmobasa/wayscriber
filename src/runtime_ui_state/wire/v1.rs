use std::collections::{BTreeMap, BTreeSet};

use toml::{Table, Value};

use super::{RuntimeUiWireError, preserve_value, restore_value};
use crate::config::{
    ToolbarItemId, ToolbarItemOrderGroup, toolbar_item_definitions, toolbar_item_order_group,
};
use crate::runtime_ui_state::{
    InteractionSeedTarget, InteractionSeedValue, ItemVisibilitySetting, RuntimeOverride,
    RuntimeUiModel, RuntimeUiWireState, WirePassthrough,
};
use crate::ui::toolbar::{SidePane, ToolbarSideSection};

const TOOLBAR_SCALARS: [(&str, InteractionSeedTarget); 5] = [
    ("top_pinned", InteractionSeedTarget::TopPinned),
    ("side_pinned", InteractionSeedTarget::SidePinned),
    ("top_minimized", InteractionSeedTarget::TopMinimized),
    ("side_minimized", InteractionSeedTarget::SideMinimized),
    ("side_pane", InteractionSeedTarget::SidePane),
];

pub(super) fn decode(root: &mut Table) -> Result<RuntimeUiWireState, RuntimeUiWireError> {
    root.remove("version");
    let toolbar = take_optional_table(root, "toolbar")?;
    let boards = take_optional_table(root, "boards")?;
    let mut wire = RuntimeUiWireState::default();
    preserve_table(root, &mut wire.passthrough.top_level)?;
    decode_toolbar(toolbar, &mut wire)?;
    decode_boards(boards, &mut wire)?;
    Ok(wire)
}

fn decode_toolbar(
    mut toolbar: Table,
    wire: &mut RuntimeUiWireState,
) -> Result<(), RuntimeUiWireError> {
    for (field, target) in TOOLBAR_SCALARS {
        if let Some(value) = toolbar.remove(field) {
            decode_override(value, target, &mut wire.model, &mut wire.passthrough)?;
        }
    }
    decode_id_map(
        toolbar.remove("collapsed_sections"),
        |id| ToolbarSideSection::from_config_id(id).map(InteractionSeedTarget::CollapsedSection),
        &mut wire.model,
        &mut wire.passthrough,
    )?;
    decode_id_map(
        toolbar.remove("item_visibility"),
        |id| {
            id.parse::<ToolbarItemId>()
                .ok()
                .map(InteractionSeedTarget::ItemVisibility)
        },
        &mut wire.model,
        &mut wire.passthrough,
    )?;
    decode_id_map(
        toolbar.remove("item_order"),
        |id| order_group_from_wire_id(id).map(InteractionSeedTarget::ItemOrder),
        &mut wire.model,
        &mut wire.passthrough,
    )?;
    preserve_table(&toolbar, &mut wire.passthrough.toolbar)
}

fn decode_boards(
    mut boards: Table,
    wire: &mut RuntimeUiWireState,
) -> Result<(), RuntimeUiWireError> {
    decode_id_map(
        boards.remove("pinned"),
        |id| (!id.trim().is_empty()).then(|| InteractionSeedTarget::BoardPin(id.to_string())),
        &mut wire.model,
        &mut wire.passthrough,
    )?;
    preserve_table(&boards, &mut wire.passthrough.boards)
}

fn decode_id_map<F>(
    value: Option<Value>,
    mut target_for_id: F,
    model: &mut RuntimeUiModel,
    passthrough: &mut WirePassthrough,
) -> Result<(), RuntimeUiWireError>
where
    F: FnMut(&str) -> Option<InteractionSeedTarget>,
{
    let Some(value) = value else {
        return Ok(());
    };
    let Value::Table(entries) = value else {
        return Err(RuntimeUiWireError::new("recognized V1 map is not a table"));
    };
    for (id, value) in entries {
        let Some(target) = target_for_id(&id) else {
            continue;
        };
        decode_override(value, target, model, passthrough)?;
    }
    Ok(())
}

fn decode_override(
    value: Value,
    target: InteractionSeedTarget,
    model: &mut RuntimeUiModel,
    passthrough: &mut WirePassthrough,
) -> Result<(), RuntimeUiWireError> {
    let Value::Table(mut entry) = value else {
        return Err(RuntimeUiWireError::new(
            "recognized override is not a table",
        ));
    };
    let seed = entry
        .remove("seed")
        .ok_or_else(|| RuntimeUiWireError::new("recognized override omitted seed"))?;
    let value = entry
        .remove("value")
        .ok_or_else(|| RuntimeUiWireError::new("recognized override omitted value"))?;
    let seed = decode_value(&target, seed)?;
    let value = decode_value(&target, value)?;
    let mut extra = BTreeMap::new();
    preserve_table(&entry, &mut extra)?;
    if !extra.is_empty() {
        passthrough.entries.insert(target.clone(), extra);
    }
    model
        .insert_decoded(target, RuntimeOverride { seed, value })
        .map_err(|_| RuntimeUiWireError::new("recognized override has mismatched value type"))
}

fn decode_value(
    target: &InteractionSeedTarget,
    value: Value,
) -> Result<InteractionSeedValue, RuntimeUiWireError> {
    use InteractionSeedTarget as Target;
    match target {
        Target::TopPinned
        | Target::SidePinned
        | Target::TopMinimized
        | Target::SideMinimized
        | Target::CollapsedSection(_)
        | Target::BoardPin(_) => value
            .as_bool()
            .map(InteractionSeedValue::Bool)
            .ok_or_else(|| RuntimeUiWireError::new("boolean override has a non-boolean value")),
        Target::SidePane => value
            .as_str()
            .and_then(SidePane::from_config_id)
            .map(InteractionSeedValue::SidePane)
            .ok_or_else(|| RuntimeUiWireError::new("side pane override has an unknown value")),
        Target::ItemVisibility(_) => match value.as_str() {
            Some("default") => Ok(InteractionSeedValue::Visibility(
                ItemVisibilitySetting::Default,
            )),
            Some("hidden") => Ok(InteractionSeedValue::Visibility(
                ItemVisibilitySetting::Hidden,
            )),
            Some("shown") => Ok(InteractionSeedValue::Visibility(
                ItemVisibilitySetting::Shown,
            )),
            _ => Err(RuntimeUiWireError::new(
                "visibility override has an unknown value",
            )),
        },
        Target::ItemOrder(group) => decode_order(*group, value),
        Target::TopPosition | Target::SidePosition => Err(RuntimeUiWireError::new(
            "config positions cannot appear in runtime state",
        )),
    }
}

fn decode_order(
    group: ToolbarItemOrderGroup,
    value: Value,
) -> Result<InteractionSeedValue, RuntimeUiWireError> {
    let Value::Array(items) = value else {
        return Err(RuntimeUiWireError::new(
            "item order override is not an array",
        ));
    };
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for item in items {
        let Some(raw) = item.as_str() else {
            return Err(RuntimeUiWireError::new(
                "item order contains a non-string value",
            ));
        };
        let Ok(id) = raw.parse::<ToolbarItemId>() else {
            continue;
        };
        if item_belongs_to_group(id, group) && seen.insert(id) {
            result.push(id);
        }
    }
    Ok(InteractionSeedValue::ItemOrder(result))
}

pub(super) fn encode(wire: &RuntimeUiWireState) -> Result<Value, RuntimeUiWireError> {
    let mut root = restore_table(&wire.passthrough.top_level)?;
    root.insert("version".to_string(), Value::Integer(1));

    let mut toolbar = restore_table(&wire.passthrough.toolbar)?;
    let mut collapsed = Table::new();
    let mut visibility = Table::new();
    let mut order = Table::new();
    let mut boards_pinned = Table::new();

    for (target, runtime_override) in wire.model.iter() {
        let entry = encode_override(
            target,
            runtime_override,
            wire.passthrough.entries.get(target),
        )?;
        match target {
            InteractionSeedTarget::TopPinned => insert_unique(&mut toolbar, "top_pinned", entry)?,
            InteractionSeedTarget::SidePinned => insert_unique(&mut toolbar, "side_pinned", entry)?,
            InteractionSeedTarget::TopMinimized => {
                insert_unique(&mut toolbar, "top_minimized", entry)?
            }
            InteractionSeedTarget::SideMinimized => {
                insert_unique(&mut toolbar, "side_minimized", entry)?
            }
            InteractionSeedTarget::SidePane => insert_unique(&mut toolbar, "side_pane", entry)?,
            InteractionSeedTarget::CollapsedSection(section) => {
                insert_unique(&mut collapsed, section.config_id(), entry)?
            }
            InteractionSeedTarget::ItemVisibility(item) => {
                insert_unique(&mut visibility, item.as_str(), entry)?
            }
            InteractionSeedTarget::ItemOrder(group) => {
                insert_unique(&mut order, order_group_wire_id(*group), entry)?
            }
            InteractionSeedTarget::BoardPin(id) => insert_unique(&mut boards_pinned, id, entry)?,
            InteractionSeedTarget::TopPosition | InteractionSeedTarget::SidePosition => {
                return Err(RuntimeUiWireError::new(
                    "config position appeared in runtime-state model",
                ));
            }
        }
    }
    insert_unique(&mut toolbar, "collapsed_sections", Value::Table(collapsed))?;
    insert_unique(&mut toolbar, "item_visibility", Value::Table(visibility))?;
    insert_unique(&mut toolbar, "item_order", Value::Table(order))?;

    let mut boards = restore_table(&wire.passthrough.boards)?;
    insert_unique(&mut boards, "pinned", Value::Table(boards_pinned))?;
    insert_unique(&mut root, "toolbar", Value::Table(toolbar))?;
    insert_unique(&mut root, "boards", Value::Table(boards))?;
    Ok(Value::Table(root))
}

fn encode_override(
    target: &InteractionSeedTarget,
    runtime_override: &RuntimeOverride,
    extra: Option<&BTreeMap<String, String>>,
) -> Result<Value, RuntimeUiWireError> {
    if !runtime_override.seed.matches_target(target)
        || !runtime_override.value.matches_target(target)
    {
        return Err(RuntimeUiWireError::new(
            "override value does not match target",
        ));
    }
    let mut entry = extra.map_or_else(|| Ok(Table::new()), restore_table)?;
    insert_unique(
        &mut entry,
        "seed",
        encode_value(target, &runtime_override.seed)?,
    )?;
    insert_unique(
        &mut entry,
        "value",
        encode_value(target, &runtime_override.value)?,
    )?;
    Ok(Value::Table(entry))
}

fn encode_value(
    target: &InteractionSeedTarget,
    value: &InteractionSeedValue,
) -> Result<Value, RuntimeUiWireError> {
    match (target, value) {
        (
            InteractionSeedTarget::TopPinned
            | InteractionSeedTarget::SidePinned
            | InteractionSeedTarget::TopMinimized
            | InteractionSeedTarget::SideMinimized
            | InteractionSeedTarget::CollapsedSection(_)
            | InteractionSeedTarget::BoardPin(_),
            InteractionSeedValue::Bool(value),
        ) => Ok(Value::Boolean(*value)),
        (InteractionSeedTarget::SidePane, InteractionSeedValue::SidePane(value)) => {
            Ok(Value::String(value.config_id().to_string()))
        }
        (InteractionSeedTarget::ItemVisibility(_), InteractionSeedValue::Visibility(value)) => {
            Ok(Value::String(
                match value {
                    ItemVisibilitySetting::Default => "default",
                    ItemVisibilitySetting::Hidden => "hidden",
                    ItemVisibilitySetting::Shown => "shown",
                }
                .to_string(),
            ))
        }
        (InteractionSeedTarget::ItemOrder(group), InteractionSeedValue::ItemOrder(items)) => {
            if items
                .iter()
                .any(|item| !item_belongs_to_group(*item, *group))
            {
                return Err(RuntimeUiWireError::new(
                    "item order contains an item from another group",
                ));
            }
            Ok(Value::Array(
                items
                    .iter()
                    .map(|item| Value::String(item.as_str().to_string()))
                    .collect(),
            ))
        }
        _ => Err(RuntimeUiWireError::new(
            "override value does not match target",
        )),
    }
}

fn take_optional_table(root: &mut Table, field: &str) -> Result<Table, RuntimeUiWireError> {
    match root.remove(field) {
        None => Ok(Table::new()),
        Some(Value::Table(table)) => Ok(table),
        Some(_) => Err(RuntimeUiWireError::new(format!("{field} is not a table"))),
    }
}

fn preserve_table(
    source: &Table,
    destination: &mut BTreeMap<String, String>,
) -> Result<(), RuntimeUiWireError> {
    for (key, value) in source {
        destination.insert(key.clone(), preserve_value(value)?);
    }
    Ok(())
}

fn restore_table(source: &BTreeMap<String, String>) -> Result<Table, RuntimeUiWireError> {
    source
        .iter()
        .map(|(key, value)| Ok((key.clone(), restore_value(value)?)))
        .collect()
}

fn insert_unique(
    table: &mut Table,
    key: impl Into<String>,
    value: Value,
) -> Result<(), RuntimeUiWireError> {
    let key = key.into();
    if table.insert(key.clone(), value).is_some() {
        return Err(RuntimeUiWireError::new(format!(
            "passthrough conflicts with recognized V1 field {key}"
        )));
    }
    Ok(())
}

fn order_group_wire_id(group: ToolbarItemOrderGroup) -> &'static str {
    match group {
        ToolbarItemOrderGroup::TopTools => "top_tools",
        ToolbarItemOrderGroup::TopControls => "top_controls",
        ToolbarItemOrderGroup::SideSections => "side_sections",
        ToolbarItemOrderGroup::Actions => "actions",
        ToolbarItemOrderGroup::Pages => "pages",
        ToolbarItemOrderGroup::Boards => "boards",
        ToolbarItemOrderGroup::Presets => "presets",
        ToolbarItemOrderGroup::ToolOptions => "tool_options",
        ToolbarItemOrderGroup::Sessions => "sessions",
    }
}

fn order_group_from_wire_id(value: &str) -> Option<ToolbarItemOrderGroup> {
    match value {
        "top_tools" => Some(ToolbarItemOrderGroup::TopTools),
        "top_controls" => Some(ToolbarItemOrderGroup::TopControls),
        "side_sections" => Some(ToolbarItemOrderGroup::SideSections),
        "actions" => Some(ToolbarItemOrderGroup::Actions),
        "pages" => Some(ToolbarItemOrderGroup::Pages),
        "boards" => Some(ToolbarItemOrderGroup::Boards),
        "presets" => Some(ToolbarItemOrderGroup::Presets),
        "tool_options" => Some(ToolbarItemOrderGroup::ToolOptions),
        "sessions" => Some(ToolbarItemOrderGroup::Sessions),
        _ => None,
    }
}

fn item_belongs_to_group(id: ToolbarItemId, group: ToolbarItemOrderGroup) -> bool {
    toolbar_item_definitions()
        .iter()
        .find(|definition| definition.id == id)
        .and_then(toolbar_item_order_group)
        == Some(group)
}
