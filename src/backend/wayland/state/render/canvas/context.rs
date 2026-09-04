use std::time::Instant;

use super::super::plan::{CanvasFrame, FrameGeometry};
use crate::util::Rect;

/// Parameters borrowed from the owned frame plan and its local Cairo target.
/// Backdrop replay resources and mutable runtime caches stay in the painter.
pub(in crate::backend::wayland::state::render) struct CanvasRenderCtx<'a> {
    pub(in crate::backend::wayland::state::render) cairo: &'a cairo::Context,
    pub(in crate::backend::wayland::state::render) geometry: &'a FrameGeometry,
    pub(in crate::backend::wayland::state::render) canvas: &'a CanvasFrame,
    pub(in crate::backend::wayland::state::render) damage_world: &'a [Rect],
    pub(in crate::backend::wayland::state::render) now: Instant,
}
