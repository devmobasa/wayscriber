use std::collections::HashSet;

use super::super::super::core::Frame;
use super::super::super::types::{ShapeId, UndoAction};

impl Frame {
    pub(super) fn clamp_stack(stack: &mut Vec<UndoAction>, limit: usize) -> usize {
        if stack.len() <= limit {
            return 0;
        }
        let overflow = stack.len() - limit;
        stack.drain(0..overflow);
        overflow
    }

    pub(super) fn prune_stack_for_removed_ids(
        stack: &mut Vec<UndoAction>,
        removed: &HashSet<ShapeId>,
    ) -> usize {
        if stack.is_empty() {
            return 0;
        }
        let before = stack.len();
        stack.retain_mut(|action| action.prune_removed_shapes(removed));
        before - stack.len()
    }

    pub(super) fn prune_stack_by_depth(stack: &mut Vec<UndoAction>, limit: usize) -> usize {
        if stack.is_empty() {
            return 0;
        }
        let before = stack.len();
        stack.retain(|action| action.depth() <= limit);
        before - stack.len()
    }

    pub(super) fn prune_stack_for_missing_shapes(
        stack: &mut Vec<UndoAction>,
        ids: &HashSet<ShapeId>,
    ) -> usize {
        if stack.is_empty() {
            return 0;
        }
        let before = stack.len();
        stack.retain_mut(|action| action.validate_against_shapes(ids));
        before - stack.len()
    }
}
