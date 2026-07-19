use std::collections::HashSet;

use anyhow::{Context, Result};
use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Key, Table, TableLike, Value};

use super::super::Config;
use crate::input::boards::BoundaryBoardIdSet;
use crate::render_profiles::normalize_profile_id;

pub(super) fn merge_config_document(
    source: &DocumentMut,
    previous: &Config,
    updated: &Config,
    known_document: &DocumentMut,
) -> Result<DocumentMut> {
    let updated_revision = updated.config_revision;
    let previous = serialize_config_document(previous)?;
    let updated = serialize_config_document(updated)?;
    let mut merged = source.clone();
    let source_was_empty = merged.as_table().is_empty();
    let empty_source_contents = source_was_empty.then(|| merged.trailing().clone());

    canonicalize_aliases(&mut merged);
    if !merged.contains_key("config_revision") && updated_revision > 0 {
        merged.insert("config_revision", toml_edit::value(updated_revision as i64));
    }
    merge_table_like(
        merged.as_table_mut(),
        Some(previous.as_table()),
        updated.as_table(),
        Some(known_document.as_table()),
        "",
    );
    if let Some(contents) = empty_source_contents
        && !merged.as_table().is_empty()
    {
        merged.as_table_mut().decor_mut().set_prefix(contents);
        merged.set_trailing("");
    }
    Ok(merged)
}

pub(super) fn serialize_config_document(config: &Config) -> Result<DocumentMut> {
    let text = toml::to_string_pretty(config).context("Failed to serialize config")?;
    text.parse::<DocumentMut>()
        .context("Failed to build editable config document")
}

pub(super) fn repair_source_document(
    source: &DocumentMut,
    previous: &Config,
    updated: &Config,
) -> Result<DocumentMut> {
    let previous = serialize_config_document(previous)?;
    let updated = serialize_config_document(updated)?;
    let mut repair_source = source.clone();
    canonicalize_aliases(&mut repair_source);
    remove_known_content(
        repair_source.as_table_mut(),
        Some(previous.as_table()),
        updated.as_table(),
    );
    Ok(repair_source)
}

pub(super) fn conservative_repair_source_document(
    source: &DocumentMut,
    previous: &Config,
    updated: &Config,
) -> Result<DocumentMut> {
    let previous = serialize_config_document(previous)?;
    let updated = serialize_config_document(updated)?;
    let mut repair_source = source.clone();
    canonicalize_aliases(&mut repair_source);
    for key in known_keys(Some(previous.as_table()), updated.as_table(), None) {
        repair_source.remove(&key);
    }
    Ok(repair_source)
}

fn remove_known_content(
    raw: &mut dyn TableLike,
    previous: Option<&dyn TableLike>,
    updated: &dyn TableLike,
) {
    for key in known_keys(previous, updated, None) {
        let previous_item = previous.and_then(|table| table.get(&key));
        let updated_item = updated.get(&key);
        let known_tables = previous_item
            .into_iter()
            .chain(updated_item)
            .filter_map(Item::as_table_like)
            .collect::<Vec<_>>();
        let all_known_items_are_tables = previous_item
            .into_iter()
            .chain(updated_item)
            .all(|item| item.as_table_like().is_some());

        if all_known_items_are_tables
            && let Some(raw_table) = raw.get_mut(&key).and_then(Item::as_table_like_mut)
        {
            let previous_table = previous_item.and_then(Item::as_table_like);
            let updated_table = updated_item
                .and_then(Item::as_table_like)
                .or_else(|| known_tables.first().copied())
                .expect("a known table-like item is available");
            remove_known_content(raw_table, previous_table, updated_table);
        } else {
            raw.remove(&key);
        }
    }
}

fn known_keys(
    previous: Option<&dyn TableLike>,
    updated: &dyn TableLike,
    known: Option<&dyn TableLike>,
) -> Vec<String> {
    let mut keys = previous
        .into_iter()
        .flat_map(TableLike::iter)
        .map(|(key, _)| key.to_string())
        .collect::<Vec<_>>();
    for (key, _) in updated.iter() {
        if !keys.iter().any(|known| known == key) {
            keys.push(key.to_string());
        }
    }
    for (key, _) in known.into_iter().flat_map(TableLike::iter) {
        if !keys.iter().any(|known| known == key) {
            keys.push(key.to_string());
        }
    }
    keys
}

fn canonicalize_aliases(document: &mut DocumentMut) {
    rename_key_at_path(
        document.as_table_mut(),
        &["ui"],
        "show_page_badge_with_status_bar",
        "show_floating_badge_always",
    );
    rename_key_at_path(
        document.as_table_mut(),
        &["render_profiles"],
        "items",
        "profiles",
    );
    rename_key_at_path(
        document.as_table_mut(),
        &["ui", "toolbar", "mode_overrides"],
        "full",
        "regular",
    );
}

fn rename_key_at_path(root: &mut Table, path: &[&str], alias: &str, canonical: &str) {
    let Some(table) = table_like_at_path_mut(root, path) else {
        return;
    };
    if table.contains_key(canonical) || !table.contains_key(alias) {
        return;
    }

    let names = table
        .iter()
        .map(|(name, _)| name.to_string())
        .collect::<Vec<_>>();
    let mut entries = Vec::with_capacity(names.len());
    for name in names {
        let Some(mut key) = table.key(&name).cloned() else {
            continue;
        };
        let Some(item) = table.remove(&name) else {
            continue;
        };
        if name == alias {
            let mut renamed = Key::new(canonical);
            *renamed.leaf_decor_mut() = key.leaf_decor().clone();
            *renamed.dotted_decor_mut() = key.dotted_decor().clone();
            key = renamed;
        }
        entries.push((key, item));
    }

    for (key, item) in entries {
        table.entry_format(&key).or_insert(item);
    }
}

fn table_like_at_path_mut<'a>(
    table: &'a mut dyn TableLike,
    path: &[&str],
) -> Option<&'a mut dyn TableLike> {
    let Some((head, tail)) = path.split_first() else {
        return Some(table);
    };
    let child = table.get_mut(head)?.as_table_like_mut()?;
    table_like_at_path_mut(child, tail)
}

fn merge_table_like(
    raw: &mut dyn TableLike,
    previous: Option<&dyn TableLike>,
    updated: &dyn TableLike,
    known: Option<&dyn TableLike>,
    path: &str,
) {
    for key in known_keys(previous, updated, known) {
        let previous_item = previous.and_then(|table| table.get(&key));
        let known_item = known.and_then(|table| table.get(&key));
        let item_path = if path.is_empty() {
            key.clone()
        } else {
            format!("{path}.{key}")
        };
        match updated.get(&key) {
            Some(updated_item) => match raw.get_mut(&key) {
                Some(raw_item) => merge_item(
                    raw_item,
                    previous_item,
                    updated_item,
                    known_item,
                    &item_path,
                ),
                None => {
                    if let Some(item) = changed_item(previous_item, updated_item, &item_path) {
                        raw.insert(&key, item);
                    }
                }
            },
            None if previous_item.is_some() || known_item.is_some() => {
                raw.remove(&key);
            }
            None => {}
        }
    }
}

fn changed_item(previous: Option<&Item>, updated: &Item, path: &str) -> Option<Item> {
    let Some(previous) = previous else {
        return Some(updated.clone());
    };
    if items_semantically_equal(previous, updated) {
        return None;
    }

    let (Some(previous_table), Some(updated_table)) =
        (previous.as_table_like(), updated.as_table_like())
    else {
        return Some(updated.clone());
    };
    let mut changed = updated.clone();
    let changed_table = changed
        .as_table_like_mut()
        .expect("a cloned table-like item remains table-like");
    changed_table.clear();
    merge_table_like(
        changed_table,
        Some(previous_table),
        updated_table,
        None,
        path,
    );
    (!changed_table.is_empty()).then_some(changed)
}

fn items_semantically_equal(left: &Item, right: &Item) -> bool {
    if let (Some(left), Some(right)) = (left.as_table_like(), right.as_table_like()) {
        return tables_semantically_equal(left, right);
    }
    if let (Some(left), Some(right)) = (left.as_array_of_tables(), right.as_array_of_tables()) {
        return left.len() == right.len()
            && left
                .iter()
                .zip(right.iter())
                .all(|(left, right)| tables_semantically_equal(left, right));
    }
    match (left.as_value(), right.as_value()) {
        (Some(left), Some(right)) => values_semantically_equal(left, right),
        _ => left.to_string() == right.to_string(),
    }
}

fn tables_semantically_equal(left: &dyn TableLike, right: &dyn TableLike) -> bool {
    left.len() == right.len()
        && left.iter().all(|(key, left_item)| {
            right
                .get(key)
                .is_some_and(|right_item| items_semantically_equal(left_item, right_item))
        })
}

fn merge_item(
    raw: &mut Item,
    previous: Option<&Item>,
    updated: &Item,
    known: Option<&Item>,
    path: &str,
) {
    if let (Item::Value(Value::Array(raw)), Item::ArrayOfTables(updated)) = (&mut *raw, updated) {
        let previous = previous
            .and_then(Item::as_array_of_tables)
            .cloned()
            .map(ArrayOfTables::into_array);
        let updated = updated.clone().into_array();
        if merge_inline_array_of_tables(raw, previous.as_ref(), &updated, path) {
            return;
        }
    }

    if let (Some(raw_table), Some(updated_table)) =
        (raw.as_table_like_mut(), updated.as_table_like())
    {
        merge_table_like(
            raw_table,
            previous.and_then(Item::as_table_like),
            updated_table,
            known.and_then(Item::as_table_like),
            path,
        );
        return;
    }

    match (raw, updated) {
        (Item::ArrayOfTables(raw), Item::ArrayOfTables(updated)) => merge_array_of_tables(
            raw,
            previous.and_then(Item::as_array_of_tables),
            updated,
            path,
        ),
        (Item::Value(raw), Item::Value(updated)) => {
            merge_value(
                raw,
                previous.and_then(Item::as_value),
                updated,
                known.and_then(Item::as_value),
                path,
            );
        }
        (raw, updated) => replace_item_preserving_decor(raw, updated),
    }
}

fn merge_value(
    raw: &mut Value,
    previous: Option<&Value>,
    updated: &Value,
    known: Option<&Value>,
    path: &str,
) {
    match (raw, updated) {
        (Value::InlineTable(raw), Value::InlineTable(updated)) => merge_table_like(
            raw,
            previous
                .and_then(Value::as_inline_table)
                .map(|table| table as _),
            updated,
            known
                .and_then(Value::as_inline_table)
                .map(|table| table as _),
            path,
        ),
        (Value::Array(raw), Value::Array(updated)) => {
            let previous = previous.and_then(Value::as_array);
            while raw.len() > updated.len() {
                raw.remove(raw.len() - 1);
            }
            for index in 0..updated.len() {
                let updated_value = updated
                    .get(index)
                    .expect("array index is bounded by updated length");
                if let Some(raw_value) = raw.get_mut(index) {
                    merge_value(
                        raw_value,
                        previous.and_then(|values| values.get(index)),
                        updated_value,
                        known
                            .and_then(Value::as_array)
                            .and_then(|values| values.get(index)),
                        path,
                    );
                } else {
                    raw.push_formatted(updated_value.clone());
                }
            }
        }
        (raw, updated) => {
            if previous.is_some_and(|previous| values_semantically_equal(previous, updated))
                && (values_semantically_equal(raw, updated)
                    || known_alternate_representation_is_equal(raw, updated, path))
            {
                return;
            }
            let decor = raw.decor().clone();
            *raw = updated.clone();
            *raw.decor_mut() = decor;
        }
    }
}

fn known_alternate_representation_is_equal(raw: &Value, updated: &Value, path: &str) -> bool {
    if !matches!(
        path,
        "boards.items.background" | "boards.items.default_pen_color"
    ) {
        return false;
    }

    let Some(raw_rgb) = board_rgb_array(raw) else {
        return false;
    };
    let Some(updated_rgb) = board_rgb_array(updated) else {
        return false;
    };
    raw_rgb.len() == updated_rgb.len()
        && raw_rgb
            .iter()
            .zip(updated_rgb.iter())
            .all(|(raw, updated)| values_semantically_equal(raw, updated))
}

fn board_rgb_array(value: &Value) -> Option<&Array> {
    match value {
        Value::Array(rgb) => Some(rgb),
        Value::InlineTable(map) => map.get("rgb").and_then(Value::as_array),
        _ => None,
    }
}

fn merge_array_of_tables(
    raw: &mut ArrayOfTables,
    previous: Option<&ArrayOfTables>,
    updated: &ArrayOfTables,
    path: &str,
) {
    let raw_tables = raw.clone().into_iter().collect::<Vec<_>>();
    let raw_positions = raw_tables.iter().map(Table::position).collect::<Vec<_>>();
    let group_position = raw_positions.iter().flatten().copied().min();
    let previous_tables = previous
        .map(|tables| tables.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    let updated_tables = updated.iter().collect::<Vec<_>>();
    let raw_table_refs = raw_tables
        .iter()
        .map(|table| table as &dyn TableLike)
        .collect::<Vec<_>>();
    let previous_table_refs = previous_tables
        .iter()
        .map(|table| *table as &dyn TableLike)
        .collect::<Vec<_>>();
    let updated_table_refs = updated_tables
        .iter()
        .map(|table| *table as &dyn TableLike)
        .collect::<Vec<_>>();
    let matches = plan_table_merges(
        &raw_table_refs,
        &previous_table_refs,
        &updated_table_refs,
        path,
    );
    let mut last_raw_index = None;
    let preserve_entry_positions = matches.raw_for_updated.iter().flatten().all(|index| {
        let preserves_order = last_raw_index.is_none_or(|last| *index > last);
        last_raw_index = Some(*index);
        preserves_order
    });
    let mut available = raw_tables.into_iter().map(Some).collect::<Vec<_>>();
    let mut rebuilt = ArrayOfTables::new();

    for (index, updated_table) in updated_tables.iter().copied().enumerate() {
        let previous_table = matches.updated_previous[index]
            .and_then(|index| previous_tables.get(index).copied())
            .map(|table| table as _);
        let raw_index = matches.raw_for_updated[index];
        let original_table = raw_index
            .and_then(|index| available.get(index))
            .and_then(Option::as_ref)
            .cloned();
        let mut merged_table = raw_index
            .and_then(|index| available.get_mut(index))
            .and_then(Option::take)
            .unwrap_or_else(|| updated_table.clone());

        merge_table_like(&mut merged_table, previous_table, updated_table, None, path);
        let position = if preserve_entry_positions {
            retained_table_position(
                index,
                &matches.raw_for_updated,
                &raw_positions,
                group_position,
            )
        } else {
            group_position
        };
        if !preserve_entry_positions || raw_index.is_none() {
            set_table_tree_position(&mut merged_table, position);
        } else if let Some(original_table) = original_table.as_ref() {
            position_new_table_children(&mut merged_table, original_table);
        }
        rebuilt.push(merged_table);
    }
    for index in matches.preserved_raw {
        if let Some(mut table) = available.get_mut(index).and_then(Option::take) {
            let position = preserve_entry_positions
                .then(|| raw_positions.get(index).copied().flatten())
                .flatten()
                .or(group_position);
            if !preserve_entry_positions {
                set_table_tree_position(&mut table, position);
            }
            rebuilt.push(table);
        }
    }

    *raw = rebuilt;
}

fn retained_table_position(
    updated_index: usize,
    raw_for_updated: &[Option<usize>],
    raw_positions: &[Option<isize>],
    group_position: Option<isize>,
) -> Option<isize> {
    raw_for_updated[updated_index]
        .and_then(|index| raw_positions.get(index).copied().flatten())
        .or_else(|| {
            raw_for_updated[..updated_index]
                .iter()
                .rev()
                .flatten()
                .find_map(|index| raw_positions.get(*index).copied().flatten())
        })
        .or_else(|| {
            raw_for_updated[updated_index + 1..]
                .iter()
                .flatten()
                .find_map(|index| raw_positions.get(*index).copied().flatten())
        })
        .or(group_position)
}

struct TableMergePlan {
    updated_previous: Vec<Option<usize>>,
    raw_for_updated: Vec<Option<usize>>,
    preserved_raw: Vec<usize>,
}

fn plan_table_merges(
    raw: &[&dyn TableLike],
    previous: &[&dyn TableLike],
    updated: &[&dyn TableLike],
    path: &str,
) -> TableMergePlan {
    let updated_previous = match_table_indices(updated, previous, path);
    let previous_raw = match_table_indices(previous, raw, path);
    let raw_for_updated = updated_previous
        .iter()
        .map(|previous_index| {
            previous_index
                .and_then(|index| previous_raw.get(index))
                .copied()
                .flatten()
        })
        .collect();
    let preserved_raw = (0..raw.len())
        .filter(|index| {
            !previous_raw
                .iter()
                .flatten()
                .any(|matched| matched == index)
        })
        .collect();
    TableMergePlan {
        updated_previous,
        raw_for_updated,
        preserved_raw,
    }
}

fn match_table_indices(
    targets: &[&dyn TableLike],
    candidates: &[&dyn TableLike],
    path: &str,
) -> Vec<Option<usize>> {
    let target_ids = table_id_keys(targets, path);
    let candidate_ids = table_id_keys(candidates, path);
    let mut matches = vec![None; targets.len()];
    let mut candidate_used = vec![false; candidates.len()];

    for (target_index, target_id) in target_ids.iter().enumerate() {
        let Some(target_id) = target_id else {
            continue;
        };
        let target_occurrences = target_ids
            .iter()
            .filter(|candidate| candidate.as_ref() == Some(target_id))
            .count();
        let candidate_indices = candidate_ids
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                (candidate.as_ref() == Some(target_id)).then_some(index)
            })
            .collect::<Vec<_>>();
        if target_occurrences == 1 && candidate_indices.len() == 1 {
            let candidate_index = candidate_indices[0];
            matches[target_index] = Some(candidate_index);
            candidate_used[candidate_index] = true;
        }
    }

    for (target_index, target) in targets.iter().enumerate() {
        if matches[target_index].is_some() {
            continue;
        }
        let candidate_indices = candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                (!candidate_used[index] && tables_semantically_equal(*target, *candidate))
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        if candidate_indices.len() == 1 {
            let candidate_index = candidate_indices[0];
            matches[target_index] = Some(candidate_index);
            candidate_used[candidate_index] = true;
        }
    }

    let unmatched_targets = matches
        .iter()
        .enumerate()
        .filter_map(|(index, matched)| matched.is_none().then_some(index))
        .collect::<Vec<_>>();
    let unmatched_candidates = candidate_used
        .iter()
        .enumerate()
        .filter_map(|(index, used)| (!used).then_some(index));
    for (target_index, candidate_index) in unmatched_targets.into_iter().zip(unmatched_candidates) {
        matches[target_index] = Some(candidate_index);
    }
    matches
}

fn table_id_keys(tables: &[&dyn TableLike], path: &str) -> Vec<Option<String>> {
    match path {
        "render_profiles.profiles" => normalized_render_profile_ids(tables),
        "boards.items" => normalized_board_ids(tables),
        _ => tables
            .iter()
            .map(|table| literal_table_id(*table))
            .collect(),
    }
}

fn normalized_render_profile_ids(tables: &[&dyn TableLike]) -> Vec<Option<String>> {
    let mut seen = HashSet::new();
    tables
        .iter()
        .enumerate()
        .map(|(index, table)| {
            let raw_id = table.get("id")?.as_value()?.as_str()?;
            let mut id = normalize_profile_id(raw_id);
            if id.is_empty() {
                id = format!("profile-{}", index + 1);
            }
            let base = id.clone();
            let mut suffix = 2;
            while seen.contains(&id) {
                id = format!("{base}-{suffix}");
                suffix += 1;
            }
            seen.insert(id.clone());
            Some(id)
        })
        .collect()
}

fn normalized_board_ids(tables: &[&dyn TableLike]) -> Vec<Option<String>> {
    let mut seen = BoundaryBoardIdSet::new();
    tables
        .iter()
        .enumerate()
        .map(|(index, table)| {
            let raw_id = table.get("id")?.as_value()?.as_str()?;
            Some(seen.normalize_unique(raw_id, index).value)
        })
        .collect()
}

fn literal_table_id(table: &dyn TableLike) -> Option<String> {
    let id = table.get("id")?.as_value()?.as_str()?.trim();
    if id.is_empty() {
        return None;
    }
    Some(id.to_string())
}

fn merge_inline_array_of_tables(
    raw: &mut Array,
    previous: Option<&Array>,
    updated: &Array,
    path: &str,
) -> bool {
    let raw_values = raw.iter().cloned().collect::<Vec<_>>();
    let previous_values = previous
        .map(|array| array.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    let updated_values = updated.iter().collect::<Vec<_>>();
    let Some(raw_tables) = inline_tables(&raw_values) else {
        return false;
    };
    let Some(previous_tables) = inline_table_refs(&previous_values) else {
        return false;
    };
    let Some(updated_tables) = inline_table_refs(&updated_values) else {
        return false;
    };

    let matches = plan_table_merges(&raw_tables, &previous_tables, &updated_tables, path);
    let mut available = raw_values.into_iter().map(Some).collect::<Vec<_>>();
    let mut rebuilt = Vec::with_capacity(updated_values.len() + matches.preserved_raw.len());

    for (index, updated_value) in updated_values.iter().copied().enumerate() {
        let previous_table =
            matches.updated_previous[index].and_then(|index| previous_tables.get(index).copied());
        let mut merged_value = matches.raw_for_updated[index]
            .and_then(|index| available.get_mut(index))
            .and_then(Option::take)
            .unwrap_or_else(|| updated_value.clone());
        let merged_table = merged_value
            .as_inline_table_mut()
            .expect("validated inline-table array contains inline tables");
        merge_table_like(
            merged_table,
            previous_table,
            updated_tables[index],
            None,
            path,
        );
        rebuilt.push(merged_value);
    }
    for index in matches.preserved_raw {
        if let Some(value) = available.get_mut(index).and_then(Option::take) {
            rebuilt.push(value);
        }
    }

    raw.clear();
    for value in rebuilt {
        raw.push_formatted(value);
    }
    true
}

fn inline_tables(values: &[Value]) -> Option<Vec<&dyn TableLike>> {
    values
        .iter()
        .map(|value| value.as_inline_table().map(|table| table as &dyn TableLike))
        .collect()
}

fn inline_table_refs<'a>(values: &'a [&'a Value]) -> Option<Vec<&'a dyn TableLike>> {
    values
        .iter()
        .map(|value| value.as_inline_table().map(|table| table as &dyn TableLike))
        .collect()
}

fn set_table_tree_position(table: &mut Table, position: Option<isize>) {
    table.set_position(position);
    for (_, item) in table.iter_mut() {
        match item {
            Item::Table(table) => set_table_tree_position(table, position),
            Item::ArrayOfTables(tables) => {
                for table in tables.iter_mut() {
                    set_table_tree_position(table, position);
                }
            }
            Item::None | Item::Value(_) => {}
        }
    }
}

fn position_new_table_children(table: &mut Table, original: &Table) {
    let parent_position = table.position();
    for (key, item) in table.iter_mut() {
        match item {
            Item::Table(child) => {
                if let Some(original_child) = original.get(key.get()).and_then(Item::as_table) {
                    position_new_table_children(child, original_child);
                } else {
                    set_table_tree_position(child, parent_position);
                }
            }
            Item::ArrayOfTables(tables) => {
                if original
                    .get(key.get())
                    .and_then(Item::as_array_of_tables)
                    .is_none()
                {
                    for child in tables.iter_mut() {
                        set_table_tree_position(child, parent_position);
                    }
                }
            }
            Item::None | Item::Value(_) => {}
        }
    }
}

fn values_semantically_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::String(left), Value::String(right)) => left.value() == right.value(),
        (Value::Integer(left), Value::Integer(right)) => left.value() == right.value(),
        (Value::Float(left), Value::Float(right)) => {
            left.value().to_bits() == right.value().to_bits()
        }
        (Value::Integer(integer), Value::Float(float))
        | (Value::Float(float), Value::Integer(integer)) => {
            (*integer.value() as f64).to_bits() == float.value().to_bits()
        }
        (Value::Boolean(left), Value::Boolean(right)) => left.value() == right.value(),
        (Value::Datetime(left), Value::Datetime(right)) => left.value() == right.value(),
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| values_semantically_equal(left, right))
        }
        (Value::InlineTable(left), Value::InlineTable(right)) => {
            tables_semantically_equal(left, right)
        }
        _ => false,
    }
}

fn replace_item_preserving_decor(raw: &mut Item, updated: &Item) {
    let mut replacement = updated.clone();
    match (&mut *raw, &mut replacement) {
        (Item::Value(raw), Item::Value(replacement)) => {
            *replacement.decor_mut() = raw.decor().clone();
        }
        (Item::Table(raw), Item::Table(replacement)) => {
            *replacement.decor_mut() = raw.decor().clone();
        }
        _ => {}
    }
    *raw = replacement;
}
