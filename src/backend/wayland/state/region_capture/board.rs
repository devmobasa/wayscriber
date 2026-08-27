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
/// One exception: when both rounded edges of a sub-pixel crop coincide, the
/// image would have no extent, so it is given the single board pixel centred on
/// it. Each edge then stays strictly within one board pixel of its exact
/// position. Pushing the far edge out from an already-rounded near edge instead
/// would allow 1.5. Being under a board pixel wide is necessary but not
/// sufficient — position decides too, and `[0, ⅔]` rounds normally to `[0, 1]`.
///
/// This is not confined to fractional scales: at any output scale above 1x a
/// crop a few source pixels wide is under a board pixel across.
pub(in crate::backend::wayland) fn board_bounds_for_world_rect(
    exact: CanvasExportRect,
) -> Option<Rect> {
    let (left, right) = board_axis(exact.x, exact.width)?;
    let (top, bottom) = board_axis(exact.y, exact.height)?;
    Rect::from_min_max(left, top, right, bottom)
}

/// One axis of the placement: the rounded edge pair, widened to the centred
/// board pixel when rounding collapses them.
fn board_axis(origin: f64, extent: f64) -> Option<(i32, i32)> {
    let near = round_edge(origin)?;
    let far = round_edge(origin + extent)?;
    if far > near {
        return Some((near, far));
    }
    // The unit cell centred on the crop. Its start is clamped into the range a
    // pair of edges can express: a crop straddling `i32::MAX` still has
    // `[MAX - 1, MAX]` available, and one at `i32::MIN` has `[MIN, MIN + 1]`,
    // so neither should fail for want of a representable neighbour.
    let centre_start = origin + extent / 2.0 - 0.5;
    if !centre_start.is_finite() {
        return None;
    }
    let start = centre_start
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX - 1)) as i32;
    let end = start + 1;
    // Clamping only bites within a pixel of the representable edges, but a crop
    // pushed past them cannot keep the bound this function promises. Refuse
    // rather than place it somewhere it does not belong.
    if (f64::from(start) - origin).abs() >= 1.0 || (f64::from(end) - (origin + extent)).abs() >= 1.0
    {
        return None;
    }
    Some((start, end))
}

fn round_edge(value: f64) -> Option<i32> {
    if !value.is_finite() {
        return None;
    }
    let rounded = value.round();
    if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        return None;
    }
    Some(rounded as i32)
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

/// Scale a source world rectangle to the composed native output size, keeping
/// the original top-left. Width and height use output/source pixel ratios.
pub(in crate::backend::wayland) fn world_rect_for_composed_region(
    source_world: CanvasExportRect,
    source_size: (u32, u32),
    output_size: (u32, u32),
) -> Option<CanvasExportRect> {
    if source_size.0 == 0
        || source_size.1 == 0
        || output_size.0 == 0
        || output_size.1 == 0
        || !source_world.x.is_finite()
        || !source_world.y.is_finite()
        || !source_world.width.is_finite()
        || !source_world.height.is_finite()
        || source_world.width <= 0.0
        || source_world.height <= 0.0
    {
        return None;
    }
    let width = source_world.width * f64::from(output_size.0) / f64::from(source_size.0);
    let height = source_world.height * f64::from(output_size.1) / f64::from(source_size.1);
    CanvasExportRect::new(source_world.x, source_world.y, width, height)
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
        // of a crop this thin round onto the same integer, so it takes the
        // whole board pixel centred on it — here landing 0.7 past its far edge,
        // still inside the one-pixel bound.
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

    /// The case that motivated centring: rounding the near edge first and then
    /// pushing the far edge out puts `[10.5, 10.75]` at `[11, 12]`, 1.25 board
    /// pixels past its far edge. The centred pixel keeps both edges under one.
    #[test]
    fn a_collapsed_axis_is_centred_rather_than_pushed_off_its_near_edge() {
        let placed =
            board_bounds_for_world_rect(CanvasExportRect::new(10.5, 10.5, 0.25, 0.25).unwrap())
                .expect("a quarter-pixel crop places");
        assert_eq!(
            (placed.x, placed.y, placed.width, placed.height),
            (10, 10, 1, 1)
        );
        assert!(
            (f64::from(placed.x + placed.width) - 10.75).abs() <= 0.5,
            "the far edge stays close, not 1.25 out"
        );
    }

    /// A crop under a board pixel across is not a fractional-scale curiosity:
    /// two source pixels at an integer 2x output scale is one board pixel, so a
    /// one-pixel crop there already collapses.
    #[test]
    fn an_integer_two_times_scale_also_reaches_the_collapsed_case() {
        let mut token = source(false);
        token.image_size = (1600, 1200);
        token.surface = (800, 600);
        token.output_scale = 2;
        let one_pixel = ImagePixelRect::new(41, 41, 1, 1, token.image_size).unwrap();

        let exact = world_rect_for_image_rect_exact(one_pixel, (0.0, 0.0), token).unwrap();
        assert_eq!(
            (exact.x, exact.width),
            (20.5, 0.5),
            "half a board pixel wide"
        );
        let placed = board_bounds_for_world_rect(exact).expect("it still places");
        assert_eq!((placed.width, placed.height), (1, 1));
        assert!(
            edge_drifts(exact, placed)
                .into_iter()
                .all(|drift| drift < 1.0)
        );
    }

    /// The bound the documentation promises, swept rather than sampled: every
    /// edge lands strictly within one board pixel, and within half of one
    /// whenever rounding did not collapse the axis.
    #[test]
    fn every_placement_stays_within_a_board_pixel_of_the_exact_crop() {
        let steps = 0..40;
        for origin_step in steps.clone() {
            for extent_step in 1..40 {
                let x = -3.0 + f64::from(origin_step) * 0.17;
                let width = f64::from(extent_step) * 0.13;
                let exact = CanvasExportRect::new(x, x, width, width).unwrap();
                let placed = board_bounds_for_world_rect(exact)
                    .unwrap_or_else(|| panic!("{x} + {width} must place"));
                // Collapse is decided by the rounded edges, not by the placed
                // width: an ordinary interval can round to width one too, and
                // counting it as collapsed would excuse it from the tighter
                // bound it actually keeps.
                let collapsed = (x + width).round() <= x.round();
                for drift in edge_drifts(exact, placed) {
                    if collapsed {
                        assert!(
                            drift < 1.0,
                            "x={x} width={width} placed={placed:?} drifted {drift}, \
                             outside the collapsed bound"
                        );
                    } else {
                        assert!(
                            drift <= 0.5 + f64::EPSILON,
                            "x={x} width={width} placed={placed:?} drifted {drift}, \
                             outside the ordinary bound"
                        );
                    }
                }
            }
        }
    }

    /// Distance from each placed edge to the exact edge it represents.
    fn edge_drifts(exact: CanvasExportRect, placed: Rect) -> [f64; 4] {
        [
            (f64::from(placed.x) - exact.x).abs(),
            (f64::from(placed.y) - exact.y).abs(),
            (f64::from(placed.x + placed.width) - (exact.x + exact.width)).abs(),
            (f64::from(placed.y + placed.height) - (exact.y + exact.height)).abs(),
        ]
    }

    /// A crop straddling the far end of the representable range still has
    /// `[MAX - 1, MAX]` to sit in, and one at the near end has `[MIN, MIN + 1]`.
    /// Neither may fail for want of a neighbour on the side it collapsed toward.
    #[test]
    fn a_collapsed_axis_at_the_integer_limits_still_places() {
        let max = f64::from(i32::MAX);
        let placed =
            board_bounds_for_world_rect(CanvasExportRect::new(max - 0.4, 0.0, 0.8, 1.0).unwrap())
                .expect("a crop straddling i32::MAX places");
        assert_eq!((placed.x, placed.width), (i32::MAX - 1, 1));
        assert!(
            edge_drifts_x(max - 0.4, 0.8, placed)
                .into_iter()
                .all(|d| d < 1.0)
        );

        let min = f64::from(i32::MIN);
        let placed =
            board_bounds_for_world_rect(CanvasExportRect::new(min - 0.4, 0.0, 0.8, 1.0).unwrap())
                .expect("a crop straddling i32::MIN places");
        assert_eq!((placed.x, placed.width), (i32::MIN, 1));
        assert!(
            edge_drifts_x(min - 0.4, 0.8, placed)
                .into_iter()
                .all(|d| d < 1.0)
        );
    }

    /// Clamping must not become a licence to place a crop that has run off the
    /// representable range entirely: that keeps no bound at all.
    #[test]
    fn a_collapsed_axis_pushed_past_the_limits_is_refused() {
        let max = f64::from(i32::MAX);
        assert_eq!(
            board_bounds_for_world_rect(CanvasExportRect::new(max + 0.3, 0.0, 0.1, 1.0).unwrap()),
            None,
            "a cell clamped back inside would sit more than a pixel from the crop"
        );
    }

    /// Horizontal drifts only, for the cases whose vertical axis is an ordinary
    /// one-pixel interval.
    fn edge_drifts_x(origin: f64, extent: f64, placed: Rect) -> [f64; 2] {
        [
            (f64::from(placed.x) - origin).abs(),
            (f64::from(placed.x + placed.width) - (origin + extent)).abs(),
        ]
    }
}
