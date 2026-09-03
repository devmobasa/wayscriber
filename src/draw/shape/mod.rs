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
pub use types::{
    ArrowLabel, ArrowStyle, BlurStyle, EmbeddedImage, EraserBrush, EraserKind, Shape,
    StepMarkerLabel,
};

pub(crate) use arrow_label::{ARROW_LABEL_BACKGROUND, arrow_label_ends, arrow_label_layout};
pub(crate) use bounds::{bounding_box_for_blur, bounding_box_for_eraser, bounding_box_for_points};
pub(crate) use polygon::{PolygonTemplate, generated_points, has_minimum_distinct_points};
pub(crate) use step_marker::{step_marker_outline_thickness, step_marker_radius};
pub(crate) use text::{
    bounding_box_for_sticky_note_preview, bounding_box_for_text, sticky_note_layout,
    sticky_note_layout_text, sticky_note_text_layout,
};
pub(crate) use text_cache::{
    CaretGeometry, LogicalBounds, TextMeasurement, VisualCaretDirection, VisualLineDirection,
    VisualLineEdge, caret_at_visual_selection_edge, caret_geometry_text,
    caret_on_adjacent_visual_line, caret_on_adjacent_visual_position, caret_on_visual_line_edge,
    configured_layout, hit_test_text, measure_text_with_context, text_preview_geometry,
};

#[cfg(test)]
mod tests;
