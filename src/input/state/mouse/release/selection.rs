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
            let bounds = state.selection_bounding_box(state.selected_shape_ids());
            state.clear_selection();
            state.mark_selection_dirty_region(bounds);
            state.needs_redraw = true;
        }
        return;
    }

    if let Some(rect) = InputState::selection_rect_from_points(start_x, start_y, end_x, end_y) {
        let ids = state.shape_ids_in_rect(rect);
        if additive {
            state.extend_selection(ids);
        } else {
            state.set_selection(ids);
        }
        state.needs_redraw = true;
    }
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
                state.undo_stack_limit,
            );
            state.mark_session_dirty();
        }
    }
}
