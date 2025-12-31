use super::core::Frame;
use super::types::{HistoryTrimStats, ShapeId, UndoAction};
use crate::draw::shape::Shape;
use std::collections::HashSet;

impl Frame {
    /// Records an undoable action, enforcing a stack limit.
    pub fn push_undo_action(&mut self, action: UndoAction, limit: usize) {
        self.undo_stack.push(action);
        if limit > 0 && self.undo_stack.len() > limit {
            let overflow = self.undo_stack.len() - limit;
            self.undo_stack.drain(0..overflow);
        }
        self.redo_stack.clear();
    }

    /// Undoes the most recent action, returning it for external bookkeeping.
    pub fn undo_last(&mut self) -> Option<UndoAction> {
        let action = self.undo_stack.pop()?;
        self.apply_inverse(&action);
        self.redo_stack.push(action.clone());
        Some(action)
    }

    /// Redoes the most recently undone action.
    pub fn redo_last(&mut self) -> Option<UndoAction> {
        let action = self.redo_stack.pop()?;
        self.apply_action(&action);
        self.undo_stack.push(action.clone());
        Some(action)
    }

    #[allow(dead_code)]
    /// Legacy helper used by existing code paths to undo an action and retrieve a representative shape.
    pub fn undo(&mut self) -> Option<Shape> {
        let action = self.undo_last()?;
        Self::primary_shape_for_undo(&action)
    }

    #[allow(dead_code)]
    /// Legacy helper used by existing code paths to redo an action and retrieve a representative shape.
    pub fn redo(&mut self) -> Option<Shape> {
        let action = self.redo_last()?;
        Self::primary_shape_for_redo(&action)
    }

    #[allow(dead_code)]
    /// Returns a reference to the undo stack (for testing).
    pub fn undo_stack_len(&self) -> usize {
        self.undo_stack.len()
    }

    #[allow(dead_code)]
    /// Returns a reference to the redo stack length (for testing).
    pub fn redo_stack_len(&self) -> usize {
        self.redo_stack.len()
    }

    /// Truncates undo/redo stacks to the provided depth, returning counts of dropped actions.
    pub fn clamp_history_depth(&mut self, limit: usize) -> HistoryTrimStats {
        let mut stats = HistoryTrimStats::default();
        if limit == 0 {
            if !self.undo_stack.is_empty() {
                stats.add_undo(self.undo_stack.len());
                self.undo_stack.clear();
            }
            if !self.redo_stack.is_empty() {
                stats.add_redo(self.redo_stack.len());
                self.redo_stack.clear();
            }
            return stats;
        }

        stats.add_undo(Self::clamp_stack(&mut self.undo_stack, limit));
        stats.add_redo(Self::clamp_stack(&mut self.redo_stack, limit));
        stats
    }

    /// Removes history entries referencing the provided shape ids.
    pub fn prune_history_for_removed_ids(
        &mut self,
        removed: &HashSet<ShapeId>,
    ) -> HistoryTrimStats {
        if removed.is_empty() {
            return HistoryTrimStats::default();
        }
        let mut stats = HistoryTrimStats::default();
        stats.add_undo(Self::prune_stack_for_removed_ids(
            &mut self.undo_stack,
            removed,
        ));
        stats.add_redo(Self::prune_stack_for_removed_ids(
            &mut self.redo_stack,
            removed,
        ));
        stats
    }

    /// Drops actions exceeding the allowed compound depth.
    pub fn validate_history(&mut self, max_depth: usize) -> HistoryTrimStats {
        if max_depth == 0 {
            return self.clamp_history_depth(0);
        }
        let mut stats = HistoryTrimStats::default();
        stats.add_undo(Self::prune_stack_by_depth(&mut self.undo_stack, max_depth));
        stats.add_redo(Self::prune_stack_by_depth(&mut self.redo_stack, max_depth));
        stats
    }

    /// Drops history actions that reference shape ids not present in the frame.
    pub fn prune_history_against_shapes(&mut self) -> HistoryTrimStats {
        let mut ids: HashSet<ShapeId> = self.shapes.iter().map(|s| s.id).collect();
        if ids.is_empty() {
            ids = self.history_shape_ids();
        }
        if ids.is_empty() {
            return HistoryTrimStats::default();
        }
        let mut stats = HistoryTrimStats::default();
        stats.add_undo(Self::prune_stack_for_missing_shapes(
            &mut self.undo_stack,
            &ids,
        ));
        stats.add_redo(Self::prune_stack_for_missing_shapes(
            &mut self.redo_stack,
            &ids,
        ));
        stats
    }

    fn apply_action(&mut self, action: &UndoAction) {
        match action {
            UndoAction::Create { shapes } => {
                for (offset, (index, shape)) in shapes.iter().enumerate() {
                    self.insert_existing(index + offset, shape.clone());
                }
            }
            UndoAction::Delete { shapes } => {
                for (_, shape) in shapes {
                    self.remove_shape_by_id(shape.id);
                }
            }
            UndoAction::Modify {
                shape_id, after, ..
            } => {
                if let Some(target) = self.shape_mut(*shape_id) {
                    target.shape = after.shape.clone();
                    target.locked = after.locked;
                }
            }
            UndoAction::Reorder {
                shape_id,
                from: _,
                to,
            } => {
                self.move_shape_to(*shape_id, *to);
            }
            UndoAction::Compound(actions) => {
                for action in actions {
                    self.apply_action(action);
                }
            }
        }
    }

    fn apply_inverse(&mut self, action: &UndoAction) {
        match action {
            UndoAction::Create { shapes } => {
                for (_, shape) in shapes.iter().rev() {
                    self.remove_shape_by_id(shape.id);
                }
            }
            UndoAction::Delete { shapes } => {
                for (offset, (index, shape)) in shapes.iter().enumerate() {
                    let insert_at = (index + offset).min(self.shapes.len());
                    self.insert_existing(insert_at, shape.clone());
                }
            }
            UndoAction::Modify {
                shape_id, before, ..
            } => {
                if let Some(target) = self.shape_mut(*shape_id) {
                    target.shape = before.shape.clone();
                    target.locked = before.locked;
                }
            }
            UndoAction::Reorder { shape_id, from, .. } => {
                self.move_shape_to(*shape_id, *from);
            }
            UndoAction::Compound(actions) => {
                for action in actions.iter().rev() {
                    self.apply_inverse(action);
                }
            }
        }
    }

    fn move_shape_to(&mut self, shape_id: ShapeId, target: usize) {
        if let Some(index) = self.find_index(shape_id) {
            if index == target {
                return;
            }
            let shape = self.shapes.remove(index);
            let mut insert_index = target.min(self.shapes.len());
            if index < insert_index && insert_index > 0 {
                insert_index -= 1;
            }
            self.shapes.insert(insert_index, shape);
        }
    }

    #[allow(dead_code)]
    pub fn primary_shape_for_undo(action: &UndoAction) -> Option<Shape> {
        match action {
            UndoAction::Create { shapes } => shapes.first().map(|(_, s)| s.shape.clone()),
            UndoAction::Delete { shapes } => shapes.first().map(|(_, s)| s.shape.clone()),
            UndoAction::Modify { before, .. } => Some(before.shape.clone()),
            UndoAction::Reorder { .. } => None,
            UndoAction::Compound(actions) => {
                actions.iter().rev().find_map(Self::primary_shape_for_undo)
            }
        }
    }

    #[allow(dead_code)]
    pub fn primary_shape_for_redo(action: &UndoAction) -> Option<Shape> {
        match action {
            UndoAction::Create { shapes } => shapes.first().map(|(_, s)| s.shape.clone()),
            UndoAction::Delete { shapes } => shapes.first().map(|(_, s)| s.shape.clone()),
            UndoAction::Modify { after, .. } => Some(after.shape.clone()),
            UndoAction::Reorder { .. } => None,
            UndoAction::Compound(actions) => actions.iter().find_map(Self::primary_shape_for_redo),
        }
    }

    fn clamp_stack(stack: &mut Vec<UndoAction>, limit: usize) -> usize {
        if stack.len() <= limit {
            return 0;
        }
        let overflow = stack.len() - limit;
        stack.drain(0..overflow);
        overflow
    }

    fn prune_stack_for_removed_ids(
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

    fn prune_stack_by_depth(stack: &mut Vec<UndoAction>, limit: usize) -> usize {
        if stack.is_empty() {
            return 0;
        }
        let before = stack.len();
        stack.retain(|action| action.depth() <= limit);
        before - stack.len()
    }

    fn prune_stack_for_missing_shapes(
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

    fn history_shape_ids(&self) -> HashSet<ShapeId> {
        let mut ids = HashSet::new();
        for action in self.undo_stack.iter().chain(self.redo_stack.iter()) {
            action.collect_ids(&mut ids);
        }
        ids
    }
}

impl UndoAction {
    fn depth(&self) -> usize {
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

    pub(super) fn max_shape_id(&self) -> Option<ShapeId> {
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

    fn prune_removed_shapes(&mut self, removed: &HashSet<ShapeId>) -> bool {
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

    fn validate_against_shapes(&mut self, ids: &HashSet<ShapeId>) -> bool {
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

    fn collect_ids(&self, ids: &mut HashSet<ShapeId>) {
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
