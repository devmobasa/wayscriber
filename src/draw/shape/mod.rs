//! Shape definitions for screen annotations.

mod arrow_label;
mod bounds;
mod polygon;
mod smoothing;
mod step_marker;
mod text;
mod text_cache;
mod transform;
mod types;

pub use polygon::{
    PolygonKind, REGULAR_POLYGON_DEFAULT_SIDES, REGULAR_POLYGON_MAX_SIDES,
    REGULAR_POLYGON_MIN_SIDES, clamp_regular_sides,
};
pub use smoothing::{MAX_PEN_SMOOTHING, clamp_pen_smoothing, smooth_path, smooth_pressure_path};
pub use text_cache::TextMeasurer;
pub(crate) use text_cache::with_legacy_measurer;
pub use types::{
    ArrowLabel, ArrowStyle, BlurStyle, EmbeddedImage, EraserBrush, EraserKind, Shape,
    StepMarkerLabel,
};

pub(crate) use arrow_label::{ARROW_LABEL_BACKGROUND, arrow_label_ends, arrow_label_layout_with};
pub(crate) use bounds::{bounding_box_for_blur, bounding_box_for_eraser, bounding_box_for_points};
pub(crate) use polygon::{PolygonTemplate, generated_points, has_minimum_distinct_points};
pub(crate) use step_marker::{step_marker_outline_thickness, step_marker_radius_with};
pub(crate) use text::{
    bounding_box_for_sticky_note_preview_with, bounding_box_for_text_with, sticky_note_layout,
    sticky_note_layout_text, sticky_note_text_layout_with_measurer,
};
pub(crate) use text_cache::{
    CaretGeometry, LogicalBounds, TextMeasurement, VisualCaretDirection, VisualLineDirection,
    VisualLineEdge, caret_geometry_text, configured_layout,
};

#[cfg(test)]
mod tests;
