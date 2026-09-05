//! Gathering the spotlight regions the compositing pass needs.
//!
//! The pass has to know every region before it paints, because it builds one dim
//! layer and punches all the openings out of it. That makes spotlights the only
//! shape kind the renderer collects up front instead of drawing in z-order.

use crate::draw::{Shape, ShapeId, SpotlightRegion, TextMeasurer, spotlight_regions_for_frame};
use crate::input::Tool;

use super::{DrawingState, InputState};

/// Every spotlight one frame must dim, collected in a single pass.
pub(crate) struct SpotlightFrameRegions {
    /// Committed regions first, then the in-progress drag when there is one.
    pub(crate) regions: Vec<SpotlightRegion>,
    /// Whether a *committed* shape is magnified.
    ///
    /// The in-progress drag is excluded on purpose: warnings that describe
    /// what a page holds must not fire for an ellipse the user is still
    /// dragging out, which cancelling would leave nothing behind for.
    pub(crate) committed_magnified: bool,
}

/// Which frame a per-shape gesture belongs to.
///
/// Shape ids are frame-local and restart per page, so a gesture that outlives
/// its frame must be discarded rather than applied to whatever now holds that
/// id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrameIdentity {
    /// Bumped whenever the set of boards is replaced, so a restored session
    /// cannot present itself as the same board list.
    board_identity: crate::input::boards::BoardIdentityGeneration,
    board_id: String,
    page_index: usize,
    /// Bumped whenever pages are added or removed, so deleting a page and
    /// landing a different one on the same index is not mistaken for the page
    /// the gesture started on.
    page_generation: u64,
}

impl FrameIdentity {
    pub(crate) fn of(boards: &crate::input::boards::BoardManager) -> Self {
        Self {
            board_identity: boards.board_identity_generation(),
            board_id: boards.active_board_id().to_string(),
            page_index: boards.active_page_index(),
            page_generation: boards.active_page_generation(),
        }
    }
}

/// An in-flight wheel adjustment of one loupe's magnification.
#[derive(Debug, Clone)]
struct SpotlightMagnificationGesture {
    /// Frame the gesture started on; it may only ever be committed there.
    frame: FrameIdentity,
    shape_id: ShapeId,
    /// Factor before the first tick, so the whole burst is one undo entry.
    before: crate::draw::frame::ShapeSnapshot,
}

#[derive(Debug, Clone, Default)]
pub(in crate::input::state) struct SpotlightWheelGesture {
    gesture: Option<SpotlightMagnificationGesture>,
    value120_remainder: Option<(ShapeId, i32)>,
}

impl SpotlightWheelGesture {
    fn is_pending(&self) -> bool {
        self.gesture.is_some()
    }

    fn owns_wheel(&self) -> bool {
        self.gesture.is_some() || self.value120_remainder.is_some()
    }

    fn begin_with(&mut self, gesture: impl FnOnce() -> SpotlightMagnificationGesture) {
        self.gesture.get_or_insert_with(gesture);
    }

    fn accumulate_value120(&mut self, shape_id: ShapeId, delta: i32) -> i32 {
        let previous = self
            .value120_remainder
            .filter(|(owner, _)| *owner == shape_id)
            .map_or(0, |(_, remainder)| remainder);
        let total = i64::from(previous) + i64::from(delta);
        let steps = total / 120;
        let remainder = (total % 120) as i32;
        self.value120_remainder = (remainder != 0).then_some((shape_id, remainder));
        steps as i32
    }

    fn clear_remainder(&mut self) {
        self.value120_remainder = None;
    }

    fn take(&mut self) -> Option<SpotlightMagnificationGesture> {
        self.clear_remainder();
        self.gesture.take()
    }

    fn shape_id(&self) -> Option<ShapeId> {
        let gesture = self.gesture.as_ref().map(|gesture| gesture.shape_id);
        let remainder = self.value120_remainder.map(|(shape_id, _)| shape_id);
        match (gesture, remainder) {
            (Some(gesture), Some(remainder)) if gesture != remainder => None,
            (Some(shape_id), _) | (_, Some(shape_id)) => Some(shape_id),
            (None, None) => None,
        }
    }

    fn gesture_shape_id(&self) -> Option<ShapeId> {
        self.gesture.as_ref().map(|gesture| gesture.shape_id)
    }

    fn is_owned_by(&self, shape_id: ShapeId) -> bool {
        self.shape_id() == Some(shape_id)
    }
}

/// What a wheel tick over the canvas did.
///
/// `NotOverLoupe` is the only outcome that lets the wheel keep its usual
/// meaning. A locked loupe and an adjustment that could not move — already at
/// 1x or 4x — still own the wheel rather than resizing a brush behind them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpotlightWheelOutcome {
    NotOverLoupe,
    Locked,
    AtRangeEnd,
    Adjusted,
}

/// Whether a vertical axis frame belongs to a loupe, and how many complete
/// magnification steps it contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpotlightWheelClaim {
    NotOverLoupe,
    Locked,
    Adjustable(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpotlightWheelTarget {
    Adjustable(ShapeId),
    Locked,
}

/// The loupe factor recorded in a shape snapshot, to the bit.
///
/// Compared rather than the whole `Shape`, which has no `PartialEq`, and the
/// factor is the only field either gesture touches.
fn snapshot_magnification(snapshot: &crate::draw::frame::ShapeSnapshot) -> Option<u64> {
    match snapshot.shape {
        Shape::Spotlight { magnification, .. } => Some(magnification.to_bits()),
        _ => None,
    }
}

impl InputState {
    /// Whether a live wheel burst still owes its single undo entry.
    pub(crate) fn has_pending_spotlight_magnification_gesture(&self) -> bool {
        self.spotlight_wheel.is_pending()
    }

    /// Whether a live wheel burst still owns an undo gesture or a partial
    /// high-resolution wheel step.
    pub(crate) fn has_pending_spotlight_wheel_axis_sequence(&self) -> bool {
        self.spotlight_wheel.owns_wheel()
    }

    /// Every committed spotlight on the active page, plus the one being dragged
    /// when `cursor` is given.
    ///
    /// Including the in-progress drag is what makes the tool usable: the dimming
    /// follows the drag instead of appearing only once the button is released.
    /// `None` asks for committed regions only, which is what a frame that
    /// suppresses transients draws.
    ///
    /// Both answers come from one collection: the render path needs the region
    /// list and the committed-magnification fact on every frame, and scanning
    /// the page twice for them would be pure waste.
    pub(crate) fn spotlight_frame_regions(
        &self,
        cursor: Option<(i32, i32)>,
    ) -> SpotlightFrameRegions {
        let mut regions = spotlight_regions_for_frame(self.boards.active_frame());
        let committed_magnified = regions
            .iter()
            .any(|region| crate::draw::spotlight_magnification_is_active(region.magnification));

        regions.extend(cursor.and_then(|cursor| self.provisional_spotlight_region(cursor)));
        SpotlightFrameRegions {
            regions,
            committed_magnified,
        }
    }

    /// Topmost committed Spotlight whose ellipse contains the canvas point.
    ///
    /// Shapes are stored in z-order, so the search runs backwards: the loupe
    /// drawn last is the one the pointer is visually over.
    fn spotlight_wheel_target_at(&self, x: i32, y: i32) -> Option<SpotlightWheelTarget> {
        let frame = self.boards.active_frame();
        frame.shapes.iter().rev().find_map(|drawn| {
            let Shape::Spotlight { cx, cy, rx, ry, .. } = drawn.shape else {
                return None;
            };
            // Use the same predicate as ordinary hit testing. A former copy
            // disagreed about degenerate radii and the boundary epsilon, so a
            // loupe could be clickable and yet not answer the wheel.
            if !crate::input::hit_test::ellipse_fill_hit(cx, cy, rx, ry, (x, y)) {
                return None;
            }
            Some(if drawn.locked {
                SpotlightWheelTarget::Locked
            } else {
                SpotlightWheelTarget::Adjustable(drawn.id)
            })
        })
    }

    /// Claims a vertical axis frame for the topmost loupe and converts it to
    /// 0.25x magnification steps.
    ///
    /// `value120` is accumulated without rounding: 120 logical units become
    /// one step, coalesced multiples stay multiples, and a partial unit remains
    /// owned by this loupe until a later frame completes or ends the sequence.
    pub(crate) fn claim_spotlight_wheel_axis_at(
        &mut self,
        x: i32,
        y: i32,
        value120: i32,
        discrete: i32,
        absolute: f64,
    ) -> SpotlightWheelClaim {
        let shape_id = match self.spotlight_wheel_target_at(x, y) {
            Some(SpotlightWheelTarget::Adjustable(shape_id)) => shape_id,
            Some(SpotlightWheelTarget::Locked) => {
                self.flush_spotlight_magnification_gesture();
                return SpotlightWheelClaim::Locked;
            }
            None => {
                self.flush_spotlight_magnification_gesture();
                return SpotlightWheelClaim::NotOverLoupe;
            }
        };

        if self.spotlight_wheel.owns_wheel() && !self.spotlight_wheel.is_owned_by(shape_id) {
            self.flush_spotlight_magnification_gesture();
        }

        let steps = if value120 != 0 {
            // Positive Wayland axis values scroll down; Spotlight
            // magnification increases when the user scrolls up.
            -self.spotlight_wheel.accumulate_value120(shape_id, value120)
        } else {
            // A source using the legacy discrete/continuous representation is
            // a separate unit stream. Do not carry a value120 fraction into it.
            self.spotlight_wheel.clear_remainder();
            if discrete != 0 {
                discrete.saturating_neg()
            } else if absolute > 0.1 {
                -1
            } else if absolute < -0.1 {
                1
            } else {
                0
            }
        };
        SpotlightWheelClaim::Adjustable(steps)
    }

    #[cfg(test)]
    pub(crate) fn spotlight_at(&self, x: i32, y: i32) -> Option<ShapeId> {
        match self.spotlight_wheel_target_at(x, y) {
            Some(SpotlightWheelTarget::Adjustable(shape_id)) => Some(shape_id),
            Some(SpotlightWheelTarget::Locked) | None => None,
        }
    }

    /// Writes a new factor onto one Spotlight and repaints, without recording
    /// undo.
    ///
    /// Undo granularity belongs to the gesture, not to each step: a wheel burst
    /// and a knob drag are each one user action, so their callers snapshot at
    /// the start and push a single entry at the end.
    pub(crate) fn set_spotlight_shape_magnification_with(
        &mut self,
        measurer: &TextMeasurer,
        shape_id: ShapeId,
        magnification: f64,
    ) -> bool {
        let normalized = crate::draw::normalize_spotlight_magnification(magnification);
        let frame = self.boards.active_frame_mut();
        let Some(drawn) = frame.shape_mut(shape_id) else {
            return false;
        };
        let Shape::Spotlight {
            magnification: current,
            ..
        } = &mut drawn.shape
        else {
            return false;
        };
        if (*current - normalized).abs() <= f64::EPSILON {
            return false;
        }
        *current = normalized;
        let bounds = drawn.bounding_box_with(measurer);
        self.mark_selection_dirty_region(bounds);
        self.invalidate_hit_cache_for_with(measurer, shape_id);
        self.mark_session_dirty();
        self.needs_redraw = true;
        true
    }

    /// Steps the magnification of the Spotlight under the pointer.
    ///
    /// The wheel is the cheapest way to reach this property: no selection, no
    /// toolbar trip, and the loupe follows the ticks live. Returns whether
    /// anything changed, so the caller can fall through to its usual wheel
    /// behaviour when the pointer is not over a loupe.
    pub(crate) fn nudge_spotlight_magnification_at_with(
        &mut self,
        measurer: &TextMeasurer,
        x: i32,
        y: i32,
        steps: i32,
    ) -> SpotlightWheelOutcome {
        let shape_id = match self.spotlight_wheel_target_at(x, y) {
            Some(SpotlightWheelTarget::Adjustable(shape_id)) => shape_id,
            Some(SpotlightWheelTarget::Locked) => {
                self.flush_spotlight_magnification_gesture();
                return SpotlightWheelOutcome::Locked;
            }
            None => {
                // Leaving the loupe ends the gesture, so the next burst over it is
                // separately undoable.
                self.flush_spotlight_magnification_gesture();
                return SpotlightWheelOutcome::NotOverLoupe;
            }
        };
        if self
            .spotlight_wheel
            .gesture_shape_id()
            .is_some_and(|owner| owner != shape_id)
        {
            self.flush_spotlight_magnification_gesture();
        }

        let Some(drawn) = self.boards.active_frame().shape(shape_id) else {
            return SpotlightWheelOutcome::NotOverLoupe;
        };
        let Shape::Spotlight { magnification, .. } = drawn.shape else {
            return SpotlightWheelOutcome::NotOverLoupe;
        };
        let before = crate::draw::frame::ShapeSnapshot {
            shape: drawn.shape.clone(),
            locked: drawn.locked,
        };
        // Snapped, not just stepped: a shape that somehow sits between steps
        // — an older session, a hand-edited file — is pulled back onto the
        // grid by the first tick instead of carrying its offset forever.
        let target = crate::ui::toolbar::model::ToolbarSliderSpec::SPOTLIGHT_MAGNIFICATION
            .normalize_value(
                magnification + crate::draw::SPOTLIGHT_MAGNIFICATION_STEP * f64::from(steps),
            );
        if !self.set_spotlight_shape_magnification_with(measurer, shape_id, target) {
            // An end of the range. The wheel still belongs to this loupe, so
            // the caller must not fall through to thickness: the pointer is
            // over a loupe and the user asked it to go further, not to resize
            // a brush.
            return SpotlightWheelOutcome::AtRangeEnd;
        }
        let boards = &self.boards;
        self.spotlight_wheel
            .begin_with(|| SpotlightMagnificationGesture {
                frame: FrameIdentity::of(boards),
                shape_id,
                before,
            });
        if crate::draw::spotlight_magnification_is_active(target) {
            self.request_spotlight_magnifier_feedback();
        }
        SpotlightWheelOutcome::Adjusted
    }

    /// Ends an in-flight wheel sequence once the pointer is no longer over the
    /// loupe that owns either its undo gesture or its partial logical step.
    ///
    /// Without this a visit minutes later would merge into the same undo entry,
    /// because nothing else runs between two wheel bursts over one shape.
    pub(crate) fn end_spotlight_magnification_gesture_if_pointer_left(&mut self, x: i32, y: i32) {
        let target = self.spotlight_wheel_target_at(x, y);
        let owner_left = match target {
            Some(SpotlightWheelTarget::Adjustable(shape_id)) => {
                self.spotlight_wheel.owns_wheel() && !self.spotlight_wheel.is_owned_by(shape_id)
            }
            Some(SpotlightWheelTarget::Locked) | None => self.spotlight_wheel.owns_wheel(),
        };
        if owner_left {
            self.flush_spotlight_magnification_gesture();
        }
    }

    /// Closes an in-flight wheel adjustment, recording the whole burst as one
    /// undo entry.
    ///
    /// Called before anything that would make a half-finished gesture
    /// confusing to undo: a pointer press, an undo, a redo.
    pub(crate) fn flush_spotlight_magnification_gesture(&mut self) {
        let Some(gesture) = self.spotlight_wheel.take() else {
            return;
        };
        let SpotlightMagnificationGesture {
            frame,
            shape_id,
            before,
        } = gesture;
        // Shape ids restart per frame, so an entry pushed after a page or board
        // change would pair this snapshot with an unrelated shape and corrupt
        // the destination page's history. Every transition flushes first, so
        // reaching here on a different frame means the gesture is already lost;
        // drop it rather than write it somewhere it does not belong.
        if frame != FrameIdentity::of(&self.boards) {
            return;
        }
        let Some(drawn) = self.boards.active_frame().shape(shape_id) else {
            return;
        };
        let after = crate::draw::frame::ShapeSnapshot {
            shape: drawn.shape.clone(),
            locked: drawn.locked,
        };
        self.record_spotlight_magnification_change(shape_id, before, after);
    }

    /// Records one completed magnification gesture, wheel or knob, as a single
    /// undo entry.
    ///
    /// Shared so the two gestures cannot disagree about when a change is worth
    /// recording. Only the factor can have moved, and a gesture that lands back
    /// where it started is not worth an entry.
    pub(crate) fn record_spotlight_magnification_change(
        &mut self,
        shape_id: ShapeId,
        before: crate::draw::frame::ShapeSnapshot,
        after: crate::draw::frame::ShapeSnapshot,
    ) {
        let after_factor = snapshot_magnification(&after);
        if snapshot_magnification(&before) == after_factor {
            return;
        }
        let limit = self.history_limits.undo_stack_limit();
        self.boards.active_frame_mut().push_undo_action(
            crate::draw::frame::UndoAction::Modify {
                shape_id,
                before,
                after,
            },
            limit,
        );
        self.mark_session_dirty();
        if after_factor
            .map(f64::from_bits)
            .is_some_and(crate::draw::spotlight_magnification_is_active)
        {
            self.request_spotlight_magnifier_feedback();
        }
    }

    /// The spotlight currently being dragged out, if the spotlight tool is active.
    pub(crate) fn provisional_spotlight_region(
        &self,
        cursor: (i32, i32),
    ) -> Option<SpotlightRegion> {
        let DrawingState::Drawing {
            tool,
            start_x,
            start_y,
            ..
        } = &self.state
        else {
            return None;
        };
        if *tool != Tool::Spotlight {
            return None;
        }

        let (cx, cy, rx, ry) = crate::util::ellipse_bounds(*start_x, *start_y, cursor.0, cursor.1);
        Some(SpotlightRegion {
            cx: f64::from(cx),
            cy: f64::from(cy),
            rx: f64::from(rx),
            ry: f64::from(ry),
            magnification: crate::draw::normalize_spotlight_magnification(
                self.style.spotlight_magnification,
            ),
        })
    }

    /// Highest magnification among the currently selected Spotlights.
    ///
    /// `None` when the selection holds no Spotlight at all. The docked
    /// selection control reports availability against this rather than the
    /// next-shape default, which is a different number whenever the user
    /// selects an existing shape.
    pub fn selection_spotlight_magnification(&self) -> Option<f64> {
        let frame = self.boards.active_frame();
        self.selected_shape_ids()
            .iter()
            .filter_map(|id| match frame.shape(*id)?.shape {
                Shape::Spotlight { magnification, .. } => Some(
                    crate::draw::normalize_spotlight_magnification(magnification),
                ),
                _ => None,
            })
            .reduce(f64::max)
    }

    /// Whether anything on the active page dims the canvas.
    ///
    /// Drives the full-damage decision: a spotlight changes every pixel outside
    /// itself, so partial damage cannot describe adding, moving, or removing one.
    pub(crate) fn has_spotlight(&self) -> bool {
        self.boards
            .active_frame()
            .shapes
            .iter()
            .any(|drawn| matches!(drawn.shape, Shape::Spotlight { .. }))
            || matches!(
                &self.state,
                DrawingState::Drawing {
                    tool: Tool::Spotlight,
                    ..
                }
            )
    }
}

#[cfg(test)]
mod wheel_gesture_tests {
    use super::*;

    fn gesture(shape_id: ShapeId) -> SpotlightMagnificationGesture {
        SpotlightMagnificationGesture {
            frame: FrameIdentity {
                board_identity: crate::input::boards::BoardIdentityGeneration(1),
                board_id: "transparent".to_string(),
                page_index: 0,
                page_generation: 0,
            },
            shape_id,
            before: crate::draw::frame::ShapeSnapshot {
                shape: Shape::Spotlight {
                    cx: 20,
                    cy: 30,
                    rx: 10,
                    ry: 15,
                    magnification: 2.0,
                },
                locked: false,
            },
        }
    }

    #[test]
    fn value120_remainder_carries_for_one_shape_and_resets_for_another() {
        let mut wheel = SpotlightWheelGesture::default();

        assert_eq!(wheel.accumulate_value120(7, 60), 0);
        assert_eq!(wheel.accumulate_value120(7, 70), 1);
        assert_eq!(wheel.accumulate_value120(8, 110), 0);
        assert_eq!(wheel.shape_id(), Some(8));
        assert_eq!(wheel.accumulate_value120(8, 10), 1);
        assert_eq!(wheel.shape_id(), None);
    }

    #[test]
    fn take_clears_the_gesture_and_its_partial_wheel_step() {
        let mut wheel = SpotlightWheelGesture::default();
        wheel.begin_with(|| gesture(7));
        assert_eq!(wheel.accumulate_value120(7, 60), 0);
        assert!(wheel.is_pending());
        assert!(wheel.owns_wheel());

        assert!(wheel.take().is_some());
        assert!(!wheel.is_pending());
        assert!(!wheel.owns_wheel());
        assert_eq!(wheel.shape_id(), None);
    }

    #[test]
    fn begin_with_does_not_build_a_replacement_for_a_pending_gesture() {
        let mut wheel = SpotlightWheelGesture::default();
        let mut builds = 0;

        wheel.begin_with(|| {
            builds += 1;
            gesture(7)
        });
        wheel.begin_with(|| {
            builds += 1;
            gesture(8)
        });

        assert_eq!(builds, 1);
        assert_eq!(wheel.gesture_shape_id(), Some(7));
    }
}
