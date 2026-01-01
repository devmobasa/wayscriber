use std::collections::HashSet;

use super::super::types::{ShapeId, UndoAction};

impl UndoAction {
    pub(super) fn depth(&self) -> usize {
        match self {
            UndoAction::Compound(actions) => {
                1 + actions
                    .iter()
                    .map(|action| action.depth())
                    .max()
                    .unwrap_or(0)
            }
            _ => 1,
        }
    }

    pub(in crate::draw::frame) fn max_shape_id(&self) -> Option<ShapeId> {
        match self {
            UndoAction::Create { shapes } | UndoAction::Delete { shapes } => {
                shapes.iter().map(|(_, shape)| shape.id).max()
            }
            UndoAction::Modify { shape_id, .. } => Some(*shape_id),
            UndoAction::Reorder { shape_id, .. } => Some(*shape_id),
            UndoAction::Compound(actions) => actions
                .iter()
                .filter_map(|action| action.max_shape_id())
                .max(),
        }
    }

    pub(super) fn prune_removed_shapes(&mut self, removed: &HashSet<ShapeId>) -> bool {
        match self {
            UndoAction::Create { shapes } | UndoAction::Delete { shapes } => {
                shapes.retain(|(_, shape)| !removed.contains(&shape.id));
                !shapes.is_empty()
            }
            UndoAction::Modify { shape_id, .. } | UndoAction::Reorder { shape_id, .. } => {
                !removed.contains(shape_id)
            }
            UndoAction::Compound(actions) => {
                actions.retain_mut(|action| action.prune_removed_shapes(removed));
                !actions.is_empty()
            }
        }
    }

    pub(super) fn validate_against_shapes(&mut self, ids: &HashSet<ShapeId>) -> bool {
        match self {
            UndoAction::Create { .. } | UndoAction::Delete { .. } => true,
            UndoAction::Modify { shape_id, .. } | UndoAction::Reorder { shape_id, .. } => {
                ids.contains(shape_id)
            }
            UndoAction::Compound(actions) => {
                actions.retain_mut(|action| action.validate_against_shapes(ids));
                !actions.is_empty()
            }
        }
    }

    pub(super) fn collect_ids(&self, ids: &mut HashSet<ShapeId>) {
        match self {
            UndoAction::Create { shapes } | UndoAction::Delete { shapes } => {
                for (_, shape) in shapes {
                    ids.insert(shape.id);
                }
            }
            UndoAction::Modify { shape_id, .. } | UndoAction::Reorder { shape_id, .. } => {
                ids.insert(*shape_id);
            }
            UndoAction::Compound(actions) => {
                for action in actions {
                    action.collect_ids(ids);
                }
            }
        }
    }
}
