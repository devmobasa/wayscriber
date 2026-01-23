use super::types::{BoardPagesSnapshot, SessionSnapshot};
use crate::draw::frame::{MAX_COMPOUND_DEPTH, ShapeId};
use log::{debug, warn};
use serde_json::Value;
use std::collections::HashSet;

pub(super) fn enforce_shape_limits(snapshot: &mut SessionSnapshot, max_shapes: usize) {
    if max_shapes == 0 {
        return;
    }

    for board in &mut snapshot.boards {
        for (idx, frame_data) in board.pages.pages.iter_mut().enumerate() {
            if frame_data.shapes.len() <= max_shapes {
                continue;
            }
            let removed: Vec<_> = frame_data.shapes.drain(max_shapes..).collect();
            warn!(
                "Session board '{}' page #{} contains {} shapes which exceeds the limit of {}; truncating",
                board.id,
                idx + 1,
                frame_data.shapes.len() + removed.len(),
                max_shapes
            );
            let removed_ids: HashSet<ShapeId> = removed.into_iter().map(|shape| shape.id).collect();
            if !removed_ids.is_empty() {
                let stats = frame_data.prune_history_for_removed_ids(&removed_ids);
                if !stats.is_empty() {
                    warn!(
                        "Dropped {} undo and {} redo actions referencing trimmed shapes in '{}' page #{} history",
                        stats.undo_removed,
                        stats.redo_removed,
                        board.id,
                        idx + 1
                    );
                }
            }
        }
    }
}

pub(super) fn apply_history_policies(
    pages: &mut BoardPagesSnapshot,
    board_id: &str,
    depth_limit: Option<usize>,
) {
    for (idx, frame_data) in pages.pages.iter_mut().enumerate() {
        let depth_trim = frame_data.validate_history(MAX_COMPOUND_DEPTH);
        if !depth_trim.is_empty() {
            warn!(
                "Removed {} undo and {} redo actions with invalid structure in '{}' page #{} history",
                depth_trim.undo_removed,
                depth_trim.redo_removed,
                board_id,
                idx + 1
            );
        }
        let shape_trim = frame_data.prune_history_against_shapes();
        if !shape_trim.is_empty() {
            warn!(
                "Removed {} undo and {} redo actions referencing missing shapes in '{}' page #{} history",
                shape_trim.undo_removed,
                shape_trim.redo_removed,
                board_id,
                idx + 1
            );
        }
        if let Some(limit) = depth_limit {
            let trimmed = frame_data.clamp_history_depth(limit);
            if !trimmed.is_empty() {
                debug!(
                    "Clamped '{}' page #{} history to {} entries (dropped {} undo / {} redo)",
                    board_id,
                    idx + 1,
                    limit,
                    trimmed.undo_removed,
                    trimmed.redo_removed
                );
            }
        }
    }
}

pub(super) fn max_history_depth(doc: &Value) -> usize {
    let mut max_depth = 0;
    if let Some(Value::Array(boards)) = doc.get("boards") {
        for board in boards {
            if let Some(obj) = board.as_object()
                && let Some(Value::Array(pages)) = obj.get("pages")
            {
                for page in pages {
                    if let Some(obj) = page.as_object() {
                        for stack_key in ["undo_stack", "redo_stack"] {
                            if let Some(Value::Array(arr)) = obj.get(stack_key) {
                                max_depth = max_depth.max(depth_array(arr));
                            }
                        }
                    }
                }
            }
        }
    }
    for key in [
        "transparent",
        "whiteboard",
        "blackboard",
        "transparent_pages",
        "whiteboard_pages",
        "blackboard_pages",
    ] {
        if let Some(Value::Object(obj)) = doc.get(key) {
            for stack_key in ["undo_stack", "redo_stack"] {
                if let Some(Value::Array(arr)) = obj.get(stack_key) {
                    max_depth = max_depth.max(depth_array(arr));
                }
            }
        } else if let Some(Value::Array(pages)) = doc.get(key) {
            for page in pages {
                if let Some(obj) = page.as_object() {
                    for stack_key in ["undo_stack", "redo_stack"] {
                        if let Some(Value::Array(arr)) = obj.get(stack_key) {
                            max_depth = max_depth.max(depth_array(arr));
                        }
                    }
                }
            }
        }
    }
    max_depth
}

pub(super) fn strip_history_fields(doc: &mut Value) {
    if let Some(obj) = doc.as_object_mut() {
        if let Some(Value::Array(boards)) = obj.get_mut("boards") {
            for board in boards {
                if let Some(board_obj) = board.as_object_mut()
                    && let Some(Value::Array(pages)) = board_obj.get_mut("pages")
                {
                    for page in pages {
                        if let Some(frame) = page.as_object_mut() {
                            frame.remove("undo_stack");
                            frame.remove("redo_stack");
                        }
                    }
                }
            }
        }
        for key in [
            "transparent",
            "whiteboard",
            "blackboard",
            "transparent_pages",
            "whiteboard_pages",
            "blackboard_pages",
        ] {
            if let Some(Value::Object(frame)) = obj.get_mut(key) {
                frame.remove("undo_stack");
                frame.remove("redo_stack");
            } else if let Some(Value::Array(pages)) = obj.get_mut(key) {
                for page in pages {
                    if let Some(frame) = page.as_object_mut() {
                        frame.remove("undo_stack");
                        frame.remove("redo_stack");
                    }
                }
            }
        }
    }
}

fn depth_array(arr: &[Value]) -> usize {
    arr.iter().map(depth_action).max().unwrap_or(1)
}

fn depth_action(action: &Value) -> usize {
    if let Some(obj) = action.as_object() {
        let is_compound = obj.get("kind").and_then(|v| v.as_str()) == Some("compound");
        if is_compound {
            let mut child_max = 0;
            for value in obj.values() {
                if let Value::Array(arr) = value {
                    child_max = child_max.max(depth_array(arr));
                }
            }
            return 1 + child_max;
        }
    }
    1
}
