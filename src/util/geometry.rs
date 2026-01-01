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
        let width = max_x - min_x;
        let height = max_y - min_y;
        Self::new(min_x, min_y, width, height)
    }

    /// Returns true if rectangle has a positive area.
    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0
    }

    /// Returns true if the point lies within the rectangle (inclusive of min, exclusive of max).
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    /// Returns a new rectangle inflated by `amount` in all directions.
    pub fn inflated(&self, amount: i32) -> Option<Self> {
        if amount == 0 {
            return Some(*self);
        }
        let new_x = self.x - amount;
        let new_y = self.y - amount;
        let new_width = self.width + amount * 2;
        let new_height = self.height + amount * 2;
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
