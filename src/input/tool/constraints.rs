//! Modifier-driven constraints applied to an in-flight drag.
//!
//! Every constrained tool builds its shape from a start point and a live end
//! point, so a constraint is expressible as a pure rewrite of that pair. Both
//! the provisional preview and the committed shape route through
//! [`constrain_drag`], which is what keeps the preview honest: what the overlay
//! shows is exactly what a release would commit.

use super::{Tool, ToolDrawingBehavior};

/// Angle increment a snapped line or arrow locks onto.
const SNAP_ANGLE_DEGREES: f64 = 15.0;

/// Constraints requested by the modifiers held during the current drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct DragConstraints {
    /// Lock lines and arrows to fixed angle steps, and boxes to equal sides.
    pub(crate) proportional: bool,
    /// Grow boxes symmetrically about the drag origin instead of corner to corner.
    pub(crate) from_center: bool,
}

impl DragConstraints {
    pub(crate) fn is_active(self) -> bool {
        self.proportional || self.from_center
    }
}

/// Rewrites a drag's endpoints to honor the held constraints.
///
/// Tools whose geometry does not come from a start/end pair — freehand, marker,
/// eraser, step markers — are returned untouched.
pub(crate) fn constrain_drag(
    tool: Tool,
    start: (i32, i32),
    end: (i32, i32),
    constraints: DragConstraints,
) -> ((i32, i32), (i32, i32)) {
    if !constraints.is_active() {
        return (start, end);
    }

    match tool.drawing_behavior() {
        // Direction matters, so snap the angle and leave the length alone.
        // A center-out drag has no meaning for a line's two endpoints.
        ToolDrawingBehavior::Line | ToolDrawingBehavior::Arrow => {
            let end = if constraints.proportional {
                snap_to_angle_steps(start, end)
            } else {
                end
            };
            (start, end)
        }
        // Extent matters, so equalize the sides and optionally mirror the drag
        // so the origin becomes the center rather than a corner.
        ToolDrawingBehavior::Rect
        | ToolDrawingBehavior::Ellipse
        | ToolDrawingBehavior::BlurRect
        | ToolDrawingBehavior::Polygon(_) => {
            let end = if constraints.proportional {
                equalize_extent(start, end)
            } else {
                end
            };
            if constraints.from_center {
                mirror_about_start(start, end)
            } else {
                (start, end)
            }
        }
        ToolDrawingBehavior::None
        | ToolDrawingBehavior::Path { .. }
        | ToolDrawingBehavior::StepMarker
        | ToolDrawingBehavior::Eraser => (start, end),
    }
}

/// Rotates `end` onto the nearest fixed angle step, preserving drag length.
fn snap_to_angle_steps(start: (i32, i32), end: (i32, i32)) -> (i32, i32) {
    let dx = f64::from(end.0 - start.0);
    let dy = f64::from(end.1 - start.1);
    let length = (dx * dx + dy * dy).sqrt();
    if length < 1.0 {
        return end;
    }

    let step = SNAP_ANGLE_DEGREES.to_radians();
    let snapped = (dy.atan2(dx) / step).round() * step;
    (
        start.0 + (length * snapped.cos()).round() as i32,
        start.1 + (length * snapped.sin()).round() as i32,
    )
}

/// Squares the drag extent, following whichever axis the user pulled further.
fn equalize_extent(start: (i32, i32), end: (i32, i32)) -> (i32, i32) {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let extent = dx.abs().max(dy.abs());
    (
        start.0 + if dx < 0 { -extent } else { extent },
        start.1 + if dy < 0 { -extent } else { extent },
    )
}

/// Turns a corner-to-corner drag into one centered on `start`.
fn mirror_about_start(start: (i32, i32), end: (i32, i32)) -> ((i32, i32), (i32, i32)) {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    ((start.0 - dx, start.1 - dy), end)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROPORTIONAL: DragConstraints = DragConstraints {
        proportional: true,
        from_center: false,
    };
    const FROM_CENTER: DragConstraints = DragConstraints {
        proportional: false,
        from_center: true,
    };
    const BOTH: DragConstraints = DragConstraints {
        proportional: true,
        from_center: true,
    };

    #[test]
    fn inactive_constraints_leave_the_drag_untouched() {
        let result = constrain_drag(Tool::Rect, (10, 10), (73, 41), DragConstraints::default());
        assert_eq!(result, ((10, 10), (73, 41)));
    }

    #[test]
    fn line_snaps_to_the_nearest_fifteen_degree_step() {
        // 40 degrees off horizontal snaps down to 45 only if closer; 40 -> 45.
        let (start, end) = constrain_drag(Tool::Line, (0, 0), (100, 84), PROPORTIONAL);
        assert_eq!(start, (0, 0));
        let angle = f64::from(end.1).atan2(f64::from(end.0)).to_degrees();
        assert!(
            (angle - 45.0).abs() < 0.5,
            "expected a 45 degree snap, got {angle}"
        );
    }

    #[test]
    fn line_snap_preserves_drag_length() {
        let (_, end) = constrain_drag(Tool::Line, (50, 50), (150, 60), PROPORTIONAL);
        let length = f64::from(end.0 - 50).hypot(f64::from(end.1 - 50));
        let original = 100.0_f64.hypot(10.0);
        assert!(
            (length - original).abs() < 1.5,
            "snap changed length from {original} to {length}"
        );
    }

    #[test]
    fn line_snap_reaches_exact_horizontal_and_vertical() {
        let (_, end) = constrain_drag(Tool::Line, (0, 0), (100, 6), PROPORTIONAL);
        assert_eq!(end.1, 0, "a near-horizontal drag should snap flat");

        let (_, end) = constrain_drag(Tool::Line, (0, 0), (6, 100), PROPORTIONAL);
        assert_eq!(end.0, 0, "a near-vertical drag should snap upright");
    }

    #[test]
    fn arrow_snaps_like_a_line() {
        let (_, end) = constrain_drag(Tool::Arrow, (0, 0), (100, 6), PROPORTIONAL);
        assert_eq!(end.1, 0);
    }

    #[test]
    fn line_ignores_center_out_dragging() {
        let result = constrain_drag(Tool::Line, (10, 10), (80, 50), FROM_CENTER);
        assert_eq!(result, ((10, 10), (80, 50)));
    }

    #[test]
    fn degenerate_drag_is_left_alone_by_angle_snapping() {
        let result = constrain_drag(Tool::Line, (25, 25), (25, 25), PROPORTIONAL);
        assert_eq!(result, ((25, 25), (25, 25)));
    }

    #[test]
    fn rect_squares_to_the_longer_drag_axis() {
        let (start, end) = constrain_drag(Tool::Rect, (10, 10), (110, 40), PROPORTIONAL);
        assert_eq!(start, (10, 10));
        assert_eq!(end, (110, 110));
    }

    #[test]
    fn rect_squaring_keeps_the_drag_direction() {
        let (_, end) = constrain_drag(Tool::Rect, (100, 100), (40, 70), PROPORTIONAL);
        assert_eq!(end, (40, 40), "up-left drag should stay up-left");
    }

    #[test]
    fn ellipse_squares_into_a_circle() {
        let (start, end) = constrain_drag(Tool::Ellipse, (0, 0), (60, 20), PROPORTIONAL);
        let (cx, cy, rx, ry) = crate::util::ellipse_bounds(start.0, start.1, end.0, end.1);
        assert_eq!((cx, cy), (30, 30));
        assert_eq!(rx, ry, "a squared drag should give equal radii");
    }

    #[test]
    fn from_center_mirrors_the_box_around_the_origin() {
        let (start, end) = constrain_drag(Tool::Rect, (100, 100), (140, 120), FROM_CENTER);
        assert_eq!(start, (60, 80));
        assert_eq!(end, (140, 120));
        // The origin stays the midpoint of the resulting box.
        assert_eq!((start.0 + end.0) / 2, 100);
        assert_eq!((start.1 + end.1) / 2, 100);
    }

    #[test]
    fn both_constraints_give_a_square_centered_on_the_origin() {
        let (start, end) = constrain_drag(Tool::Rect, (100, 100), (140, 120), BOTH);
        assert_eq!((start.0 + end.0) / 2, 100);
        assert_eq!((start.1 + end.1) / 2, 100);
        assert_eq!(end.0 - start.0, end.1 - start.1, "box should be square");
    }

    #[test]
    fn polygon_tools_honor_both_constraints() {
        let (start, end) = constrain_drag(Tool::Triangle, (50, 50), (90, 60), BOTH);
        assert_eq!(end.0 - start.0, end.1 - start.1);
        assert_eq!((start.0 + end.0) / 2, 50);
    }

    #[test]
    fn blur_rect_honors_both_constraints() {
        let (start, end) = constrain_drag(Tool::Blur, (0, 0), (80, 30), BOTH);
        assert_eq!(end.0 - start.0, end.1 - start.1);
    }

    #[test]
    fn freehand_and_marker_drags_are_never_rewritten() {
        for tool in [Tool::Pen, Tool::Marker, Tool::Eraser, Tool::StepMarker] {
            let result = constrain_drag(tool, (10, 10), (90, 33), BOTH);
            assert_eq!(
                result,
                ((10, 10), (90, 33)),
                "{tool:?} geometry does not come from a start/end box"
            );
        }
    }
}
