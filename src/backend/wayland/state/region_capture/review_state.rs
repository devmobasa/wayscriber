use super::*;

pub(super) struct InteractiveReviewSeed {
    pub(super) generation: u64,
    pub(super) source: ScreenSourceToken,
    pub(super) rect: ImagePixelRect,
    #[cfg(test)]
    pub(super) display: RegionSelection,
}

impl InteractiveReviewSeed {
    pub(super) fn into_edits(self) -> RegionReviewEdits {
        RegionReviewEdits::new(
            super::cut_review::RegionReviewCorrelation {
                generation: self.generation,
                source: self.source,
            },
            self.rect,
        )
    }
}

impl ActiveScreenRegion {
    pub(super) fn enter_review_seed(
        &mut self,
        rect: ImagePixelRect,
    ) -> Option<InteractiveReviewSeed> {
        let Self::Ready {
            purpose,
            generation,
            source,
            anchor,
            raw_edge,
            logical_anchor,
            logical_edge,
            review_resize,
            phase,
            ..
        } = self
        else {
            return None;
        };
        if *purpose != RegionPurposeTag::CaptureInteractive {
            return None;
        }
        ImagePixelRect::new(
            rect.x(),
            rect.y(),
            rect.width(),
            rect.height(),
            source.image_size,
        )?;
        let seed = InteractiveReviewSeed {
            generation: *generation,
            source: *source,
            rect,
            #[cfg(test)]
            display: {
                let display = super::super::screen_image::screen_rect_for_image_rect(source, rect);
                RegionSelection {
                    start: (f64::from(display.x), f64::from(display.y)),
                    end: (
                        f64::from(display.x.saturating_add(display.width)),
                        f64::from(display.y.saturating_add(display.height)),
                    ),
                }
            },
        };
        // Re-entering Review replaces the rectangle wholesale — `Ctrl+A` can
        // do that while a grip is still held — so the old grip must not
        // survive to block the next move, resize or nudge.
        *review_resize = None;
        *phase = RegionInteractionPhase::Review { owner: None };
        *anchor = None;
        *raw_edge = None;
        *logical_anchor = None;
        *logical_edge = None;
        *anchor = Some(ImagePoint::new(f64::from(rect.x()), f64::from(rect.y())));
        *raw_edge = Some(ImagePoint::new(
            f64::from(rect.x() + rect.width()),
            f64::from(rect.y() + rect.height()),
        ));
        Some(seed)
    }

    pub(super) fn begin_review_move(&mut self, logical: (f64, f64)) -> bool {
        let Some(rect) = (*self).stored_review_rect() else {
            return false;
        };
        let Self::Ready {
            source,
            logical_anchor,
            review_resize,
            ..
        } = self
        else {
            return false;
        };
        if logical_anchor.is_some() || review_resize.is_some() {
            return false;
        }
        let display =
            crate::backend::wayland::state::screen_image::screen_rect_for_image_rect(source, rect);
        let x = logical.0.floor() as i32;
        let y = logical.1.floor() as i32;
        if x < display.x
            || y < display.y
            || x >= display.x.saturating_add(display.width)
            || y >= display.y.saturating_add(display.height)
        {
            return false;
        }
        let origin = image_point_for_screen_point(source, logical);
        *logical_anchor = Some((origin.x, origin.y));
        true
    }

    pub(super) fn update_review_move(&mut self, logical: (f64, f64)) -> bool {
        let Some(image_rect) = (*self).stored_review_rect() else {
            return false;
        };
        let Self::Ready {
            source,
            logical_anchor: Some(origin),
            ..
        } = self
        else {
            return false;
        };
        let current = image_point_for_screen_point(source, logical);
        let delta_x = (current.x - origin.0).round() as i64;
        let delta_y = (current.y - origin.1).round() as i64;
        if delta_x == 0 && delta_y == 0 {
            return false;
        }
        let Some(next) = image_rect.translated_clamped(delta_x, delta_y, source.image_size) else {
            return false;
        };
        if image_rect == next {
            return false;
        }
        let applied_x = i64::from(next.x()) - i64::from(image_rect.x());
        let applied_y = i64::from(next.y()) - i64::from(image_rect.y());
        let next_origin = (origin.0 + applied_x as f64, origin.1 + applied_y as f64);
        if !self.store_review_rect(next) {
            return false;
        }
        if let Self::Ready { logical_anchor, .. } = self {
            *logical_anchor = Some(next_origin);
        }
        true
    }

    pub(super) fn finish_review_move(&mut self) -> bool {
        let Self::Ready { logical_anchor, .. } = self else {
            return false;
        };
        if logical_anchor.is_none() {
            return false;
        }
        *logical_anchor = None;
        true
    }

    pub(in crate::backend::wayland::state) fn review_resize_handle(
        self,
    ) -> Option<SelectionHandle> {
        match self {
            Self::Ready { review_resize, .. } => review_resize.map(ReviewResizeGrip::handle),
            Self::Measure { .. } | Self::PendingFrozen { .. } | Self::PendingZoom { .. } => None,
        }
    }

    pub(super) fn begin_review_resize(
        &mut self,
        handle: SelectionHandle,
        logical: (f64, f64),
    ) -> bool {
        let Some(rect) = (*self).stored_review_rect() else {
            return false;
        };
        let Self::Ready {
            source,
            logical_anchor,
            review_resize,
            ..
        } = self
        else {
            return false;
        };
        if logical_anchor.is_some() || review_resize.is_some() {
            return false;
        }
        let pointer = image_point_for_screen_point(source, logical);
        let Some(grip) = ReviewResizeGrip::grab(handle, rect, pointer) else {
            return false;
        };
        *review_resize = Some(grip);
        true
    }

    /// Move the edges the held grip owns, keeping the offset captured when the
    /// grip was grabbed, so the edge tracks the pointer without jumping to it.
    pub(super) fn update_review_resize(&mut self, logical: (f64, f64)) -> bool {
        let Some(rect) = (*self).stored_review_rect() else {
            return false;
        };
        let Self::Ready {
            source,
            review_resize: Some(grip),
            ..
        } = *self
        else {
            return false;
        };
        let edge = grip.edge_for(image_point_for_screen_point(&source, logical));
        let Some(next) = resized_review_rect(rect, grip.handle, edge, source.image_size) else {
            return false;
        };
        if next == rect {
            return false;
        }
        self.store_review_rect(next)
    }

    pub(super) fn finish_review_resize(&mut self) -> bool {
        let Self::Ready { review_resize, .. } = self else {
            return false;
        };
        if review_resize.is_none() {
            return false;
        }
        *review_resize = None;
        true
    }

    pub(super) fn reset_review_for_selection(&mut self) {
        if let Self::Ready {
            anchor,
            raw_edge,
            logical_anchor,
            review_resize,
            ..
        } = self
        {
            *anchor = None;
            *raw_edge = None;
            *logical_anchor = None;
            *review_resize = None;
        }
    }

    pub(super) fn nudge_review(&mut self, delta_x: i64, delta_y: i64) -> Option<RegionSelection> {
        let review_rect = (*self).stored_review_rect()?;
        let Self::Ready {
            source,
            logical_anchor,
            review_resize,
            ..
        } = self
        else {
            return None;
        };
        if logical_anchor.is_some() || review_resize.is_some() {
            return None;
        }
        let rect = review_rect.translated_clamped(delta_x, delta_y, source.image_size)?;
        self.store_review_rect(rect);
        self.review_geometry()
            .map(|geometry| geometry.display_selection())
    }
}

/// The smallest rectangle a resize may leave behind, in image pixels. A grip
/// dragged past its opposite edge stops here instead of collapsing or
/// inverting the rectangle, so the grip under the pointer keeps its identity
/// for the whole drag.
const MIN_REVIEW_SIDE: u32 = 1;

/// A grip being dragged, with the offset between the pointer and the edges it
/// owns, captured when the grip was grabbed.
///
/// The offset is what keeps a click-without-motion inert. Grips are placed on
/// the *display* rectangle, which is the image rectangle rounded outward, so
/// at a fractional scale or under zoom the pointer's image coordinate on the
/// chip is not the edge's own coordinate. Without the offset, the release
/// event alone would re-round that position and shift the edge by a pixel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::backend::wayland::state) struct ReviewResizeGrip {
    handle: SelectionHandle,
    offset: (f64, f64),
}

impl ReviewResizeGrip {
    const fn handle(self) -> SelectionHandle {
        self.handle
    }

    fn grab(handle: SelectionHandle, rect: ImagePixelRect, pointer: ImagePoint) -> Option<Self> {
        if !pointer.x.is_finite() || !pointer.y.is_finite() {
            return None;
        }
        let (moves_left, moves_right) = handle_owns_horizontal(handle);
        let (moves_top, moves_bottom) = handle_owns_vertical(handle);
        let right = f64::from(rect.x().checked_add(rect.width())?);
        let bottom = f64::from(rect.y().checked_add(rect.height())?);
        // An axis the grip does not own contributes no offset, so a stray
        // component of the pointer position can never leak into it.
        let offset_x = match (moves_left, moves_right) {
            (true, _) => pointer.x - f64::from(rect.x()),
            (_, true) => pointer.x - right,
            _ => 0.0,
        };
        let offset_y = match (moves_top, moves_bottom) {
            (true, _) => pointer.y - f64::from(rect.y()),
            (_, true) => pointer.y - bottom,
            _ => 0.0,
        };
        Some(Self {
            handle,
            offset: (offset_x, offset_y),
        })
    }

    /// Where the owned edges should sit for a pointer now at `pointer`.
    fn edge_for(self, pointer: ImagePoint) -> ImagePoint {
        ImagePoint::new(pointer.x - self.offset.0, pointer.y - self.offset.1)
    }
}

const fn handle_owns_horizontal(handle: SelectionHandle) -> (bool, bool) {
    match handle {
        SelectionHandle::TopLeft | SelectionHandle::BottomLeft | SelectionHandle::Left => {
            (true, false)
        }
        SelectionHandle::TopRight | SelectionHandle::BottomRight | SelectionHandle::Right => {
            (false, true)
        }
        SelectionHandle::Top | SelectionHandle::Bottom => (false, false),
    }
}

const fn handle_owns_vertical(handle: SelectionHandle) -> (bool, bool) {
    match handle {
        SelectionHandle::TopLeft | SelectionHandle::TopRight | SelectionHandle::Top => {
            (true, false)
        }
        SelectionHandle::BottomLeft | SelectionHandle::BottomRight | SelectionHandle::Bottom => {
            (false, true)
        }
        SelectionHandle::Left | SelectionHandle::Right => (false, false),
    }
}

/// Apply a grip drag to `rect`: the edges the grip owns follow `edge`, the
/// opposite ones stay put. Returns `None` only when the result cannot be a
/// valid rectangle inside `bounds`.
pub(super) fn resized_review_rect(
    rect: ImagePixelRect,
    handle: SelectionHandle,
    edge: ImagePoint,
    bounds: (u32, u32),
) -> Option<ImagePixelRect> {
    let mut left = rect.x();
    let mut top = rect.y();
    let mut right = rect.x().checked_add(rect.width())?;
    let mut bottom = rect.y().checked_add(rect.height())?;
    let pointer_x = clamp_image_coordinate(edge.x, bounds.0)?;
    let pointer_y = clamp_image_coordinate(edge.y, bounds.1)?;
    let (moves_left, moves_right) = handle_owns_horizontal(handle);
    let (moves_top, moves_bottom) = handle_owns_vertical(handle);

    if moves_left {
        left = pointer_x.min(right.saturating_sub(MIN_REVIEW_SIDE));
    }
    if moves_right {
        right = pointer_x
            .max(left.saturating_add(MIN_REVIEW_SIDE))
            .min(bounds.0);
    }
    if moves_top {
        top = pointer_y.min(bottom.saturating_sub(MIN_REVIEW_SIDE));
    }
    if moves_bottom {
        bottom = pointer_y
            .max(top.saturating_add(MIN_REVIEW_SIDE))
            .min(bounds.1);
    }

    ImagePixelRect::new(
        left,
        top,
        right.checked_sub(left)?,
        bottom.checked_sub(top)?,
        bounds,
    )
}

/// Quantize a fractional image coordinate onto the pixel grid, clamped to the
/// closed edge range so a rectangle may end exactly at the image boundary.
///
/// A finite coordinate far off-screen is a real drag and clamps; a non-finite
/// one is a broken event and yields `None`, so the caller ignores it instead of
/// snapping the edge to the image origin.
fn clamp_image_coordinate(value: f64, bound: u32) -> Option<u32> {
    if !value.is_finite() {
        return None;
    }
    if value <= 0.0 {
        return Some(0);
    }
    let rounded = value.round();
    if rounded >= f64::from(bound) {
        return Some(bound);
    }
    Some(rounded as u32)
}
