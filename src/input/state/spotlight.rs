//! Gathering the spotlight regions the compositing pass needs.
//!
//! The pass has to know every region before it paints, because it builds one dim
//! layer and punches all the openings out of it. That makes spotlights the only
//! shape kind the renderer collects up front instead of drawing in z-order.

use crate::draw::{Shape, SpotlightRegion, spotlight_regions_for_frame};
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

impl InputState {
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
                self.spotlight_magnification,
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
