mod apply;
mod primary;
mod prune;

use std::collections::HashSet;

use crate::draw::shape::Shape;

use super::super::core::Frame;
use super::super::types::{HistoryTrimStats, ShapeId, UndoAction};

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
}
