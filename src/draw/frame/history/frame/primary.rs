use std::collections::HashSet;

use crate::draw::shape::Shape;

use super::super::super::core::Frame;
use super::super::super::types::{ShapeId, UndoAction};

impl Frame {
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

    pub(super) fn history_shape_ids(&self) -> HashSet<ShapeId> {
        let mut ids = HashSet::new();
        for action in self.undo_stack.iter().chain(self.redo_stack.iter()) {
            action.collect_ids(&mut ids);
        }
        ids
    }
}
