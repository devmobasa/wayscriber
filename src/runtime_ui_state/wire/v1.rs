use std::collections::{BTreeMap, BTreeSet};

use toml::{Table, Value};

use super::{RuntimeUiWireError, preserve_value, restore_value};
use crate::config::{
    ToolbarItemId, ToolbarItemOrderGroup, toolbar_item_definitions, toolbar_item_order_group,
};
use crate::runtime_ui_state::{
    InteractionSeedTarget, InteractionSeedValue, ItemVisibilitySetting, PersistedTopDisplayMode,
    RuntimeOverride, RuntimeUiModel, RuntimeUiWireState, ToolbarPositionSeed, WirePassthrough,
};
use crate::ui::toolbar::{SidePane, ToolbarSideSection};

/// Recognized `[toolbar]` overrides that carry a single scalar or table value.
///
/// `top_position`, `side_position`, and `top_display_mode` were added after
/// V1 shipped. They stay in V1 because an older build decodes them as unknown
/// keys and preserves them verbatim through `WirePassthrough`, whereas a
/// version bump would make that build treat the whole file as read-only.
const TOOLBAR_SCALARS: [(&str, InteractionSeedTarget); 8] = [
    ("top_pinned", InteractionSeedTarget::TopPinned),
    ("side_pinned", InteractionSeedTarget::SidePinned),
    ("top_minimized", InteractionSeedTarget::TopMinimized),
    ("side_minimized", InteractionSeedTarget::SideMinimized),
    ("side_pane", InteractionSeedTarget::SidePane),
    ("top_position", InteractionSeedTarget::TopPosition),
    ("side_position", InteractionSeedTarget::SidePosition),
    ("top_display_mode", InteractionSeedTarget::TopDisplayMode),
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
        Target::TopPosition | Target::SidePosition => decode_position(value),
        Target::TopDisplayMode => value
            .as_str()
            .and_then(PersistedTopDisplayMode::from_wire_id)
            .map(InteractionSeedValue::TopDisplayMode)
            .ok_or_else(|| {
                RuntimeUiWireError::new("top display mode override has an unknown value")
            }),
    }
}

fn decode_position(value: Value) -> Result<InteractionSeedValue, RuntimeUiWireError> {
    let Value::Table(mut position) = value else {
        return Err(RuntimeUiWireError::new("position override is not a table"));
    };
    let mut coordinate = |axis: &str| {
        position
            .remove(axis)
            .and_then(|value| match value {
                Value::Float(value) => Some(value),
                // A hand-written whole number parses as a TOML integer.
                Value::Integer(value) => Some(value as f64),
                _ => None,
            })
            .ok_or_else(|| {
                RuntimeUiWireError::new(format!("position override has no numeric {axis}"))
            })
    };
    let x = coordinate("x")?;
    let y = coordinate("y")?;
    if !position.is_empty() {
        return Err(RuntimeUiWireError::new(
            "position override has unknown coordinates",
        ));
    }
    // Non-finite offsets are rejected here for the same reason seeding rejects
    // them: they cannot be compared bit-exactly against an authored seed.
    ToolbarPositionSeed::new(x, y)
        .map(InteractionSeedValue::Position)
        .ok_or_else(|| RuntimeUiWireError::new("position override is not finite"))
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
            InteractionSeedTarget::TopPosition => {
                insert_unique(&mut toolbar, "top_position", entry)?
            }
            InteractionSeedTarget::SidePosition => {
                insert_unique(&mut toolbar, "side_position", entry)?
            }
            InteractionSeedTarget::TopDisplayMode => {
                insert_unique(&mut toolbar, "top_display_mode", entry)?
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
        (
            InteractionSeedTarget::TopPosition | InteractionSeedTarget::SidePosition,
            InteractionSeedValue::Position(position),
        ) => {
            let mut table = Table::new();
            table.insert("x".to_string(), Value::Float(position.x.get()));
            table.insert("y".to_string(), Value::Float(position.y.get()));
            Ok(Value::Table(table))
        }
        (InteractionSeedTarget::TopDisplayMode, InteractionSeedValue::TopDisplayMode(mode)) => {
            Ok(Value::String(mode.wire_id().to_string()))
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
