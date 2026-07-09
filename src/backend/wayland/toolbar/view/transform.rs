//! The single coordinate transform between a toolbar surface and its tree.
//!
//! Composition (per axis): `surface = (logical - scroll + offset) * scale`.
//! `offset` is the logical position of the tree origin on the target surface
//! (zero for standalone layer surfaces; the toolbar's placement for inline
//! rendering into the main overlay). `scroll` shifts tree content that lives
//! in a scrolling pane; chrome outside the pane uses a transform with zero
//! scroll. Input handlers map pointer coordinates through
//! [`ViewTransform::surface_to_logical`] *before* hit-testing, and the
//! painter applies the forward transform once — there are deliberately no
//! per-node coordinate fixups anywhere else.

// Consumed starting with the top-strip port; the allow is removed then.
#![allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewTransform {
    /// UI scale factor (toolbar scale × output scale handling).
    pub scale: f64,
    /// Logical position of the tree origin on the surface.
    pub offset: (f64, f64),
    /// Logical scroll displacement of the tree content.
    pub scroll: (f64, f64),
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self::identity()
    }
}

impl ViewTransform {
    pub fn identity() -> Self {
        Self {
            scale: 1.0,
            offset: (0.0, 0.0),
            scroll: (0.0, 0.0),
        }
    }

    pub fn scaled(scale: f64) -> Self {
        Self {
            scale,
            ..Self::identity()
        }
    }

    pub fn with_offset(mut self, offset: (f64, f64)) -> Self {
        self.offset = offset;
        self
    }

    pub fn with_scroll(mut self, scroll: (f64, f64)) -> Self {
        self.scroll = scroll;
        self
    }

    /// Map a logical point to surface coordinates.
    pub fn logical_to_surface(&self, x: f64, y: f64) -> (f64, f64) {
        (
            (x - self.scroll.0 + self.offset.0) * self.scale,
            (y - self.scroll.1 + self.offset.1) * self.scale,
        )
    }

    /// Map a surface point into the tree's logical space (inverse).
    pub fn surface_to_logical(&self, x: f64, y: f64) -> (f64, f64) {
        (
            x / self.scale - self.offset.0 + self.scroll.0,
            y / self.scale - self.offset.1 + self.scroll.1,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(a: (f64, f64), b: (f64, f64)) {
        assert!(
            (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9,
            "{a:?} != {b:?}"
        );
    }

    #[test]
    fn identity_is_a_no_op() {
        let t = ViewTransform::identity();
        assert_close(t.logical_to_surface(12.5, -3.0), (12.5, -3.0));
        assert_close(t.surface_to_logical(12.5, -3.0), (12.5, -3.0));
    }

    #[test]
    fn scale_offset_and_scroll_round_trip() {
        let t = ViewTransform::scaled(1.25)
            .with_offset((16.0, 12.0))
            .with_scroll((0.0, 48.0));

        let surface = t.logical_to_surface(100.0, 200.0);
        assert_close(t.surface_to_logical(surface.0, surface.1), (100.0, 200.0));

        // Scrolled-down content maps to a higher point on the surface.
        let (_, unscrolled_y) = ViewTransform::scaled(1.25)
            .with_offset((16.0, 12.0))
            .logical_to_surface(100.0, 200.0);
        assert!(surface.1 < unscrolled_y);
    }

    #[test]
    fn inverse_matches_manual_composition() {
        let t = ViewTransform {
            scale: 2.0,
            offset: (10.0, 20.0),
            scroll: (5.0, 0.0),
        };
        // surface = (logical - scroll + offset) * scale
        assert_close(t.logical_to_surface(30.0, 40.0), (70.0, 120.0));
        assert_close(t.surface_to_logical(70.0, 120.0), (30.0, 40.0));
    }
}
