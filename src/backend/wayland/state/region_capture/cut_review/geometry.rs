use crate::backend::wayland::state::screen_image::{
    ScreenSourceToken, screen_rect_for_native_extent,
};
use crate::capture::{CutAxis, CutBand, output_size};
use crate::input::state::RegionSelection;
use crate::screen_pixels::{ImagePixelRect, ImagePoint, pixel_span};
use crate::util::Rect;

pub(in crate::backend::wayland::state::region_capture) fn dominant_cut_axis(
    dx: f64,
    dy: f64,
) -> CutAxis {
    if dy.abs() > dx.abs() {
        CutAxis::Rows
    } else {
        CutAxis::Columns
    }
}

pub(in crate::backend::wayland::state::region_capture) fn output_display_for(
    token: &ScreenSourceToken,
    source_rect: ImagePixelRect,
    cuts: &[CutBand],
) -> Option<RegionSelection> {
    let size = output_size((source_rect.width(), source_rect.height()), cuts).ok()?;
    native_extent_display(token, source_rect, size)
}

pub(in crate::backend::wayland::state::region_capture) fn native_extent_display(
    token: &ScreenSourceToken,
    source_rect: ImagePixelRect,
    size: (u32, u32),
) -> Option<RegionSelection> {
    let rect = screen_rect_for_native_extent(token, (source_rect.x(), source_rect.y()), size)?;
    Some(region_selection_from_rect(rect))
}

pub(in crate::backend::wayland::state::region_capture) fn region_selection_from_rect(
    rect: Rect,
) -> RegionSelection {
    RegionSelection {
        start: (f64::from(rect.x), f64::from(rect.y)),
        end: (
            f64::from(rect.x.saturating_add(rect.width)),
            f64::from(rect.y.saturating_add(rect.height)),
        ),
    }
}

pub(in crate::backend::wayland::state::region_capture) fn display_contains(
    display: RegionSelection,
    point: (f64, f64),
) -> bool {
    let left = display.start.0.min(display.end.0);
    let right = display.start.0.max(display.end.0);
    let top = display.start.1.min(display.end.1);
    let bottom = display.start.1.max(display.end.1);
    point.0 >= left && point.0 < right && point.1 >= top && point.1 < bottom
}

pub(in crate::backend::wayland::state::region_capture) fn logical_to_output_point(
    display: RegionSelection,
    output_size: (u32, u32),
    point: (f64, f64),
) -> Option<ImagePoint> {
    if output_size.0 == 0 || output_size.1 == 0 || !point.0.is_finite() || !point.1.is_finite() {
        return None;
    }
    let left = display.start.0.min(display.end.0);
    let right = display.start.0.max(display.end.0);
    let top = display.start.1.min(display.end.1);
    let bottom = display.start.1.max(display.end.1);
    let width = right - left;
    let height = bottom - top;
    if width <= 0.0 || height <= 0.0 || !width.is_finite() || !height.is_finite() {
        return None;
    }
    let x = ((point.0 - left) / width) * f64::from(output_size.0);
    let y = ((point.1 - top) / height) * f64::from(output_size.1);
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    Some(ImagePoint::new(
        x.clamp(0.0, f64::from(output_size.0)),
        y.clamp(0.0, f64::from(output_size.1)),
    ))
}

pub(super) fn quantized_cut(
    axis: CutAxis,
    display: RegionSelection,
    output_size: (u32, u32),
    start: (f64, f64),
    current: (f64, f64),
) -> Option<CutBand> {
    let first = logical_to_output_point(display, output_size, start)?;
    let second = logical_to_output_point(display, output_size, current)?;
    let span = pixel_span(first, second, output_size)?;
    match axis {
        CutAxis::Columns => {
            CutBand::from_unordered_edges(axis, span.x(), span.x().checked_add(span.width())?).ok()
        }
        CutAxis::Rows => {
            CutBand::from_unordered_edges(axis, span.y(), span.y().checked_add(span.height())?).ok()
        }
    }
}

pub(in crate::backend::wayland::state::region_capture) fn cut_band_display(
    display: RegionSelection,
    output_size: (u32, u32),
    axis: CutAxis,
    start: u32,
    end: u32,
) -> Option<RegionSelection> {
    if end <= start || output_size.0 == 0 || output_size.1 == 0 {
        return None;
    }
    let left = display.start.0.min(display.end.0);
    let right = display.start.0.max(display.end.0);
    let top = display.start.1.min(display.end.1);
    let bottom = display.start.1.max(display.end.1);
    let width = right - left;
    let height = bottom - top;
    match axis {
        CutAxis::Columns => {
            let x0 = left + f64::from(start) * width / f64::from(output_size.0);
            let x1 = left + f64::from(end) * width / f64::from(output_size.0);
            Some(RegionSelection {
                start: (x0, top),
                end: (x1, bottom),
            })
        }
        CutAxis::Rows => {
            let y0 = top + f64::from(start) * height / f64::from(output_size.1);
            let y1 = top + f64::from(end) * height / f64::from(output_size.1);
            Some(RegionSelection {
                start: (left, y0),
                end: (right, y1),
            })
        }
    }
}
