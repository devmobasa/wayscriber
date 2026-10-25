use crate::draw::{DrawnShape, ShapeId, TextMeasurer};
use crate::input::InputState;
use crate::util::Rect;

/// Combined painted bounds with exclusive maxima.
///
/// The edges are widened because a shape's painted bounds can end past
/// `i32::MAX`, and a union of distant shapes can be wider than [`Rect`] holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SelectionExtent {
    pub(super) min_x: i64,
    pub(super) min_y: i64,
    pub(super) max_x: i64,
    pub(super) max_y: i64,
}

impl SelectionExtent {
    fn combine<'frame>(
        shapes: impl Iterator<Item = &'frame DrawnShape>,
        measurer: &TextMeasurer,
    ) -> Option<Self> {
        shapes
            .filter_map(|drawn| Self::of_shape(drawn, measurer))
            .reduce(Self::union)
    }

    /// Painted bounds of one shape.
    ///
    /// Shape bounds are clamped to the `i32` range, so a shape at the
    /// coordinate limit is measured on a copy moved toward the origin. Its
    /// painted size does not depend on where it is.
    fn of_shape(drawn: &DrawnShape, measurer: &TextMeasurer) -> Option<Self> {
        let extent = Self::from_rect(drawn.bounding_box_with(measurer)?);
        if !extent.touches_coordinate_limit() {
            return Some(extent);
        }

        let (dx, dy) = drawn.shape.constrain_translation(
            saturating_delta(-(extent.min_x + extent.max_x) / 2),
            saturating_delta(-(extent.min_y + extent.max_y) / 2),
        );
        let mut copy = drawn.shape.clone();
        copy.translate(dx, dy);
        let Some(moved) = copy.bounding_box_with(measurer) else {
            return Some(extent);
        };
        let moved = Self::from_rect(moved);
        let (dx, dy) = (i64::from(dx), i64::from(dy));
        Some(Self {
            min_x: moved.min_x - dx,
            min_y: moved.min_y - dy,
            max_x: moved.max_x - dx,
            max_y: moved.max_y - dy,
        })
    }

    fn touches_coordinate_limit(self) -> bool {
        let min = i64::from(i32::MIN);
        let max = i64::from(i32::MAX);
        self.min_x <= min || self.min_y <= min || self.max_x > max || self.max_y > max
    }

    fn from_rect(rect: Rect) -> Self {
        let min_x = i64::from(rect.x);
        let min_y = i64::from(rect.y);
        Self {
            min_x,
            min_y,
            max_x: min_x + i64::from(rect.width),
            max_y: min_y + i64::from(rect.height),
        }
    }

    fn union(self, other: Self) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }

    /// Returns no [`Rect`] for bounds whose far edge ends past `i32::MAX`,
    /// because handle hit-testing and other selection code compute
    /// `x + width` in `i32`.
    fn to_rect(self) -> Option<Rect> {
        let limit = i64::from(i32::MAX);
        if self.max_x > limit || self.max_y > limit {
            return None;
        }

        let x = i32::try_from(self.min_x).ok()?;
        let y = i32::try_from(self.min_y).ok()?;
        let width = i32::try_from(self.width()).ok()?;
        let height = i32::try_from(self.height()).ok()?;
        Rect::new(x, y, width, height)
    }

    pub(super) fn width(self) -> i64 {
        self.max_x - self.min_x
    }

    pub(super) fn height(self) -> i64 {
        self.max_y - self.min_y
    }
}

impl InputState {
    /// Returns the combined bounding box of all selected shapes (public for rendering).
    pub fn selection_bounds(&self) -> Option<Rect> {
        let measurer = TextMeasurer::default();
        self.selection_bounds_with(&measurer)
    }

    /// Combined selection bounds using the supplied text measurement owner.
    pub fn selection_bounds_with(&self, measurer: &TextMeasurer) -> Option<Rect> {
        self.selection_bounding_box_with(measurer, self.selected_shape_ids())
    }

    /// Combined painted bounds of `ids`, or `None` when nothing is painted
    /// or the bounds do not fit a [`Rect`].
    pub(crate) fn selection_bounding_box_with(
        &self,
        measurer: &TextMeasurer,
        ids: &[ShapeId],
    ) -> Option<Rect> {
        self.painted_extent_with(measurer, ids)?.to_rect()
    }

    /// Whether any selected shape is painted, including when the combined
    /// bounds are too large for [`InputState::selection_bounds_with`].
    pub(crate) fn selection_is_painted_with(&self, measurer: &TextMeasurer) -> bool {
        self.painted_extent_with(measurer, self.selected_shape_ids())
            .is_some()
    }

    fn painted_extent_with(
        &self,
        measurer: &TextMeasurer,
        ids: &[ShapeId],
    ) -> Option<SelectionExtent> {
        let frame = self.boards.active_frame();
        let shapes = ids.iter().filter_map(|id| frame.shape(*id));
        SelectionExtent::combine(shapes, measurer)
    }

    /// Combined painted bounds of the selected shapes that a move can change.
    pub(super) fn movable_selection_extent(
        &self,
        measurer: &TextMeasurer,
    ) -> Option<SelectionExtent> {
        let frame = self.boards.active_frame();
        let shapes = self
            .selected_shape_ids()
            .iter()
            .filter_map(|id| frame.shape(*id))
            .filter(|drawn| !drawn.locked);
        SelectionExtent::combine(shapes, measurer)
    }

    /// Limits a requested selection move to one delta shared by every
    /// unlocked selected shape.
    ///
    /// A pannable solid board has an unbounded canvas, so the viewport is not
    /// a movement boundary there. The transparent overlay, and a solid board
    /// with panning disabled, keep a fixed canvas the size of the screen. On
    /// every board the move is then shortened so that each translated
    /// coordinate and the painted bounds stay representable. Limits only
    /// shorten a move; they never reverse it.
    pub(super) fn clamp_selection_translation(
        &self,
        measurer: &TextMeasurer,
        dx: i32,
        dy: i32,
    ) -> Option<(i32, i32)> {
        let extent = self.movable_selection_extent(measurer);
        let (dx, dy) = if self.boards.pan_enabled() && !self.board_is_transparent() {
            (dx, dy)
        } else {
            let extent = extent?;
            let (screen_width, screen_height) = self.view.screen_size();
            // An unknown screen dimension leaves that axis unclamped.
            let dx = if screen_width == 0 {
                dx
            } else {
                clamp_delta(
                    dx,
                    fixed_canvas_range(extent.min_x, extent.max_x, i64::from(screen_width)),
                )
            };
            let dy = if screen_height == 0 {
                dy
            } else {
                clamp_delta(
                    dy,
                    fixed_canvas_range(extent.min_y, extent.max_y, i64::from(screen_height)),
                )
            };
            (dx, dy)
        };

        let frame = self.boards.active_frame();
        let unlocked = self
            .selected_shape_ids()
            .iter()
            .filter_map(|id| frame.shape(*id))
            .filter(|drawn| !drawn.locked);
        let (dx, dy) = unlocked.fold((dx, dy), |(dx, dy), drawn| {
            drawn.shape.constrain_translation(dx, dy)
        });

        // Keep the painted bounds inside the i32 range too, because selection
        // code reads them back as a `Rect`.
        let Some(extent) = extent else {
            return Some((dx, dy));
        };
        let min = i64::from(i32::MIN);
        let max = i64::from(i32::MAX);
        Some((
            clamp_delta(dx, (min - extent.min_x, max - extent.max_x)),
            clamp_delta(dy, (min - extent.min_y, max - extent.max_y)),
        ))
    }
}

/// Saturates a wide delta to one `i32` step.
pub(super) fn saturating_delta(delta: i64) -> i32 {
    i32::try_from(delta).unwrap_or(if delta.is_negative() {
        i32::MIN
    } else {
        i32::MAX
    })
}

/// Movement range along one axis of a fixed canvas `0..canvas`.
///
/// A selection that fits may move until it touches either edge; a wider one
/// may slide while it still covers the canvas.
fn fixed_canvas_range(min: i64, max: i64, canvas: i64) -> (i64, i64) {
    if max - min <= canvas {
        (-min, canvas - max)
    } else {
        (canvas - max, -min)
    }
}

/// Clamps `delta` to `low..=high` after widening that range to contain zero.
///
/// A selection that already crosses a limit, or lies entirely past it,
/// therefore cannot move further out, but it is never pulled back against the
/// requested direction.
fn clamp_delta(delta: i32, (low, high): (i64, i64)) -> i32 {
    let clamped = i64::from(delta).clamp(low.min(0), high.max(0));
    i32::try_from(clamped)
        .expect("a range containing zero keeps the delta between zero and its i32 value")
}
