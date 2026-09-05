//! Canvas mutation transactions. InputState applies UI and persistence effects.
use crate::draw::frame::{Frame, ShapeSnapshot, UndoAction};
use crate::draw::{Shape, ShapeId, TextMeasurer};
use crate::util::Rect;
use std::borrow::Cow;

pub(in crate::input::state) struct CanvasEdit<'a> {
    snapshots: Cow<'a, [(ShapeId, ShapeSnapshot)]>,
}

#[derive(Default)]
#[must_use = "apply edit effects to keep canvas caches, redraw, and persistence coherent"]
pub(in crate::input::state) struct EditEffects {
    regions: Vec<(ShapeId, Option<Rect>, Option<Rect>)>,
    committed: bool,
}

impl<'a> CanvasEdit<'a> {
    pub(in crate::input::state) fn capture(frame: &Frame, ids: &[ShapeId]) -> Self {
        Self {
            snapshots: Cow::Owned(
                ids.iter()
                    .filter_map(|id| {
                        let drawn = frame.shape(*id)?;
                        (!drawn.locked).then(|| {
                            (
                                *id,
                                ShapeSnapshot {
                                    shape: drawn.shape.clone(),
                                    locked: drawn.locked,
                                },
                            )
                        })
                    })
                    .collect(),
            ),
        }
    }

    pub(in crate::input::state) fn from_snapshots(
        snapshots: Vec<(ShapeId, ShapeSnapshot)>,
    ) -> Self {
        Self {
            snapshots: Cow::Owned(snapshots),
        }
    }

    /// Live resize borrows the gesture's original snapshots; motion events
    /// must not clone whole freehand paths just to read their starting geometry.
    pub(in crate::input::state) fn borrow_snapshots(
        snapshots: &'a [(ShapeId, ShapeSnapshot)],
    ) -> Self {
        Self {
            snapshots: Cow::Borrowed(snapshots),
        }
    }

    pub(in crate::input::state) fn preview_current(
        frame: &mut Frame,
        ids: &[ShapeId],
        measurer: &TextMeasurer,
        mut apply: impl FnMut(&mut Shape) -> bool,
    ) -> EditEffects {
        let mut effects = EditEffects::default();
        for id in ids {
            effects.preview_shape(frame, *id, measurer, &mut apply);
        }
        effects
    }

    pub(in crate::input::state) fn into_snapshots(self) -> Vec<(ShapeId, ShapeSnapshot)> {
        self.snapshots.into_owned()
    }

    pub(in crate::input::state) fn preview(
        &self,
        frame: &mut Frame,
        measurer: &TextMeasurer,
        mut apply: impl FnMut(&mut Shape, &ShapeSnapshot) -> bool,
    ) -> EditEffects {
        let mut effects = EditEffects::default();
        for (id, snapshot) in self.snapshots.iter() {
            effects.preview_shape(frame, *id, measurer, |shape| apply(shape, snapshot));
        }
        effects
    }

    pub(in crate::input::state) fn commit(self, frame: &mut Frame, limit: usize) -> EditEffects {
        let actions = self
            .snapshots
            .into_owned()
            .into_iter()
            .filter_map(|(id, before)| {
                let drawn = frame.shape(id)?;
                let after = ShapeSnapshot {
                    shape: drawn.shape.clone(),
                    locked: drawn.locked,
                };
                (before != after).then(|| UndoAction::modify_from_snapshots(id, before, after))
            })
            .collect();
        EditEffects {
            committed: record(frame, limit, actions),
            ..EditEffects::default()
        }
    }

    pub(in crate::input::state) fn rollback(
        self,
        frame: &mut Frame,
        measurer: &TextMeasurer,
    ) -> EditEffects {
        let mut effects = EditEffects::default();
        for (id, snapshot) in self.snapshots.into_owned() {
            let Some(drawn) = frame.shape_mut(id) else {
                continue;
            };
            let before = drawn.bounding_box_with(measurer);
            drawn.set_shape(snapshot.shape);
            drawn.locked = snapshot.locked;
            effects
                .regions
                .push((id, before, drawn.bounding_box_with(measurer)));
        }
        effects
    }

    pub(in crate::input::state) fn apply_selection(
        frame: &mut Frame,
        ids: &[ShapeId],
        measurer: &TextMeasurer,
        limit: usize,
        mut applicable: impl FnMut(&Shape) -> bool,
        mut apply: impl FnMut(&mut Shape) -> bool,
    ) -> (usize, usize, usize, EditEffects) {
        let mut applicable_count = 0;
        let mut locked = 0;
        let mut editable = Vec::new();
        for id in ids {
            let Some(drawn) = frame.shape(*id) else {
                continue;
            };
            if !applicable(&drawn.shape) {
                continue;
            }
            applicable_count += 1;
            if drawn.locked {
                locked += 1;
            } else {
                editable.push(*id);
            }
        }
        let edit = Self::capture(frame, &editable);
        let mut effects = edit.preview(frame, measurer, |shape, _| apply(shape));
        let changed = effects.regions.len();
        effects.committed = edit.commit(frame, limit).committed;
        (changed, locked, applicable_count, effects)
    }

    pub(in crate::input::state) fn delete(
        frame: &mut Frame,
        ids: &std::collections::HashSet<ShapeId>,
        measurer: &TextMeasurer,
        limit: usize,
    ) -> EditEffects {
        let removed: Vec<_> = frame
            .shapes
            .iter()
            .enumerate()
            .filter(|(_, shape)| ids.contains(&shape.id) && !shape.locked)
            .map(|(index, shape)| (index, shape.clone()))
            .collect();
        let mut effects = EditEffects::default();
        for (index, shape) in removed.iter().rev() {
            frame.remove_shape_at(*index);
            effects
                .regions
                .push((shape.id, shape.bounding_box_with(measurer), None));
        }
        if !removed.is_empty() {
            effects.committed = record(frame, limit, vec![UndoAction::Delete { shapes: removed }]);
        }
        effects
    }
}

impl EditEffects {
    fn preview_shape(
        &mut self,
        frame: &mut Frame,
        id: ShapeId,
        measurer: &TextMeasurer,
        apply: impl FnOnce(&mut Shape) -> bool,
    ) {
        let Some(drawn) = frame.shape_mut(id) else {
            return;
        };
        if drawn.locked {
            return;
        }
        let before = drawn.bounding_box_with(measurer);
        if apply(&mut drawn.shape) {
            drawn.invalidate_bounds();
            self.regions
                .push((id, before, drawn.bounding_box_with(measurer)));
        }
    }
}

fn record(frame: &mut Frame, limit: usize, mut actions: Vec<UndoAction>) -> bool {
    let action = match actions.len() {
        0 => return false,
        1 => actions.remove(0),
        _ => UndoAction::Compound { actions },
    };
    frame.push_undo_action(action, limit);
    true
}

impl super::base::InputState {
    pub(in crate::input::state) fn apply_edit_effects(
        &mut self,
        measurer: &TextMeasurer,
        effects: EditEffects,
    ) -> bool {
        let changed = effects.committed || !effects.regions.is_empty();
        for (id, before, after) in effects.regions {
            self.mark_selection_dirty_region(before);
            self.mark_selection_dirty_region(after);
            self.invalidate_hit_cache_for_with(measurer, id);
        }
        if effects.committed {
            self.mark_session_dirty();
        }
        if changed {
            self.needs_redraw = true;
        }
        changed
    }
}

#[cfg(test)]
mod tests;
