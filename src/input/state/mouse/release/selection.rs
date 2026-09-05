use crate::draw::frame::{ShapeSnapshot, UndoAction};
use crate::draw::{Shape, ShapeId};
use crate::input::InputState;

use super::super::SELECTION_DRAG_THRESHOLD;

pub(super) fn finish_moving_selection(
    state: &mut InputState,
    snapshots: Vec<(ShapeId, ShapeSnapshot)>,
    moved: bool,
) {
    if moved {
        state.push_translation_undo(snapshots);
    }
}

pub(super) fn finish_selection_drag(
    state: &mut InputState,
    measurer: &crate::draw::TextMeasurer,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    additive: bool,
) {
    state.clear_provisional_dirty();
    let dx = (end_x - start_x).abs();
    let dy = (end_y - start_y).abs();
    if dx < SELECTION_DRAG_THRESHOLD && dy < SELECTION_DRAG_THRESHOLD {
        if !additive {
            let bounds = state.selection_bounding_box_with(measurer, state.selected_shape_ids());
            state.clear_selection();
            state.mark_selection_dirty_region(bounds);
            state.needs_redraw = true;
        }
        return;
    }

    if let Some(rect) = InputState::selection_rect_from_points(start_x, start_y, end_x, end_y) {
        let ids = state.shape_ids_in_rect_with(measurer, rect);
        if additive {
            state.extend_selection(ids);
        } else {
            state.set_selection(ids);
        }
        state.needs_redraw = true;
    }
}

/// Commits one magnification drag as a single undo entry.
///
/// The live updates during the drag deliberately recorded nothing, so the
/// whole gesture undoes in one step rather than per motion event. The wheel
/// gesture ends the same way, through the same recorder, so the two cannot
/// disagree about when a change is worth keeping.
pub(super) fn finish_spotlight_magnification(
    state: &mut InputState,
    shape_id: ShapeId,
    snapshot: ShapeSnapshot,
) {
    let Some(shape) = state.boards.active_frame().shape(shape_id) else {
        return;
    };
    let after = ShapeSnapshot {
        shape: shape.shape.clone(),
        locked: shape.locked,
    };
    state.record_spotlight_magnification_change(shape_id, snapshot, after);
}

/// Commits one bend drag as a single undo entry.
///
/// Shares the commit with `finish_active_arrow_bend`, which anything that
/// mutates the arrow mid-gesture has to run first — a release and an
/// interrupted gesture must record the drag the same way.
pub(super) fn finish_arrow_bend(
    state: &mut InputState,
    shape_id: ShapeId,
    snapshot: ShapeSnapshot,
) {
    state.commit_arrow_bend(shape_id, snapshot);
}

pub(super) fn finish_text_resize(
    state: &mut InputState,
    shape_id: ShapeId,
    snapshot: ShapeSnapshot,
) {
    let frame = state.boards.active_frame_mut();
    if let Some(shape) = frame.shape(shape_id) {
        let after_snapshot = ShapeSnapshot {
            shape: shape.shape.clone(),
            locked: shape.locked,
        };
        let before_wrap = match &snapshot.shape {
            Shape::Text { wrap_width, .. } | Shape::StickyNote { wrap_width, .. } => *wrap_width,
            _ => None,
        };
        let after_wrap = match &after_snapshot.shape {
            Shape::Text { wrap_width, .. } | Shape::StickyNote { wrap_width, .. } => *wrap_width,
            _ => None,
        };
        if before_wrap != after_wrap {
            frame.push_undo_action(
                UndoAction::Modify {
                    shape_id,
                    before: snapshot,
                    after: after_snapshot,
                },
                state.history_limits.undo_stack_limit(),
            );
            state.mark_session_dirty();
        }
    }
}

pub(super) fn finish_selection_resize(
    state: &mut InputState,
    measurer: &crate::draw::TextMeasurer,
    snapshots: &[(ShapeId, ShapeSnapshot)],
) {
    // Capture after-snapshots and push undo actions
    let mut has_changes = false;
    let frame = state.boards.active_frame_mut();
    for (shape_id, before_snapshot) in snapshots {
        if let Some(shape) = frame.shape(*shape_id) {
            let after_snapshot = ShapeSnapshot {
                shape: shape.shape.clone(),
                locked: shape.locked,
            };
            // Check if shape bounds changed (simpler than full PartialEq on Shape)
            let before_bounds = before_snapshot.shape.bounding_box_with(measurer);
            let after_bounds = after_snapshot.shape.bounding_box_with(measurer);
            if before_bounds != after_bounds {
                frame.push_undo_action(
                    UndoAction::modify_from_snapshots(
                        *shape_id,
                        before_snapshot.clone(),
                        after_snapshot,
                    ),
                    state.history_limits.undo_stack_limit(),
                );
                has_changes = true;
            }
        }
    }
    if has_changes {
        state.mark_session_dirty();
    }
    state.needs_redraw = true;
}
