//! The on-canvas bend handle of a selected curved arrow.
//!
//! A curved arrow's shaft follows a quadratic Bezier whose control point is
//! pinned to the chord's perpendicular bisector, so the whole arc is described
//! by one signed scalar: `bend`. The handle rides the arc's midpoint, and
//! dragging it sets that scalar from the pointer's perpendicular distance to
//! the chord.

use crate::draw::{ArrowStyle, Shape, ShapeId};
use crate::input::InputState;
use crate::util::{self, Rect};

/// Side length of the square bend handle, in canvas pixels.
const BEND_HANDLE_SIZE: i32 = 10;

/// Increment the bend snaps to while `Shift` is held.
///
/// `Shift` means constrain everywhere else in this codebase; on a control whose
/// arc is already symmetric by construction, the thing left to constrain is the
/// magnitude. Ten steps each way is fine enough to shape an arc and coarse
/// enough that two arrows drawn a minute apart can be made to match.
const BEND_SNAP_STEP: f64 = 0.1;

/// The bend handle of one selected curved arrow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ArrowBendHandle {
    pub(crate) shape_id: ShapeId,
    /// Square to draw and to hit-test, centred on the arc's midpoint.
    pub(crate) rect: Rect,
}

impl InputState {
    /// The bend handle, when exactly one unlocked curved arrow is selected.
    ///
    /// Single selection only, like the text resize handle and the spotlight
    /// knob: the handle edits one arrow's arc, and a mixed selection has no
    /// honest position to put it at.
    pub(crate) fn selected_arrow_bend_handle(&self) -> Option<ArrowBendHandle> {
        let ids = self.selected_shape_ids();
        if ids.len() != 1 {
            return None;
        }
        let shape_id = ids[0];
        let drawn = self.boards.active_frame().shape(shape_id)?;
        if drawn.locked {
            return None;
        }
        let Shape::Arrow {
            x1,
            y1,
            x2,
            y2,
            head_at_end,
            style,
            bend,
            ..
        } = drawn.shape
        else {
            return None;
        };
        if style != ArrowStyle::Curved {
            return None;
        }

        let (tip, tail) = arrow_ends((x1, y1), (x2, y2), head_at_end);
        let (mx, my) = arc_midpoint(tip, tail, bend)?;
        let half = BEND_HANDLE_SIZE / 2;
        let rect = Rect::new(
            mx.round() as i32 - half,
            my.round() as i32 - half,
            BEND_HANDLE_SIZE,
            BEND_HANDLE_SIZE,
        )?;
        Some(ArrowBendHandle { shape_id, rect })
    }

    /// Whether the pointer is on the bend handle, and which arrow it belongs to.
    pub(crate) fn hit_arrow_bend_handle(&self, x: i32, y: i32) -> Option<ArrowBendHandle> {
        let handle = self.selected_arrow_bend_handle()?;
        let tolerance = self.hit_test_tolerance().ceil() as i32;
        let hit = handle.rect.inflated(tolerance).unwrap_or(handle.rect);
        hit.contains(x, y).then_some(handle)
    }

    /// Applies a pointer position to the arrow being bent.
    ///
    /// The chord is re-read from the shape rather than frozen at press: bending
    /// never moves the endpoints, so the mapping is stable for the whole
    /// gesture and cannot drift from what is on screen.
    pub(crate) fn drag_arrow_bend_to(&mut self, x: i32, y: i32, snap: bool) -> bool {
        let crate::input::state::DrawingState::BendingArrow { shape_id, .. } = self.state else {
            return false;
        };
        let frame = self.boards.active_frame();
        let Some(drawn) = frame.shape(shape_id) else {
            return false;
        };
        let Shape::Arrow {
            x1,
            y1,
            x2,
            y2,
            head_at_end,
            ..
        } = drawn.shape
        else {
            return false;
        };
        let (tip, tail) = arrow_ends((x1, y1), (x2, y2), head_at_end);
        let Some(bend) = bend_for_pointer(tip, tail, (x, y)) else {
            return false;
        };
        let bend = if snap {
            (bend / BEND_SNAP_STEP).round() * BEND_SNAP_STEP
        } else {
            bend
        };
        self.set_arrow_shape_bend(shape_id, bend)
    }

    /// Writes a bend onto one arrow, marking what the change repainted.
    ///
    /// Records no undo entry: the drag pushes one entry when it ends, so the
    /// whole gesture undoes in a single step instead of once per motion event.
    pub(crate) fn set_arrow_shape_bend(&mut self, shape_id: ShapeId, bend: f64) -> bool {
        let clamped = util::clamp_arrow_bend(bend);
        let frame = self.boards.active_frame_mut();
        let Some(drawn) = frame.shape_mut(shape_id) else {
            return false;
        };
        let before = drawn.bounding_box();
        let Shape::Arrow { bend: current, .. } = &mut drawn.shape else {
            return false;
        };
        if (*current - clamped).abs() <= f64::EPSILON {
            return false;
        }
        *current = clamped;
        drawn.invalidate_bounds();
        let after = drawn.bounding_box();
        self.mark_selection_dirty_region(before);
        self.mark_selection_dirty_region(after);
        self.invalidate_hit_cache_for(shape_id);
        self.mark_session_dirty();
        self.needs_redraw = true;
        true
    }

    /// Ends an in-progress bend drag, committing what it has already changed.
    ///
    /// Anything that mutates the arrow being bent has to call this first.
    /// Otherwise the mutation pushes its own undo entry while the gesture is
    /// still holding a snapshot from before the bend, and the eventual release
    /// pushes a second entry measured from that same stale snapshot — so undo
    /// walks back through a shape that never existed. Restyling is the case
    /// that bites, because leaving `Curved` hides the arc the drag is editing.
    ///
    /// Returns `true` when a gesture was actually ended.
    pub(crate) fn finish_active_arrow_bend(&mut self) -> bool {
        // Checked before the take, not after: `mem::replace` would install
        // `Idle` on the way to discovering there was no bend to end, silently
        // cancelling whatever interaction was actually running.
        if !matches!(
            self.state,
            crate::input::state::DrawingState::BendingArrow { .. }
        ) {
            return false;
        }
        let crate::input::state::DrawingState::BendingArrow { shape_id, snapshot } =
            std::mem::replace(&mut self.state, crate::input::state::DrawingState::Idle)
        else {
            return false;
        };
        self.commit_arrow_bend(shape_id, snapshot);
        self.end_pointer_drag();
        true
    }

    /// Records one finished bend drag as a single undo entry.
    ///
    /// The live updates during the drag deliberately recorded nothing, so the
    /// whole gesture undoes in one step rather than once per motion event. A
    /// drag that ended where it started records nothing at all.
    pub(crate) fn commit_arrow_bend(
        &mut self,
        shape_id: ShapeId,
        snapshot: crate::draw::frame::ShapeSnapshot,
    ) {
        let Some(shape) = self.boards.active_frame().shape(shape_id) else {
            return;
        };
        let after = crate::draw::frame::ShapeSnapshot {
            shape: shape.shape.clone(),
            locked: shape.locked,
        };
        if snapshot_bend(&snapshot) == snapshot_bend(&after) {
            return;
        }
        let limit = self.history_limits.undo_stack_limit();
        self.boards.active_frame_mut().push_undo_action(
            crate::draw::frame::UndoAction::Modify {
                shape_id,
                before: snapshot,
                after,
            },
            limit,
        );
        self.mark_session_dirty();
    }
}

/// Bit pattern of an arrow snapshot's bend, so the comparison above is exact
/// rather than an epsilon nobody chose.
fn snapshot_bend(snapshot: &crate::draw::frame::ShapeSnapshot) -> Option<u64> {
    match &snapshot.shape {
        Shape::Arrow { bend, .. } => Some(bend.to_bits()),
        _ => None,
    }
}

/// Splits an arrow's stored endpoints into tip and tail.
///
/// `head_at_end` is what decides which stored point the head sits on, and the
/// bend's sign is measured against the tail-to-tip direction — so reading it
/// here is what keeps flipping the head from also flipping which way the arc
/// bulges.
fn arrow_ends(p1: (i32, i32), p2: (i32, i32), head_at_end: bool) -> ((i32, i32), (i32, i32)) {
    if head_at_end { (p2, p1) } else { (p1, p2) }
}

/// Point on the arc at `t = 0.5`, which is where the handle rides.
///
/// For a quadratic Bezier with the control point at `M + perp * bend * chord`,
/// that midpoint works out to `M + perp * bend * chord / 2` — half the control
/// point's offset, and always on the curve rather than off it.
fn arc_midpoint(tip: (i32, i32), tail: (i32, i32), bend: f64) -> Option<(f64, f64)> {
    let (perp, chord_len) = chord_frame(tip, tail)?;
    let mid_x = (tip.0 as f64 + tail.0 as f64) / 2.0;
    let mid_y = (tip.1 as f64 + tail.1 as f64) / 2.0;
    let offset = util::clamp_arrow_bend(bend) * chord_len / 2.0;
    Some((mid_x + perp.0 * offset, mid_y + perp.1 * offset))
}

/// The bend that would put the arc's midpoint under `pointer`.
///
/// Inverts [`arc_midpoint`]: the perpendicular distance from the chord's
/// midpoint is half the control offset, so the bend is twice that distance over
/// the chord length. Distance along the chord is ignored, which is what keeps
/// the arc symmetric no matter where along it the user grabs.
fn bend_for_pointer(tip: (i32, i32), tail: (i32, i32), pointer: (i32, i32)) -> Option<f64> {
    let (perp, chord_len) = chord_frame(tip, tail)?;
    let mid_x = (tip.0 as f64 + tail.0 as f64) / 2.0;
    let mid_y = (tip.1 as f64 + tail.1 as f64) / 2.0;
    let offset = (pointer.0 as f64 - mid_x) * perp.0 + (pointer.1 as f64 - mid_y) * perp.1;
    Some(2.0 * offset / chord_len)
}

/// Left normal of the tail-to-tip direction, and the chord length.
///
/// Shares `util::arrow`'s normal rather than deriving one, so the handle and
/// the arc it edits cannot disagree about which way a positive bend bulges and
/// leave the arrow curving away from the drag.
fn chord_frame(tip: (i32, i32), tail: (i32, i32)) -> Option<((f64, f64), f64)> {
    util::chord_normal((tail.0 as f64, tail.1 as f64), (tip.0 as f64, tip.1 as f64))
}

#[cfg(test)]
mod tests;
