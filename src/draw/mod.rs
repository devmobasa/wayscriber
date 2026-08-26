//! Rendering primitives and shape definitions (Cairo-based).
//!
//! This module defines the core drawing types used for screen annotation:
//! - [`Color`]: RGBA color representation with predefined color constants
//! - [`Shape`]: Different annotation types (lines, rectangles, text, etc.)
//! - [`Frame`]: Container for all shapes in the current drawing
//! - Rendering functions for Cairo-based output

pub mod canvas_set;
pub mod color;
pub mod dirty;
pub mod font;
pub mod frame;
pub mod render;
pub mod shape;
pub mod spotlight;

// Re-export commonly used types at module level
#[allow(unused_imports)]
pub use canvas_set::{BoardPages, PageDeleteOutcome};
pub use color::Color;
pub use dirty::{DirtyFullReason, DirtyRegionReport, DirtyTracker};
pub use font::{
    FontDescriptor, families_match, family_is_installed, monospace_font_families,
    system_font_families,
};
pub(crate) use font::{
    prewarm_system_font_catalog, system_font_catalog_is_ready, try_monospace_font_families,
    try_system_font_families,
};
pub use frame::{DrawnShape, Frame, ShapeId};
#[allow(unused_imports)]
pub(crate) use render::render_eraser_stroke;
pub(crate) use render::render_sticky_note_preview;
pub(crate) use render::with_saved_state;
#[allow(unused_imports)]
pub use render::{
    BlurRectParams, EraserReplayContext, IMMUTABLE_RASTER_SOURCE_TOKEN, SpotlightMagnifierMetrics,
    SpotlightMagnifierOutcome, SpotlightMagnifierScratch, SpotlightMagnifierSource, SpotlightPass,
    SpotlightRegion, SpotlightSnapshotStrategy, caret_line_width, caret_outline_width,
    painted_background_luminance, perceived_luminance, render_blur_rect, render_board_background,
    render_click_highlight, render_freehand_borrowed, render_marker_stroke_borrowed,
    render_selection_halo, render_selection_handles, render_shape, render_shape_over,
    render_shape_over_with_halo, render_shape_with_halo, render_spotlight_magnification_pass,
    render_spotlight_pass, render_sticky_note, render_text, render_text_over_with_halo,
    render_text_with_halo, selection_handle_rects, spotlight_regions_for_frame,
    sticky_note_foreground, text_outline_color,
};
#[allow(unused_imports)]
pub use shape::{
    ArrowLabel, ArrowStyle, BlurStyle, EmbeddedImage, EraserBrush, EraserKind, MAX_PEN_SMOOTHING,
    PolygonKind, REGULAR_POLYGON_DEFAULT_SIDES, REGULAR_POLYGON_MAX_SIDES,
    REGULAR_POLYGON_MIN_SIDES, Shape, StepMarkerLabel, clamp_regular_sides,
};
pub use spotlight::{
    DEFAULT_SPOTLIGHT_MAGNIFICATION, MAX_SPOTLIGHT_MAGNIFICATION, MIN_SPOTLIGHT_MAGNIFICATION,
    SPOTLIGHT_MAGNIFICATION_STEP, default_spotlight_magnification,
    deserialize_spotlight_magnification, format_spotlight_magnification,
    normalize_spotlight_magnification, spotlight_magnification_is_active,
};

// Re-export color constants for public API (unused internally but part of public interface)
#[allow(unused_imports)]
pub use color::{BLACK, BLUE, GREEN, ORANGE, PINK, RED, TRANSPARENT, WHITE, YELLOW};

// Re-export utility functions for public API (unused internally but part of public interface)
#[allow(unused_imports)]
pub use render::fill_transparent;
