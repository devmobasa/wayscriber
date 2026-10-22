use std::time::Instant;

use super::super::plan::{CanvasFrame, FrameGeometry};
use crate::util::Rect;

/// Parameters borrowed from the owned frame plan and its local Cairo target.
/// Backdrop replay resources and mutable runtime caches stay in the painter.
pub(in crate::backend::wayland::state::render) struct CanvasRenderCtx<'frame> {
    pub(in crate::backend::wayland::state::render) cairo: &'frame cairo::Context,
    pub(in crate::backend::wayland::state::render) geometry: &'frame FrameGeometry,
    pub(in crate::backend::wayland::state::render) canvas: &'frame CanvasFrame,
    pub(in crate::backend::wayland::state::render) damage_world: &'frame [Rect],
    pub(in crate::backend::wayland::state::render) now: Instant,
}
