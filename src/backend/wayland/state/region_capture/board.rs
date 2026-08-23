use crate::util::Rect;
use crate::{canvas_export::CanvasExportRect, screen_pixels::ImagePixelRect};

use super::super::screen_image::ScreenSourceToken;

/// Round an exact world rectangle to the integer bounds `Shape::Image` needs.
///
/// Each edge rounds to nearest, so the pasted image normally lands within half
/// a board pixel of where composition drew it. Going through the picker's
/// display rectangle instead would compound two outward quantizations and could
/// only ever stretch the crop, never shrink it.
///
/// One exception: when both edges of an axis round to the same integer the
/// image would have no extent, so the far edge is pushed a whole pixel out to
/// give it one. That edge can then sit up to one board pixel from its exact
/// position — 0.7 for a crop spanning 10.1 to 10.3, for instance. It is
/// unavoidable while `Shape::Image` bounds are integers, and only reachable for
/// a crop of one or two source pixels at a fractional scale.
pub(in crate::backend::wayland) fn board_bounds_for_world_rect(
    exact: CanvasExportRect,
) -> Option<Rect> {
    let round_edge = |value: f64| {
        if !value.is_finite() {
            return None;
        }
        let rounded = value.round();
        if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
            return None;
        }
        Some(rounded as i32)
    };
    let left = round_edge(exact.x)?;
    let top = round_edge(exact.y)?;
    let right = round_edge(exact.x + exact.width)?;
    let bottom = round_edge(exact.y + exact.height)?;
    // A crop narrower than a board pixel still has to occupy one. This is the
    // documented exception to the half-pixel bound above.
    Rect::from_min_max(
        left,
        top,
        right.max(left.checked_add(1)?),
        bottom.max(top.checked_add(1)?),
    )
}

/// Map authoritative source-image pixel edges directly into board-world space.
///
/// This intentionally bypasses the integer display rectangle used by picker
/// chrome. Its floor/ceil quantization can expand a crop at fractional output
/// scales. Zoom does not appear here: selecting an image-space rectangle has
/// already inverted the captured zoom view, so only the board offset and the
/// source image-to-logical-surface ratio remain.
pub(in crate::backend::wayland) fn world_rect_for_image_rect_exact(
    rect: ImagePixelRect,
    board_offset: (f64, f64),
    source: ScreenSourceToken,
) -> Option<CanvasExportRect> {
    if !board_offset.0.is_finite()
        || !board_offset.1.is_finite()
        || source.image_size.0 == 0
        || source.image_size.1 == 0
        || source.surface.0 == 0
        || source.surface.1 == 0
        || rect.x().checked_add(rect.width())? > source.image_size.0
        || rect.y().checked_add(rect.height())? > source.image_size.1
    {
        return None;
    }
    let scale_x = f64::from(source.surface.0) / f64::from(source.image_size.0);
    let scale_y = f64::from(source.surface.1) / f64::from(source.image_size.1);
    CanvasExportRect::new(
        board_offset.0 + f64::from(rect.x()) * scale_x,
        board_offset.1 + f64::from(rect.y()) * scale_y,
        f64::from(rect.width()) * scale_x,
        f64::from(rect.height()) * scale_y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::state::screen_image::ScreenImageKind;

    fn source(zoom_transformed: bool) -> ScreenSourceToken {
        ScreenSourceToken {
            output_id: 1,
            output_layout_generation: 2,
            kind: ScreenImageKind::Frozen,
            image_generation: 3,
            image_size: (800, 600),
            stride: 3200,
            surface: (800, 600),
            output_scale: 1,
            output_transform: wayland_client::protocol::wl_output::Transform::Normal,
            zoom_transformed,
            zoom_scale: if zoom_transformed { 2.0 } else { 1.0 },
            zoom_view_offset: if zoom_transformed {
                (100.0, 50.0)
            } else {
                (0.0, 0.0)
            },
        }
    }

    #[test]
    fn exact_image_mapping_preserves_native_and_fractional_scale_edges() {
        let rect = ImagePixelRect::new(150, 225, 300, 150, (1200, 900)).unwrap();
        let mut token = source(false);
        token.image_size = (1200, 900);
        token.surface = (800, 600);
        token.output_scale = 2;
        assert_eq!(
            world_rect_for_image_rect_exact(rect, (7.0, 11.0), token),
            CanvasExportRect::new(107.0, 161.0, 200.0, 100.0)
        );

        token.image_size = (1600, 1200);
        let rect = ImagePixelRect::new(200, 300, 400, 200, token.image_size).unwrap();
        assert_eq!(
            world_rect_for_image_rect_exact(rect, (7.0, 11.0), token),
            CanvasExportRect::new(107.0, 161.0, 200.0, 100.0)
        );
    }

    #[test]
    fn exact_image_mapping_is_invariant_to_the_captured_zoom_view() {
        let rect = ImagePixelRect::new(20, 30, 80, 40, (800, 600)).unwrap();
        let plain = source(false);
        let zoomed = source(true);
        assert_eq!(
            world_rect_for_image_rect_exact(rect, (7.0, 11.0), plain),
            world_rect_for_image_rect_exact(rect, (7.0, 11.0), zoomed)
        );
        assert_eq!(
            world_rect_for_image_rect_exact(rect, (7.0, 11.0), zoomed),
            CanvasExportRect::new(27.0, 41.0, 80.0, 40.0)
        );
    }

    #[test]
    fn board_bounds_round_each_edge_to_the_nearest_board_pixel() {
        // Composition draws the crop over the exact world rectangle; placement
        // has to land on integers, so each edge takes the nearest one and the
        // pasted image stays within half a board pixel of the annotations.
        assert_eq!(
            board_bounds_for_world_rect(CanvasExportRect::new(10.4, 20.6, 30.4, 40.6).unwrap()),
            Rect::new(10, 21, 31, 40),
            "left 10.4 -> 10, right 40.8 -> 41, top 20.6 -> 21, bottom 61.2 -> 61"
        );
        // Negative origins round half away from zero, matching `f64::round`.
        assert_eq!(
            board_bounds_for_world_rect(CanvasExportRect::new(-3.5, -0.4, 2.0, 1.0).unwrap()),
            Rect::new(-4, 0, 2, 1)
        );
    }

    #[test]
    fn a_sub_pixel_crop_takes_a_whole_board_pixel_and_says_so() {
        let bounds =
            board_bounds_for_world_rect(CanvasExportRect::new(10.1, 20.1, 0.2, 0.2).unwrap())
                .expect("a rectangle narrower than a board pixel still places");
        assert_eq!(
            (bounds.x, bounds.y, bounds.width, bounds.height),
            (10, 20, 1, 1)
        );

        // This is the documented exception to the half-pixel bound. Both edges
        // of a crop this thin round to the same integer, so the far edge is
        // pushed a whole pixel out to give the image an extent at all, landing
        // 0.7 board pixels from where composition drew it.
        let far_edge_drift = f64::from(bounds.x + bounds.width) - (10.1 + 0.2);
        assert!(
            (far_edge_drift - 0.7).abs() < 1e-9,
            "expected the far edge 0.7 out, got {far_edge_drift}"
        );

        // A crop whose edges do land in different buckets keeps the bound.
        let ordinary = board_bounds_for_world_rect(
            CanvasExportRect::new(0.0, 0.0, 2.0 / 3.0, 2.0 / 3.0).unwrap(),
        )
        .expect("a two-thirds-pixel crop places");
        assert_eq!((ordinary.width, ordinary.height), (1, 1));
        let ordinary_drift = f64::from(ordinary.x + ordinary.width) - 2.0 / 3.0;
        assert!(
            ordinary_drift <= 0.5,
            "no minimum was forced here, so the bound holds: {ordinary_drift}"
        );
    }

    #[test]
    fn board_placement_tracks_the_exact_crop_at_fractional_scale_and_under_zoom() {
        // A 2x output: the display rectangle the picker paints is the image
        // rectangle rounded outward, so placing from it would stretch the crop.
        // Placing from the exact mapping does not.
        let mut token = source(false);
        token.image_size = (1600, 1200);
        token.surface = (800, 600);
        token.output_scale = 2;
        let rect = ImagePixelRect::new(21, 17, 43, 31, token.image_size).unwrap();

        let exact = world_rect_for_image_rect_exact(rect, (0.0, 0.0), token).unwrap();
        assert_eq!(
            (exact.x, exact.y, exact.width, exact.height),
            (10.5, 8.5, 21.5, 15.5)
        );
        let placed = board_bounds_for_world_rect(exact).unwrap();
        assert_eq!(
            (placed.x, placed.y, placed.width, placed.height),
            (11, 9, 21, 15),
            "each edge rounds once, from the true world position"
        );
        for (edge, exact_edge) in [
            (f64::from(placed.x), exact.x),
            (f64::from(placed.y), exact.y),
            (f64::from(placed.x + placed.width), exact.x + exact.width),
            (f64::from(placed.y + placed.height), exact.y + exact.height),
        ] {
            assert!(
                (edge - exact_edge).abs() <= 0.5,
                "{edge} drifted more than half a board pixel from {exact_edge}"
            );
        }

        // Zoom is already inverted by the image-space selection, so a zoomed
        // token places identically to a plain one.
        let mut zoomed = token;
        zoomed.zoom_transformed = true;
        zoomed.zoom_scale = 2.0;
        zoomed.zoom_view_offset = (100.0, 50.0);
        assert_eq!(
            world_rect_for_image_rect_exact(rect, (0.0, 0.0), zoomed)
                .and_then(board_bounds_for_world_rect),
            Some(placed)
        );
    }
}
