/// Clamps a value to a specified range.
///
/// Kept for future use (e.g., dirty region optimization, bounds checking).
#[allow(dead_code)]
pub fn clamp(val: i32, min: i32, max: i32) -> i32 {
    if val < min {
        min
    } else if val > max {
        max
    } else {
        val
    }
}

/// Axis-aligned rectangle helper used for dirty region tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    /// Creates a new rectangle. Width/height must be non-negative.
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Option<Self> {
        if width <= 0 || height <= 0 {
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

    /// Builds a rectangle from min/max bounds (inclusive min, exclusive max).
    pub fn from_min_max(min_x: i32, min_y: i32, max_x: i32, max_y: i32) -> Option<Self> {
        let width = i32::try_from(i64::from(max_x) - i64::from(min_x)).ok()?;
        let height = i32::try_from(i64::from(max_y) - i64::from(min_y)).ok()?;
        Self::new(min_x, min_y, width, height)
    }

    /// Returns true if rectangle has a positive area.
    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0
    }

    /// Returns true if the point lies within the rectangle (inclusive of min, exclusive of max).
    pub fn contains(&self, x: i32, y: i32) -> bool {
        let x = i64::from(x);
        let y = i64::from(y);
        let min_x = i64::from(self.x);
        let min_y = i64::from(self.y);
        let max_x = min_x + i64::from(self.width);
        let max_y = min_y + i64::from(self.height);

        self.is_valid() && x >= min_x && x < max_x && y >= min_y && y < max_y
    }

    /// Returns a new rectangle inflated by `amount` in all directions.
    pub fn inflated(&self, amount: i32) -> Option<Self> {
        if !self.is_valid() {
            return None;
        }
        if amount == 0 {
            return Some(*self);
        }
        let amount = i64::from(amount);
        let new_x = i32::try_from(i64::from(self.x) - amount).ok()?;
        let new_y = i32::try_from(i64::from(self.y) - amount).ok()?;
        let new_width = i32::try_from(i64::from(self.width) + amount * 2).ok()?;
        let new_height = i32::try_from(i64::from(self.height) + amount * 2).ok()?;
        Rect::new(new_x, new_y, new_width, new_height)
    }
}

/// Calculates ellipse parameters from two corner points.
///
/// Converts a drag rectangle (from corner to corner) into ellipse parameters
/// (center point and radii) suitable for Cairo's ellipse rendering.
///
/// # Arguments
/// * `x1` - First corner X coordinate
/// * `y1` - First corner Y coordinate
/// * `x2` - Opposite corner X coordinate
/// * `y2` - Opposite corner Y coordinate
///
/// # Returns
/// Tuple `(cx, cy, rx, ry)` where:
/// - `cx`, `cy` = center point coordinates
/// - `rx` = horizontal radius (half width)
/// - `ry` = vertical radius (half height)
pub fn ellipse_bounds(x1: i32, y1: i32, x2: i32, y2: i32) -> (i32, i32, i32, i32) {
    let cx = (x1 + x2) / 2;
    let cy = (y1 + y2) / 2;
    let rx = ((x2 - x1).abs()) / 2;
    let ry = ((y2 - y1).abs()) / 2;
    (cx, cy, rx, ry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_new_rejects_non_positive_dimensions() {
        assert_eq!(Rect::new(0, 0, 0, 4), None);
        assert_eq!(Rect::new(0, 0, 4, -1), None);
    }

    #[test]
    fn rect_contains_uses_inclusive_min_and_exclusive_max_edges() {
        let rect = Rect::new(10, 20, 5, 4).unwrap();

        assert!(rect.contains(10, 20));
        assert!(rect.contains(14, 23));
        assert!(!rect.contains(15, 23));
        assert!(!rect.contains(14, 24));
    }

    #[test]
    fn rect_inflated_expands_evenly_in_all_directions() {
        let rect = Rect::new(10, 20, 5, 4).unwrap();

        assert_eq!(rect.inflated(2), Rect::new(8, 18, 9, 8));
    }

    #[test]
    fn rect_inflated_returns_none_when_negative_amount_eliminates_area() {
        let rect = Rect::new(10, 20, 5, 4).unwrap();

        assert_eq!(rect.inflated(-3), None);
    }

    #[test]
    fn rect_from_min_max_rejects_unrepresentable_dimensions() {
        assert_eq!(
            Rect::from_min_max(i32::MIN, 0, i32::MAX, 1),
            None,
            "the full i32 coordinate span cannot fit in an i32 width"
        );
        assert_eq!(Rect::from_min_max(0, i32::MIN, 1, i32::MAX), None);
    }

    #[test]
    fn rect_contains_handles_endpoints_beyond_i32_range() {
        let near_max = Rect::new(i32::MAX - 1, i32::MAX - 1, 2, 2).unwrap();
        assert!(near_max.contains(i32::MAX, i32::MAX));

        let near_min = Rect::new(i32::MIN, i32::MIN, 2, 2).unwrap();
        assert!(near_min.contains(i32::MIN + 1, i32::MIN + 1));
    }

    #[test]
    fn rect_inflated_rejects_coordinate_and_dimension_overflow() {
        let at_min = Rect::new(i32::MIN, 0, 1, 1).unwrap();
        assert_eq!(at_min.inflated(1), None);

        let widest = Rect::new(0, 0, i32::MAX, 1).unwrap();
        assert_eq!(widest.inflated(1), None);

        let ordinary = Rect::new(0, 0, 10, 10).unwrap();
        assert_eq!(ordinary.inflated(i32::MIN), None);

        let invalid = Rect {
            x: 0,
            y: 0,
            width: -1,
            height: 10,
        };
        assert_eq!(invalid.inflated(0), None);
    }

    #[test]
    fn ellipse_bounds_are_order_independent() {
        assert_eq!(ellipse_bounds(0, 0, 10, 20), ellipse_bounds(10, 20, 0, 0));
    }

    #[test]
    fn ellipse_bounds_compute_center_and_radii_from_drag_corners() {
        assert_eq!(ellipse_bounds(4, 6, 14, 18), (9, 12, 5, 6));
    }
}
