//! Frame container for managing collections of shapes with undo/redo support.

use super::shape::Shape;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

/// Unique identifier for a drawn shape within a frame.
pub type ShapeId = u64;

/// Maximum allowed compound nesting depth in persisted history.
pub const MAX_COMPOUND_DEPTH: usize = 16;

/// A shape stored in a frame with additional metadata.
#[derive(Clone, Debug)]
pub struct DrawnShape {
    pub id: ShapeId,
    pub shape: Shape,
    pub created_at: u64,
    pub locked: bool,
}

impl DrawnShape {
    fn new(id: ShapeId, shape: Shape) -> Self {
        Self {
            id,
            shape,
            created_at: current_timestamp_ms(),
            locked: false,
        }
    }

    fn with_metadata(id: ShapeId, shape: Shape, created_at: u64, locked: bool) -> Self {
        Self {
            id,
            shape,
            created_at,
            locked,
        }
    }
}

impl Serialize for DrawnShape {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        PersistedDrawnShape::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DrawnShape {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let helper = PersistedDrawnShape::deserialize(deserializer)?;
        Ok(helper.into())
    }
}

#[derive(Serialize, Deserialize)]
struct PersistedDrawnShape {
    id: ShapeId,
    shape: Shape,
    created_at: u64,
    locked: bool,
}

impl From<&DrawnShape> for PersistedDrawnShape {
    fn from(value: &DrawnShape) -> Self {
        Self {
            id: value.id,
            shape: value.shape.clone(),
            created_at: value.created_at,
            locked: value.locked,
        }
    }
}

impl From<PersistedDrawnShape> for DrawnShape {
    fn from(value: PersistedDrawnShape) -> Self {
        Self::with_metadata(value.id, value.shape, value.created_at, value.locked)
    }
}

/// Snapshot of a shape used for undo/redo of modifications.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShapeSnapshot {
    pub shape: Shape,
    pub locked: bool,
}

/// Undoable actions stored in the frame history.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UndoAction {
    Create {
        shapes: Vec<(usize, DrawnShape)>,
    },
    Delete {
        shapes: Vec<(usize, DrawnShape)>,
    },
    Modify {
        shape_id: ShapeId,
        before: ShapeSnapshot,
        after: ShapeSnapshot,
    },
    Reorder {
        shape_id: ShapeId,
        from: usize,
        to: usize,
    },
    Compound(Vec<UndoAction>),
}

/// Result of trimming or validating undo/redo history.
#[derive(Debug, Clone, Copy, Default)]
pub struct HistoryTrimStats {
    pub undo_removed: usize,
    pub redo_removed: usize,
}

impl HistoryTrimStats {
    pub fn is_empty(&self) -> bool {
        self.undo_removed == 0 && self.redo_removed == 0
    }

    fn add_undo(&mut self, count: usize) {
        self.undo_removed = self.undo_removed.saturating_add(count);
    }

    fn add_redo(&mut self, count: usize) {
        self.redo_removed = self.redo_removed.saturating_add(count);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Frame {
    #[serde(with = "frame_storage")]
    pub shapes: Vec<DrawnShape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    undo_stack: Vec<UndoAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    redo_stack: Vec<UndoAction>,
    #[serde(skip)]
    next_shape_id: ShapeId,
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

impl Frame {
    /// Creates a new empty frame.
    pub fn new() -> Self {
        Self {
            shapes: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            next_shape_id: 1,
        }
    }

    /// Clears all shapes and history from the frame.
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.shapes.clear();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.next_shape_id = 1;
    }

    #[allow(dead_code)]
    /// Returns the number of shapes in the frame.
    pub fn len(&self) -> usize {
        self.shapes.len()
    }

    #[allow(dead_code)]
    /// Returns true if the frame contains no shapes.
    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty()
    }

    /// Returns true if shapes or history stacks contain data worth persisting.
    pub fn has_persistable_data(&self) -> bool {
        !self.shapes.is_empty() || !self.undo_stack.is_empty() || !self.redo_stack.is_empty()
    }

    /// Adds a shape at the end of the stack and returns its identifier.
    pub fn add_shape(&mut self, shape: Shape) -> ShapeId {
        let index = self.shapes.len();
        let id = self.insert_new_shape(index, shape);
        self.redo_stack.clear();
        id
    }

    /// Attempts to add a shape respecting a maximum count.
    ///
    /// Returns true if the shape was added.
    pub fn try_add_shape(&mut self, shape: Shape, max: usize) -> bool {
        self.try_add_shape_with_id(shape, max).is_some()
    }

    /// Attempts to add a shape and returns the identifier on success.
    pub fn try_add_shape_with_id(&mut self, shape: Shape, max: usize) -> Option<ShapeId> {
        if max > 0 && self.shapes.len() >= max {
            return None;
        }
        Some(self.add_shape(shape))
    }

    #[allow(dead_code)]
    /// Inserts a shape at the given index and returns its identifier.
    pub fn insert_shape_at(&mut self, index: usize, shape: Shape) -> ShapeId {
        let index = index.min(self.shapes.len());
        let id = self.insert_new_shape(index, shape);
        self.redo_stack.clear();
        id
    }

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

    /// Finds the index of a shape by id.
    pub fn find_index(&self, id: ShapeId) -> Option<usize> {
        self.shapes.iter().position(|shape| shape.id == id)
    }

    /// Returns a reference to a shape by id.
    pub fn shape(&self, id: ShapeId) -> Option<&DrawnShape> {
        self.find_index(id).map(|i| &self.shapes[i])
    }

    /// Returns a mutable reference to a shape by id.
    pub fn shape_mut(&mut self, id: ShapeId) -> Option<&mut DrawnShape> {
        self.find_index(id).map(|i| &mut self.shapes[i])
    }

    /// Removes a shape by id, returning its index and data.
    pub fn remove_shape_by_id(&mut self, id: ShapeId) -> Option<(usize, DrawnShape)> {
        let index = self.find_index(id)?;
        Some((index, self.shapes.remove(index)))
    }

    /// Moves a shape from one index to another.
    pub fn move_shape(&mut self, from: usize, to: usize) -> Option<()> {
        if from >= self.shapes.len() || to >= self.shapes.len() {
            return None;
        }
        if from == to {
            return Some(());
        }
        let shape = self.shapes.remove(from);
        let mut insert_index = to.min(self.shapes.len());
        if from < to && insert_index > 0 {
            insert_index -= 1;
        }
        self.shapes.insert(insert_index, shape);
        Some(())
    }

    fn insert_new_shape(&mut self, index: usize, shape: Shape) -> ShapeId {
        let id = self.generate_id();
        let drawn = DrawnShape::new(id, shape);
        self.insert_existing(index, drawn);
        id
    }

    fn insert_existing(&mut self, index: usize, drawn: DrawnShape) {
        self.mark_id_used(drawn.id);
        self.shapes.insert(index, drawn);
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

    fn generate_id(&mut self) -> ShapeId {
        let id = self.next_shape_id;
        self.next_shape_id = self.next_shape_id.saturating_add(1);
        id
    }

    fn mark_id_used(&mut self, id: ShapeId) {
        if id >= self.next_shape_id {
            self.next_shape_id = id.saturating_add(1);
        }
    }

    fn rebuild_next_id(&mut self) {
        let shapes_max = self.shapes.iter().map(|shape| shape.id).max().unwrap_or(0);
        let history_max = self
            .undo_stack
            .iter()
            .chain(self.redo_stack.iter())
            .filter_map(|action| action.max_shape_id())
            .max()
            .unwrap_or(0);

        self.next_shape_id = shapes_max.max(history_max).saturating_add(1);
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

    fn prune_stack_for_missing_shapes(stack: &mut Vec<UndoAction>, ids: &HashSet<ShapeId>) -> usize {
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

impl<'de> Deserialize<'de> for Frame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct FrameHelper {
            #[serde(with = "frame_storage")]
            shapes: Vec<DrawnShape>,
            #[serde(default)]
            undo_stack: Vec<UndoAction>,
            #[serde(default)]
            redo_stack: Vec<UndoAction>,
        }

        let helper = FrameHelper::deserialize(deserializer)?;
        let mut frame = Frame {
            shapes: helper.shapes,
            undo_stack: helper.undo_stack,
            redo_stack: helper.redo_stack,
            next_shape_id: 1,
        };
        frame.rebuild_next_id();
        Ok(frame)
    }
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_millis() as u64)
        .unwrap_or(0)
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

    fn max_shape_id(&self) -> Option<ShapeId> {
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

mod frame_storage {
    use super::{DrawnShape, ShapeId, current_timestamp_ms};
    use crate::draw::shape::Shape;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(shapes: &Vec<DrawnShape>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let helper: Vec<SerializedDrawnShape<'_>> = shapes
            .iter()
            .map(|shape| SerializedDrawnShape {
                id: shape.id,
                shape: &shape.shape,
                created_at: shape.created_at,
                locked: shape.locked,
            })
            .collect();
        helper.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<DrawnShape>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helpers: Vec<ShapeElement> = Vec::deserialize(deserializer)?;
        let mut shapes = Vec::with_capacity(helpers.len());
        let mut next_id: ShapeId = 1;

        for entry in helpers {
            match entry {
                ShapeElement::WithMeta(helper) => {
                    let mut id = helper.id.unwrap_or(0);
                    if id == 0 {
                        id = next_id;
                    }
                    let created_at = helper.created_at.unwrap_or_else(current_timestamp_ms);
                    let locked = helper.locked.unwrap_or(false);
                    shapes.push(DrawnShape::with_metadata(
                        id,
                        helper.shape,
                        created_at,
                        locked,
                    ));
                    next_id = next_id.max(id.saturating_add(1));
                }
                ShapeElement::Legacy(shape) => {
                    let id = next_id;
                    shapes.push(DrawnShape::with_metadata(
                        id,
                        shape,
                        current_timestamp_ms(),
                        false,
                    ));
                    next_id = next_id.saturating_add(1);
                }
            }
        }

        Ok(shapes)
    }

    #[derive(Serialize)]
    struct SerializedDrawnShape<'a> {
        id: ShapeId,
        shape: &'a Shape,
        created_at: u64,
        locked: bool,
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ShapeElement {
        WithMeta(WithMeta),
        Legacy(Shape),
    }

    #[derive(Deserialize)]
    struct WithMeta {
        #[serde(default)]
        id: Option<ShapeId>,
        shape: Shape,
        #[serde(default)]
        created_at: Option<u64>,
        #[serde(default)]
        locked: Option<bool>,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{Shape, color::BLACK};

    #[test]
    fn frame_serializes_history() {
        let mut frame = Frame::new();
        let first = frame.add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 10,
            y2: 10,
            color: BLACK,
            thick: 2.0,
        });
        let first_index = frame.find_index(first).unwrap();
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(first_index, frame.shape(first).unwrap().clone())],
            },
            100,
        );

        let second = frame.add_shape(Shape::Line {
            x1: 1,
            y1: 1,
            x2: 5,
            y2: 5,
            color: BLACK,
            thick: 2.0,
        });
        let second_index = frame.find_index(second).unwrap();
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(second_index, frame.shape(second).unwrap().clone())],
            },
            100,
        );
        // Move the second action to the redo stack.
        frame.undo_last();

        assert_eq!(frame.undo_stack_len(), 1);
        assert_eq!(frame.redo_stack_len(), 1);

        let json = serde_json::to_string(&frame).expect("serialize frame");
        let mut restored: Frame = serde_json::from_str(&json).expect("deserialize frame");
        assert_eq!(restored.undo_stack_len(), 1);
        assert_eq!(restored.redo_stack_len(), 1);

        let new_id = restored.add_shape(Shape::Line {
            x1: 2,
            y1: 2,
            x2: 6,
            y2: 6,
            color: BLACK,
            thick: 1.0,
        });
        assert!(new_id > second);
    }

    #[test]
    fn frame_with_history_is_persistable_even_without_shapes() {
        let mut frame = Frame::new();
        let id = frame.add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 20,
            y2: 20,
            color: BLACK,
            thick: 2.0,
        });
        let index = frame.find_index(id).unwrap();
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(index, frame.shape(id).unwrap().clone())],
            },
            100,
        );

        // Undo to move the action into redo stack and clear the canvas.
        frame.undo_last();

        assert!(frame.shapes.is_empty());
        assert!(frame.has_persistable_data());
    }

    #[test]
    fn try_add_shape_respects_limit() {
        let mut frame = Frame::new();
        assert!(frame.try_add_shape(
            Shape::Line {
                x1: 0,
                y1: 0,
                x2: 1,
                y2: 1,
                color: BLACK,
                thick: 2.0,
            },
            1
        ));
        assert!(!frame.try_add_shape(
            Shape::Line {
                x1: 1,
                y1: 1,
                x2: 2,
                y2: 2,
                color: BLACK,
                thick: 2.0,
            },
            1
        ));
    }

    #[test]
    fn undo_and_redo_cycle_shapes() {
        let mut frame = Frame::new();
        let shape = Shape::Line {
            x1: 0,
            y1: 0,
            x2: 10,
            y2: 10,
            color: BLACK,
            thick: 2.0,
        };

        let id = frame.add_shape(shape.clone());
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(
                    frame.shapes.len().saturating_sub(1),
                    frame.shape(id).unwrap().clone(),
                )],
            },
            10,
        );
        assert_eq!(frame.shapes.len(), 1);

        let undone = frame.undo_last();
        assert!(undone.is_some());
        assert_eq!(frame.shapes.len(), 0);

        let redone = frame.redo_last();
        assert!(redone.is_some());
        assert_eq!(frame.shapes.len(), 1);
    }

    #[test]
    fn adding_new_shape_clears_redo_stack() {
        let mut frame = Frame::new();
        let first = Shape::Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
            color: BLACK,
            thick: 2.0,
        };
        let id = frame.add_shape(first);
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(
                    frame.shapes.len().saturating_sub(1),
                    frame.shape(id).unwrap().clone(),
                )],
            },
            10,
        );
        frame.undo_last();
        assert_eq!(frame.shapes.len(), 0);

        let second = Shape::Rect {
            x: 10,
            y: 10,
            w: 15,
            h: 15,
            color: BLACK,
            thick: 2.0,
        };
        frame.add_shape(second);
        assert_eq!(frame.redo_stack_len(), 0);
    }

    #[test]
    fn undo_stack_respects_limit() {
        let mut frame = Frame::new();
        for i in 0..5 {
            let shape = Shape::Line {
                x1: i,
                y1: 0,
                x2: i + 10,
                y2: 10,
                color: BLACK,
                thick: 2.0,
            };
            let id = frame.add_shape(shape);
            let index = frame.find_index(id).unwrap();
            let snapshot = frame.shape(id).unwrap().clone();
            frame.push_undo_action(
                UndoAction::Create {
                    shapes: vec![(index, snapshot)],
                },
                3,
            );
        }

        assert_eq!(frame.undo_stack_len(), 3);
    }
}
