//! Zoom math helpers (coordinate transforms and clamps).
//!
//! These functions stay UI/backend-agnostic so both rendering and input
//! handling can share the same clamping and mapping logic.

/// Axis-aligned rectangle in floating-point logical space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectF {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl RectF {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Option<Self> {
        if width <= 0.0 || height <= 0.0 {
            None
        } else {
            Some(Self {
                x,
                y,
                width,
                height,
            })
        }
    }
}

/// Clamp a zoom factor to the permitted range.
pub fn clamp_factor(factor: f32, min: f32, max: f32) -> f32 {
    factor.clamp(min.max(0.1), max.max(min.max(0.1)))
}

/// Compute the logical crop rect for a zoom factor and center, clamped to the viewport.
pub fn crop_rect_logical(
    center_logical: (f64, f64),
    factor: f32,
    viewport_logical: (f64, f64),
    min_factor: f32,
    max_factor: f32,
) -> RectF {
    let clamped = clamp_factor(factor, min_factor, max_factor) as f64;
    let view_w = viewport_logical.0.max(1.0);
    let view_h = viewport_logical.1.max(1.0);
    let crop_w = (view_w / clamped).max(1.0);
    let crop_h = (view_h / clamped).max(1.0);
    let mut x = center_logical.0 - crop_w * 0.5;
    let mut y = center_logical.1 - crop_h * 0.5;

    let max_x = (view_w - crop_w).max(0.0);
    let max_y = (view_h - crop_h).max(0.0);
    if x < 0.0 {
        x = 0.0;
    } else if x > max_x {
        x = max_x;
    }
    if y < 0.0 {
        y = 0.0;
    } else if y > max_y {
        y = max_y;
    }

    RectF {
        x,
        y,
        width: crop_w,
        height: crop_h,
    }
}

/// Map a logical-space rect into frame pixels for a given output origin and scale (rotation 0 only).
pub fn logical_rect_to_frame_px(
    rect: RectF,
    output_origin_logical: (f64, f64),
    scale: f64,
) -> RectF {
    let ox = output_origin_logical.0 * scale;
    let oy = output_origin_logical.1 * scale;
    RectF {
        x: (rect.x * scale) + ox,
        y: (rect.y * scale) + oy,
        width: rect.width * scale,
        height: rect.height * scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_factor_respects_min_max() {
        assert_eq!(clamp_factor(0.05, 1.0, 4.0), 1.0);
        assert_eq!(clamp_factor(10.0, 1.0, 4.0), 4.0);
        assert!((clamp_factor(2.0, 1.0, 4.0) - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn crop_rect_clamps_to_edges() {
        // Viewport 1920x1080, factor 2.0 -> crop 960x540
        let rect = crop_rect_logical((100.0, 100.0), 2.0, (1920.0, 1080.0), 1.0, 4.0);
        assert_eq!(
            rect,
            RectF {
                x: 0.0,
                y: 0.0,
                width: 960.0,
                height: 540.0
            }
        );

        // Center near bottom-right clamps to max extents.
        let rect = crop_rect_logical((1910.0, 1070.0), 2.0, (1920.0, 1080.0), 1.0, 4.0);
        assert_eq!(
            rect,
            RectF {
                x: 960.0,
                y: 540.0,
                width: 960.0,
                height: 540.0
            }
        );
    }

    #[test]
    fn logical_to_frame_applies_origin_and_scale() {
        let rect = RectF {
            x: 100.0,
            y: 50.0,
            width: 200.0,
            height: 100.0,
        };
        let mapped = logical_rect_to_frame_px(rect, (1920.0, 0.0), 1.25);
        assert_eq!(
            mapped,
            RectF {
                x: (100.0 * 1.25) + (1920.0 * 1.25),
                y: 50.0 * 1.25,
                width: 200.0 * 1.25,
                height: 100.0 * 1.25,
            }
        );
    }
}
