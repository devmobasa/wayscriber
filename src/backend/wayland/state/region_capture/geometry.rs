use crate::input::state::{RegionPurposeTag, RegionSelection};
use crate::screen_pixels::{
    ImagePixelRect, ImagePoint, PixelSpan, clamp_edge, pixel_span, snap_anchor, squared_cursor,
};

use super::super::screen_image::{ScreenSourceToken, screen_point_for_image_point};

/// One coherent view of the active drag for painting, readout, and delivery.
///
/// `image_span` deliberately permits an empty axis while the pointer is down;
/// `image_rect` becomes available only once both axes contain pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::backend::wayland) struct RegionSelectionGeometry {
    image_span: PixelSpan,
    display_selection: RegionSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum RegionPickerMeasurement {
    Point { x: u32, y: u32 },
    Size { width: u32, height: u32 },
}

impl RegionSelectionGeometry {
    pub const fn image_span(self) -> PixelSpan {
        self.image_span
    }

    pub const fn display_selection(self) -> RegionSelection {
        self.display_selection
    }

    pub fn image_rect(self) -> Option<ImagePixelRect> {
        self.image_span.try_into().ok()
    }
}

pub(super) fn selection_anchor(
    purpose: RegionPurposeTag,
    mapped: ImagePoint,
    bounds: (u32, u32),
) -> Option<ImagePoint> {
    if purpose.selection_policy().snap_anchor() {
        snap_anchor(mapped, bounds)
    } else {
        clamp_edge(mapped, bounds)
    }
}

pub(super) fn selection_geometry(
    purpose: RegionPurposeTag,
    source: ScreenSourceToken,
    anchor: ImagePoint,
    raw_edge: ImagePoint,
    logical_selection: Option<RegionSelection>,
    square_modifier: bool,
) -> Option<RegionSelectionGeometry> {
    let effective_edge = if square_modifier && purpose.selection_policy().allow_square() {
        squared_cursor(anchor, raw_edge, source.image_size)?
    } else {
        raw_edge
    };
    let image_span = pixel_span(anchor, effective_edge, source.image_size)?;
    let display_selection = if purpose.is_capture() {
        RegionSelection {
            start: screen_point_for_image_point(&source, anchor),
            end: screen_point_for_image_point(&source, effective_edge),
        }
    } else {
        logical_selection?
    };
    Some(RegionSelectionGeometry {
        image_span,
        display_selection,
    })
}

pub(super) fn whole_image_rect(
    purpose: RegionPurposeTag,
    bounds: (u32, u32),
) -> Option<ImagePixelRect> {
    if !purpose.is_capture() {
        return None;
    }
    ImagePixelRect::whole(bounds)
}

pub(super) fn point_measurement(
    purpose: RegionPurposeTag,
    mapped: ImagePoint,
    bounds: (u32, u32),
) -> Option<RegionPickerMeasurement> {
    if !purpose.is_capture() {
        return None;
    }
    let point = snap_anchor(mapped, bounds)?;
    Some(RegionPickerMeasurement::Point {
        x: point.x as u32,
        y: point.y as u32,
    })
}
