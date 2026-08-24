//! Gathering the spotlight regions the compositing pass needs.
//!
//! The pass has to know every region before it paints, because it builds one dim
//! layer and punches all the openings out of it. That makes spotlights the only
//! shape kind the renderer collects up front instead of drawing in z-order.

use crate::draw::{Shape, SpotlightRegion, spotlight_regions_for_frame};
use crate::input::Tool;

use super::{DrawingState, InputState};

impl InputState {
    /// Every committed spotlight on the active page, plus the one being dragged.
    ///
    /// Including the in-progress drag is what makes the tool usable: the dimming
    /// follows the drag instead of appearing only once the button is released.
    pub(crate) fn spotlight_regions(&self, cursor: (i32, i32)) -> Vec<SpotlightRegion> {
        let mut regions = spotlight_regions_for_frame(self.boards.active_frame());

        regions.extend(self.provisional_spotlight_region(cursor));
        regions
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
