//! Dirty region tracking for incremental rendering.
//!
//! Collects axis-aligned rectangles that need repainting between frames.

use super::Shape;
use crate::util::Rect;

/// Tracks dirty rectangles accumulated between renders.
#[derive(Debug, Default)]
pub struct DirtyTracker {
    regions: Vec<Rect>,
    force_full: bool,
}

impl DirtyTracker {
    /// Creates a new, empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks the entire surface as dirty. Clears any accumulated rectangles.
    pub fn mark_full(&mut self) {
        self.force_full = true;
        self.regions.clear();
    }

    /// Adds a dirty rectangle if the tracker is not already full.
    pub fn mark_rect(&mut self, rect: Rect) {
        if !rect.is_valid() || self.force_full {
            return;
        }
        self.regions.push(rect);
    }

    /// Adds a dirty rectangle when present.
    pub fn mark_optional_rect(&mut self, rect: Option<Rect>) {
        if let Some(rect) = rect {
            self.mark_rect(rect);
        }
    }

    /// Adds the bounding box for the given shape, or full damage if none is available.
    pub fn mark_shape(&mut self, shape: &Shape) {
        match shape.bounding_box() {
            Some(rect) => self.mark_rect(rect),
            None => self.mark_full(),
        }
    }

    /// Drains the dirty regions gathered so far.
    ///
    /// When the full surface is marked, returns a single rectangle covering the
    /// entire surface; otherwise returns accumulated rectangles.
    pub fn take_regions(&mut self, width: i32, height: i32) -> Vec<Rect> {
        if self.force_full {
            self.force_full = false;
            self.regions.clear();
            if width > 0 && height > 0 {
                if let Some(full) = Rect::new(0, 0, width, height) {
                    return vec![full];
                }
            }
            Vec::new()
        } else {
            self.regions.drain(..).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{Color, Shape};

    #[test]
    fn mark_shape_records_rectangles() {
        let mut tracker = DirtyTracker::new();
        tracker.mark_shape(&Shape::Line {
            x1: 0,
            y1: 0,
            x2: 10,
            y2: 10,
            color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            thick: 2.0,
        });

        let rects = tracker.take_regions(100, 100);
        assert_eq!(rects.len(), 1);
        assert!(rects[0].width > 0);
        assert!(rects[0].height > 0);
    }

    #[test]
    fn mark_full_takes_precedence() {
        let mut tracker = DirtyTracker::new();
        tracker.mark_shape(&Shape::Rect {
            x: 5,
            y: 5,
            w: 10,
            h: 10,
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 2.0,
        });
        tracker.mark_full();
        tracker.mark_shape(&Shape::Rect {
            x: 20,
            y: 20,
            w: 15,
            h: 15,
            color: Color {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 2.0,
        });

        let rects = tracker.take_regions(200, 100);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0], Rect::new(0, 0, 200, 100).unwrap());
    }
}
