//! Which on-canvas handle a point belongs to.
//!
//! The handles a selection can show overlap: a curved arrow's bend grip rides
//! the arc's midpoint, which on a shallow curve lands within a few pixels of
//! the selection box's edge handle. Whichever probe runs first wins, so the
//! order is a real decision and not an implementation detail.
//!
//! It is also a decision that has to be made once. A press and the cursor that
//! previews it are answered in different layers — `handle_idle_tool_click` in
//! the input state, the compositor's pointer handler in the backend — and when
//! those layers each kept their own list, they disagreed: the pointer showed a
//! resize arrow over a grip that a click would bend. Both now ask this.

use crate::draw::ShapeId;
use crate::draw::frame::ShapeSnapshot;
use crate::input::InputState;
use crate::input::state::core::base::SelectionHandle;

/// The handle under a point, or `None` when the point is on none of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdleHandle {
    /// A selected loupe's magnification track.
    SpotlightMagnification(ShapeId),
    /// A selected curved arrow's bend grip.
    ArrowBend(ShapeId),
    /// A selected text block's font-size grip.
    TextResize(ShapeId),
    /// One of the eight handles on the selection's bounding box.
    SelectionResize(SelectionHandle),
}

impl InputState {
    /// Resolves a canvas point to the handle a press there would grab.
    ///
    /// Ordered most-specific first. The magnification track and the bend grip
    /// both sit outside the shape's own bounding box, so nothing else claims
    /// their pixels and letting the selection box swallow them would make them
    /// unusable on exactly the shapes that need them most — a shallow arc, a
    /// loupe with the Spotlight tool still active.
    pub(crate) fn hit_idle_handle_with(
        &self,
        measurer: &crate::draw::TextMeasurer,
        x: i32,
        y: i32,
    ) -> Option<IdleHandle> {
        if let Some(control) = self.hit_spotlight_magnification_track_with(measurer, x, y) {
            return Some(IdleHandle::SpotlightMagnification(control.shape_id));
        }
        if let Some(handle) = self.hit_arrow_bend_handle(x, y) {
            return Some(IdleHandle::ArrowBend(handle.shape_id));
        }
        if let Some(shape_id) = self.hit_text_resize_handle_with(measurer, x, y) {
            return Some(IdleHandle::TextResize(shape_id));
        }
        self.hit_selection_handle_with(measurer, x, y)
            .map(IdleHandle::SelectionResize)
    }

    /// Snapshot of one shape on the active frame, for a gesture about to
    /// change it. `None` when the id no longer resolves.
    pub(crate) fn shape_snapshot(&self, shape_id: ShapeId) -> Option<ShapeSnapshot> {
        self.boards
            .active_frame()
            .shape(shape_id)
            .map(|shape| ShapeSnapshot {
                shape: shape.shape.clone(),
                locked: shape.locked,
            })
    }
}

#[cfg(test)]
mod tests;
