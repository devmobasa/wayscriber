//! Anchored popover placement.
//!
//! A popover is a small panel anchored to a widget (the shape grid under
//! its split button, the overflow menu under », per-tool options under the
//! active tool). Placement is pure geometry so every consumer agrees on the
//! rules: prefer opening below the anchor, flip above when the bottom would
//! overflow, clamp horizontally into the surface, and point a caret at the
//! anchor's center.
//!
//! On layer-shell the surface grows to make room and the input region
//! shrinks to bar + popover so the transparent remainder stays
//! click-through (`ToolbarSurface::set_input_rects`); inline mode simply
//! draws on the overlay.

// Consumed by the top-strip overflow and per-tool options phases; the
// allows are removed as those land.
#![allow(dead_code)]

pub type Rect = (f64, f64, f64, f64);

/// Which side of the anchor the popover opened on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopoverSide {
    Below,
    Above,
}

#[derive(Debug, Clone, Copy)]
pub struct PopoverSpec {
    /// Anchor rect in logical surface coordinates.
    pub anchor: Rect,
    /// Content size the popover wants (w, h).
    pub content: (f64, f64),
    /// Surface bounds the popover must stay inside (w, h).
    pub bounds: (f64, f64),
    /// Gap between the anchor edge and the popover.
    pub gap: f64,
    /// Minimum distance to the surface edges.
    pub margin: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopoverPlacement {
    /// Final popover rect.
    pub rect: Rect,
    pub side: PopoverSide,
    /// X of the caret tip, clamped inside the popover's horizontal span.
    pub caret_x: f64,
}

/// Place a popover relative to its anchor. Pure and total: pathological
/// inputs clamp instead of panicking.
pub fn place_popover(spec: PopoverSpec) -> PopoverPlacement {
    let (ax, ay, aw, ah) = spec.anchor;
    let (cw, ch) = spec.content;
    let (bw, bh) = spec.bounds;

    let x = (ax + aw / 2.0 - cw / 2.0).clamp(spec.margin, (bw - cw - spec.margin).max(spec.margin));

    let below_y = ay + ah + spec.gap;
    let (y, side) = if below_y + ch <= bh - spec.margin {
        (below_y, PopoverSide::Below)
    } else {
        ((ay - spec.gap - ch).max(spec.margin), PopoverSide::Above)
    };

    // The caret points at the anchor center but never leaves the panel's
    // rounded-corner-safe span.
    let caret_inset = 10.0_f64.min(cw / 2.0);
    let caret_x = (ax + aw / 2.0).clamp(x + caret_inset, (x + cw - caret_inset).max(x));

    PopoverPlacement {
        rect: (x, y, cw, ch),
        side,
        caret_x,
    }
}

/// Input rects for a bar with an optional open popover: the transparent
/// grown area between them must not eat clicks meant for the canvas.
pub fn input_rects_with_popover(bar: Rect, popover: Option<Rect>) -> Vec<Rect> {
    match popover {
        Some(popover) => vec![bar, popover],
        None => vec![bar],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(anchor: Rect) -> PopoverSpec {
        PopoverSpec {
            anchor,
            content: (120.0, 80.0),
            bounds: (400.0, 300.0),
            gap: 6.0,
            margin: 8.0,
        }
    }

    #[test]
    fn opens_below_and_centers_on_the_anchor() {
        let placement = place_popover(spec((140.0, 20.0, 40.0, 40.0)));
        assert_eq!(placement.side, PopoverSide::Below);
        assert_eq!(placement.rect, (100.0, 66.0, 120.0, 80.0));
        assert!((placement.caret_x - 160.0).abs() < 1e-9);
    }

    #[test]
    fn clamps_into_the_surface_and_keeps_the_caret_on_the_anchor() {
        // Anchor at the far right edge: the panel clamps, the caret stays
        // as close to the anchor center as the panel allows.
        let placement = place_popover(spec((370.0, 20.0, 24.0, 24.0)));
        let (x, _, w, _) = placement.rect;
        assert_eq!(x, 400.0 - 120.0 - 8.0);
        assert!((placement.caret_x - (x + w - 10.0)).abs() < 1e-9);
    }

    #[test]
    fn flips_above_when_the_bottom_would_overflow() {
        let placement = place_popover(spec((140.0, 250.0, 40.0, 40.0)));
        assert_eq!(placement.side, PopoverSide::Above);
        let (_, y, _, h) = placement.rect;
        assert!((y + h + 6.0 - 250.0).abs() < 1e-9, "sits gap above anchor");
    }

    #[test]
    fn degenerate_bounds_clamp_instead_of_panicking() {
        let placement = place_popover(PopoverSpec {
            anchor: (0.0, 0.0, 10.0, 10.0),
            content: (500.0, 500.0),
            bounds: (100.0, 100.0),
            gap: 4.0,
            margin: 8.0,
        });
        assert_eq!(placement.rect.0, 8.0);
        assert_eq!(placement.rect.1, 8.0);
    }

    #[test]
    fn input_rects_cover_bar_and_open_popover_only() {
        let bar = (0.0, 0.0, 400.0, 58.0);
        assert_eq!(input_rects_with_popover(bar, None), vec![bar]);
        let pop = (40.0, 64.0, 120.0, 80.0);
        assert_eq!(input_rects_with_popover(bar, Some(pop)), vec![bar, pop]);
    }
}
