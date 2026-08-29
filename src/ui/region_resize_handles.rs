use crate::input::SelectionHandle;
use crate::input::state::RegionSelection;
use crate::ui::theme::{self, Rgba};

use super::primitives::draw_rounded_rect;

/// Side length of a corner chip. One step up from the canvas selection handles
/// because the picker paints over a screenshot rather than the board, and the
/// rectangle it decorates is usually much larger than a shape's bounds.
const CORNER_SIZE: f64 = 12.0;
/// Edge-midpoint chips are three quarters of a corner — the same ratio the
/// canvas selection handles use, so the two surfaces read as one language.
const EDGE_SIZE: f64 = CORNER_SIZE * 0.75;
const RADIUS: f64 = 3.0;
/// Extra forgiveness around a chip, for hit testing only. The drawn chip stays
/// small; the grab target is deliberately larger than it looks.
const HIT_TOLERANCE: f64 = 4.0;
/// How far a full-size corner grip reaches from its centre.
const CORNER_REACH: f64 = CORNER_SIZE / 2.0 + HIT_TOLERANCE;
/// Grips may claim at most this fraction of a side, measured from each end, so
/// a central move area always survives. Four corner reaches therefore need a
/// side of `CORNER_REACH / GRIP_SIDE_SHARE` before the grips stop shrinking.
const GRIP_SIDE_SHARE: f64 = 0.25;
/// A side shorter than this cannot seat an edge chip between its two corner
/// chips without them touching, so that side's midpoint handle is dropped.
/// Corners are always offered, so a rectangle is never unresizable.
const MIN_SIDE_FOR_EDGE_HANDLE: f64 = 48.0;

const FILL: Rgba = (1.0, 1.0, 1.0, 0.96);
const FILL_HOVER: Rgba = theme::rgba(theme::ACCENT_BRIGHT_RGB, 1.0);
const BORDER: Rgba = theme::rgba(theme::ACCENT_RGB, 0.95);
const BORDER_WIDTH: f64 = 1.5;
/// A chip narrower than this cannot show a white body behind a ring at all, so
/// it is painted as a solid accent dot instead. Keeps the ring from collapsing
/// into sub-pixel geometry on a rectangle whose grips have scaled right down.
const MIN_BORDERED_SIZE: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct HandleChip {
    handle: SelectionHandle,
    center: (f64, f64),
    size: f64,
    /// Half-extent of the grab target, already scaled with the chip.
    reach: f64,
}

impl HandleChip {
    fn contains(self, point: (f64, f64)) -> bool {
        (point.0 - self.center.0).abs() <= self.reach
            && (point.1 - self.center.1).abs() <= self.reach
    }

    fn squared_distance_to(self, point: (f64, f64)) -> f64 {
        let dx = point.0 - self.center.0;
        let dy = point.1 - self.center.1;
        dx * dx + dy * dy
    }

    const fn rect(self) -> (f64, f64, f64, f64) {
        let half = self.size / 2.0;
        (
            self.center.0 - half,
            self.center.1 - half,
            self.size,
            self.size,
        )
    }
}

/// The eight resize grips on a reviewed rectangle: four corners, always
/// present, plus the four edge midpoints when their side is long enough to
/// seat one. Chips are centred on the edge rather than parked outside it, so a
/// selection flush with the screen edge keeps every grip reachable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RegionResizeHandles {
    /// Corners first: their order breaks a tie when two chips sit on the same
    /// point, matching the priority `selection_handle_probes` uses.
    chips: [Option<HandleChip>; 8],
}

impl RegionResizeHandles {
    pub(crate) fn place(selection: RegionSelection) -> Self {
        let left = selection.start.0.min(selection.end.0);
        let right = selection.start.0.max(selection.end.0);
        let top = selection.start.1.min(selection.end.1);
        let bottom = selection.start.1.max(selection.end.1);
        let mid_x = (left + right) / 2.0;
        let mid_y = (top + bottom) / 2.0;
        let horizontal_room = (right - left) >= MIN_SIDE_FOR_EDGE_HANDLE;
        let vertical_room = (bottom - top) >= MIN_SIDE_FOR_EDGE_HANDLE;
        // On a rectangle small enough that four full-size corner targets would
        // meet in the middle, the grips shrink together rather than swallowing
        // the interior — otherwise a small rectangle could be resized but never
        // dragged. Chip and target scale as one, so what is drawn stays what is
        // grabbable.
        let shortest = (right - left).min(bottom - top).max(0.0);
        let scale = (shortest * GRIP_SIDE_SHARE / CORNER_REACH).min(1.0);
        let corner = |handle, center| {
            Some(HandleChip {
                handle,
                center,
                size: CORNER_SIZE * scale,
                reach: CORNER_REACH * scale,
            })
        };
        let edge = |handle, center, room: bool| {
            room.then_some(HandleChip {
                handle,
                center,
                size: EDGE_SIZE * scale,
                reach: (EDGE_SIZE / 2.0 + HIT_TOLERANCE) * scale,
            })
        };
        Self {
            chips: [
                corner(SelectionHandle::TopLeft, (left, top)),
                corner(SelectionHandle::TopRight, (right, top)),
                corner(SelectionHandle::BottomLeft, (left, bottom)),
                corner(SelectionHandle::BottomRight, (right, bottom)),
                edge(SelectionHandle::Top, (mid_x, top), horizontal_room),
                edge(SelectionHandle::Bottom, (mid_x, bottom), horizontal_room),
                edge(SelectionHandle::Left, (left, mid_y), vertical_room),
                edge(SelectionHandle::Right, (right, mid_y), vertical_room),
            ],
        }
    }

    /// The grip under `point`, or the nearest one where two grab targets
    /// overlap. Nearest rather than first matters on a rectangle small enough
    /// that every chip's target covers the whole thing: taking the first would
    /// leave only the top-left corner reachable, so the rectangle could be
    /// grown up and left but never down and right.
    pub(crate) fn hit(self, point: (f64, f64)) -> Option<SelectionHandle> {
        self.chips
            .into_iter()
            .flatten()
            .filter(|chip| chip.contains(point))
            .min_by(|left, right| {
                left.squared_distance_to(point)
                    .total_cmp(&right.squared_distance_to(point))
            })
            .map(|chip| chip.handle)
    }
}

pub(crate) fn render_region_resize_handles(
    ctx: &cairo::Context,
    handles: &RegionResizeHandles,
    hovered: Option<SelectionHandle>,
) {
    let _ = ctx.save();
    for chip in handles.chips.iter().flatten() {
        let (x, y, width, height) = chip.rect();
        if width <= 0.0 || height <= 0.0 {
            continue;
        }
        let hovered_chip = hovered == Some(chip.handle);
        // Radius and border track the chip, so a scaled-down grip stays a
        // rounded chip rather than becoming accidental geometry.
        let radius = RADIUS.min(chip.size / 4.0);
        if chip.size < MIN_BORDERED_SIZE {
            theme::set_color(ctx, if hovered_chip { FILL_HOVER } else { BORDER });
            draw_rounded_rect(ctx, x, y, width, height, radius);
            let _ = ctx.fill();
            continue;
        }

        theme::set_color(ctx, if hovered_chip { FILL_HOVER } else { FILL });
        draw_rounded_rect(ctx, x, y, width, height, radius);
        let _ = ctx.fill();

        // At most an eighth of the chip per side, so the inset rectangle is
        // always positive.
        let border = BORDER_WIDTH.min(chip.size / 8.0);
        theme::set_color(ctx, BORDER);
        ctx.set_line_width(border);
        draw_rounded_rect(
            ctx,
            x + border / 2.0,
            y + border / 2.0,
            width - border,
            height - border,
            (radius - border / 2.0).max(0.0),
        );
        let _ = ctx.stroke();
    }
    let _ = ctx.restore();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(x: f64, y: f64, width: f64, height: f64) -> RegionSelection {
        RegionSelection {
            start: (x, y),
            end: (x + width, y + height),
        }
    }

    #[test]
    fn a_roomy_rectangle_offers_all_eight_grips_at_its_corners_and_midpoints() {
        let handles = RegionResizeHandles::place(selection(100.0, 50.0, 200.0, 120.0));
        for (point, handle) in [
            ((100.0, 50.0), SelectionHandle::TopLeft),
            ((300.0, 50.0), SelectionHandle::TopRight),
            ((100.0, 170.0), SelectionHandle::BottomLeft),
            ((300.0, 170.0), SelectionHandle::BottomRight),
            ((200.0, 50.0), SelectionHandle::Top),
            ((200.0, 170.0), SelectionHandle::Bottom),
            ((100.0, 110.0), SelectionHandle::Left),
            ((300.0, 110.0), SelectionHandle::Right),
        ] {
            assert_eq!(handles.hit(point), Some(handle), "at {point:?}");
        }
        assert_eq!(
            handles.hit((200.0, 110.0)),
            None,
            "the interior is not a grip"
        );
        assert_eq!(handles.hit((400.0, 400.0)), None, "far outside");
    }

    #[test]
    fn a_reversed_selection_places_the_same_grips() {
        let forward = RegionResizeHandles::place(selection(100.0, 50.0, 200.0, 120.0));
        let backward = RegionResizeHandles::place(RegionSelection {
            start: (300.0, 170.0),
            end: (100.0, 50.0),
        });
        assert_eq!(forward, backward);
    }

    #[test]
    fn grab_targets_are_larger_than_the_drawn_chips() {
        let handles = RegionResizeHandles::place(selection(100.0, 50.0, 200.0, 120.0));
        // A corner chip is 12 wide, so its ink stops 6px out; a rectangle this
        // roomy gets the full CORNER_REACH of 10.
        assert_eq!(
            handles.hit((109.0, 50.0)),
            Some(SelectionHandle::TopLeft),
            "inside the tolerance ring"
        );
        assert_eq!(
            handles.hit((111.0, 50.0)),
            None,
            "beyond the tolerance ring"
        );
    }

    #[test]
    fn a_short_side_drops_its_midpoint_grip_but_never_its_corners() {
        // 200 wide is roomy; 20 tall is not, so Left/Right go and Top/Bottom stay.
        let wide = RegionResizeHandles::place(selection(100.0, 50.0, 200.0, 20.0));
        assert_eq!(wide.hit((200.0, 50.0)), Some(SelectionHandle::Top));
        assert_eq!(wide.hit((200.0, 70.0)), Some(SelectionHandle::Bottom));
        assert_eq!(wide.hit((100.0, 52.0)), Some(SelectionHandle::TopLeft));
        assert_eq!(wide.hit((300.0, 52.0)), Some(SelectionHandle::TopRight));

        let tiny = RegionResizeHandles::place(selection(100.0, 50.0, 6.0, 6.0));
        for (point, handle) in [
            ((100.0, 50.0), SelectionHandle::TopLeft),
            ((106.0, 56.0), SelectionHandle::BottomRight),
        ] {
            assert_eq!(
                tiny.hit(point),
                Some(handle),
                "a collapsed rectangle stays resizable"
            );
        }
    }

    #[test]
    fn grips_never_swallow_the_move_area_however_small_the_rectangle() {
        // Grips are hit-tested before the interior, so a rectangle whose grab
        // targets met in the middle could be resized but never dragged.
        for side in [6.0, 12.0, 20.0, 30.0, 40.0, 41.0, 80.0, 400.0] {
            let handles = RegionResizeHandles::place(selection(0.0, 0.0, side, side));
            let middle = (side / 2.0, side / 2.0);
            assert_eq!(
                handles.hit(middle),
                None,
                "a {side} x {side} rectangle must keep a draggable centre"
            );
            assert_eq!(
                handles.hit((0.0, 0.0)),
                Some(SelectionHandle::TopLeft),
                "and must stay resizable at {side} x {side}"
            );
        }
    }

    #[test]
    fn grips_stop_shrinking_once_the_rectangle_can_seat_them() {
        // Above the threshold every rectangle gets identical, full-size grips.
        let threshold = CORNER_REACH / GRIP_SIDE_SHARE;
        let at = RegionResizeHandles::place(selection(0.0, 0.0, threshold, threshold));
        let far_above = RegionResizeHandles::place(selection(0.0, 0.0, 600.0, 600.0));
        let chip = |handles: RegionResizeHandles| handles.chips[0].unwrap();
        assert_eq!(chip(at).size, CORNER_SIZE);
        assert_eq!(chip(at).reach, CORNER_REACH);
        assert_eq!(chip(far_above).size, CORNER_SIZE);

        // Below it they scale linearly with the shortest side.
        let half = RegionResizeHandles::place(selection(0.0, 0.0, threshold / 2.0, 600.0));
        assert_eq!(chip(half).size, CORNER_SIZE / 2.0);
        assert_eq!(chip(half).reach, CORNER_REACH / 2.0);
    }

    #[test]
    fn corners_win_where_their_targets_overlap_an_edge_chip() {
        // At exactly the threshold the Top chip's target and the TopLeft chip's
        // target still meet; the corner must answer, matching hit priority in
        // the canvas selection handles.
        let handles =
            RegionResizeHandles::place(selection(0.0, 0.0, MIN_SIDE_FOR_EDGE_HANDLE, 200.0));
        assert_eq!(handles.hit((0.0, 0.0)), Some(SelectionHandle::TopLeft));
        assert_eq!(
            handles.hit((MIN_SIDE_FOR_EDGE_HANDLE / 2.0, 0.0)),
            Some(SelectionHandle::Top)
        );
    }

    #[test]
    fn tiny_rectangles_render_solid_dots_without_negative_border_geometry() {
        // Grips scale with the rectangle, so a 1x1 crop drives every chip below
        // the border width. Nothing may be asked to stroke a negative rect.
        for side in [1.0, 2.0, 3.0, 4.0, 6.0, 12.0] {
            let handles = RegionResizeHandles::place(selection(20.0, 20.0, side, side));
            let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 60, 60).unwrap();
            let ctx = cairo::Context::new(&surface).unwrap();
            render_region_resize_handles(&ctx, &handles, Some(SelectionHandle::TopLeft));
            assert_eq!(
                ctx.status(),
                Ok(()),
                "a {side} x {side} rectangle must leave Cairo healthy"
            );
            drop(ctx);
            surface.flush();
            let stride = surface.stride() as usize;
            let data = surface.data().unwrap();
            assert!(
                data[20 * stride + 20 * 4 + 3] > 0,
                "the top-left grip is still visible at {side} x {side}"
            );
            assert_eq!(
                data[55 * stride + 55 * 4 + 3],
                0,
                "and nothing leaks far outside it"
            );
        }
    }

    #[test]
    fn a_scaled_chip_keeps_its_border_inside_itself() {
        // The inset the renderer strokes must stay positive at every size.
        for size in [1.0_f64, 1.5, 4.0, 8.0, CORNER_SIZE] {
            let border = BORDER_WIDTH.min(size / 8.0);
            assert!(
                size - border > 0.0,
                "a {size} chip would stroke a {}-wide rectangle",
                size - border
            );
            let radius = RADIUS.min(size / 4.0);
            assert!(radius >= 0.0 && radius <= size / 2.0);
        }
    }

    #[test]
    fn rendering_paints_a_chip_at_every_offered_grip() {
        let handles = RegionResizeHandles::place(selection(20.0, 20.0, 160.0, 160.0));
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 200, 200).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        render_region_resize_handles(&ctx, &handles, Some(SelectionHandle::TopLeft));
        drop(ctx);
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().unwrap();
        let alpha = |x: usize, y: usize| data[y * stride + x * 4 + 3];

        for (x, y) in [
            (20, 20),
            (180, 20),
            (20, 180),
            (180, 180),
            (100, 20),
            (100, 180),
            (20, 100),
            (180, 100),
        ] {
            assert!(alpha(x, y) > 0, "grip at ({x}, {y})");
        }
        assert_eq!(alpha(100, 100), 0, "the interior stays clear");
    }
}
